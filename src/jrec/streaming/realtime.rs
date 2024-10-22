use anyhow::Context;
use axum::extract::ws::WebSocket;
use futures::{SinkExt, StreamExt};
use tokio::io::AsyncReadExt;
use tokio_util::codec::Framed;
use tracing::{error, info, warn};

use crate::{
    jrec::{streaming::std_stream::{AsyncBufferReader, StdStream}, ws::websocket_compat},
    utils::state::AppState,
};

use super::SimpleCodec;

pub async fn handle_realtime_stream(
    file: std::path::PathBuf,
    websocket: WebSocket,
    state: AppState,
) {
    let recording_manager = state.recording_manager();
    let mut stream_read = match recording_manager.start_streaming(&file).await {
        Ok(stream_read) => stream_read,
        Err(e) => {
            warn!("Not streaming, read as file, error: {:?}", e);
            let open_file = tokio::task::spawn(async move {
                let file = tokio::fs::File::open(&file).await?;
                let stream = AsyncBufferReader::from_file(file).await?;
                Ok::<_, anyhow::Error>(stream)
            });

            let result = open_file.await;

            

            match result {
                Ok(Ok(stream_read)) => stream_read,
                _ => {
                    error!("Failed to open file: {:?}", result);
                    return;
                }
            }
        }
    };

    let mut framed = Framed::new(websocket_compat(websocket), SimpleCodec);

    tokio::spawn(async move {
        {
            info!("Starting realtime stream");
            loop {
                let Some(request) = framed.next().await else {
                    tracing::info!("Websocket closed");
                    break;
                };

                info!("Received request: {:?}", request);

                let request = request.with_context(|| "Failed to read from websocket")?;

                let size = match request {
                    super::ClientRequest::Pull { size } => size.unwrap_or(1024 * 1024),
                    super::ClientRequest::Stop => {
                        tracing::info!("Stopping stream");
                        break;
                    }
                };
                let mut buffer = vec![0; size];
                let data = stream_read.read(&mut buffer).await?;

                framed
                    .send(super::ServerResponse::Chunk {
                        metadata: None,
                        data: buffer[..data].to_vec(),
                    })
                    .await?;
            }

            Ok::<_, anyhow::Error>(())
        }
        .inspect_err(|e| tracing::error!("Error in realtime stream: {:?}", e))
    });
}
