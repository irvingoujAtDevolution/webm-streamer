use std::sync::Arc;

use axum::extract::ws::WebSocket;
use blocking::StdStreamingFile;
use bytes::{BufMut, BytesMut};
use futures::{SinkExt, StreamExt, TryStreamExt};
use tokio::io::{self, AsyncReadExt, AsyncSeekExt};
use tokio_util::codec::{Decoder, Encoder, Framed};
use tracing::{debug, error, info};
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};

use crate::utils::recording_manager::RecordingManager;

use super::ws::websocket_compat;

pub mod blocking;
pub mod realtime;
pub mod std_stream;

#[derive(Debug, Clone, serde::Deserialize)]
pub enum ClientRequest {
    Pull { size: Option<usize> },
    Stop,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Metadata {
    pub chunk_size: usize,
    pub offset: usize,
    total_size: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ServerResponse {
    Chunk {
        metadata: Option<Metadata>,
        data: Vec<u8>,
    },
    EOF,
}

impl ServerResponse {
    pub fn type_code(&self) -> u8 {
        match self {
            ServerResponse::Chunk {
                metadata: _,
                data: _,
            } => 0,
            ServerResponse::EOF => 1,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ServerResponse::Chunk {
                metadata: _,
                data: _,
            } => "Chunk",
            ServerResponse::EOF => "EOF",
        }
    }
}

pub struct StreamFile {
    inner: tokio::fs::File,
    path: std::path::PathBuf,
}

impl StreamFile {
    pub async fn open_read(path: std::path::PathBuf) -> io::Result<Self> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
            .open(&path)
            .await?;

        Ok(StreamFile { inner: file, path })
    }

    pub async fn open_from(old: &mut Self) -> io::Result<Self> {
        let path = old.path.clone();
        Self::open_read(path).await
    }

    pub(crate) fn from_std(file: StdStreamingFile) -> Self {
        let (inner, path) = file.destruct();

        StreamFile {
            inner: tokio::fs::File::from_std(inner),
            path,
        }
    }
}

pub async fn test_stream(
    file: std::path::PathBuf,
    ws: WebSocket,
    recording_manager: Arc<RecordingManager>,
) {
    let ws = websocket_compat(ws);
    let ws_frame = Framed::new(ws, SimpleCodec);

    // we should have a rwlock here that keep reading the file and write it to a temp file, use the temp file to stream
    let source_file = StreamFile::open_read(file.clone())
        .await
        .expect("open file");

    tokio::spawn(async move {
        if let Err(e) = handle_request(source_file, ws_frame, recording_manager).await {
            error!("Error handling request: {:?}", e);
        }
    });
}

async fn handle_request(
    mut file: StreamFile,
    ws_frame: Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, SimpleCodec>,
    recording_manager: Arc<RecordingManager>,
) -> anyhow::Result<()> {
    let mut ws_frame = ws_frame;

    let mut seek_position = file.inner.seek(io::SeekFrom::Start(0)).await?;
    loop {
        let Some(request) = ws_frame.next().await else {
            return Ok(());
        };

        if let Err(e) = request {
            if Some(10054) == e.raw_os_error() {
                info!("Client disconnected");
                return Ok(());
            }

            return Err(e.into());
        }
        let request = request?;
        info!(?request, "Received request");
        match request {
            ClientRequest::Stop => {
                info!("Client requested stop");
                return Ok(());
            }
            ClientRequest::Pull { size } => 'pull_loop: loop {
                file = reopen(file, seek_position).await?;
                let size = size.unwrap_or(1024);
                let mut buf = vec![0; size];
                let n = file.inner.read(&mut buf).await?;
                debug!(data_size = n, "Read data from file");
                if n == 0 {
                    if recording_manager.is_recording(&file.path).await {
                        info!("Recording is still in progress, waiting for more data");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue 'pull_loop;
                    }

                    let response = ServerResponse::EOF;
                    info!("Sending EOF response");
                    ws_frame.send(response).await?;
                }

                let response = ServerResponse::Chunk {
                    metadata: Some(Metadata {
                        chunk_size: n,
                        offset: file.inner.seek(io::SeekFrom::Current(0)).await? as usize,
                        total_size: file.inner.metadata().await?.len() as usize,
                    }),
                    data: buf[..n].to_vec(),
                };

                seek_position = file.inner.seek(io::SeekFrom::Current(0)).await?;

                info!(data_size = n, "Sending response");
                ws_frame.send(response).await?;
                break 'pull_loop;
            },
        }
    }
}

async fn reopen(mut file: StreamFile, seek_position: u64) -> tokio::io::Result<StreamFile> {
    let mut file = StreamFile::open_from(&mut file).await?;
    file.inner.seek(io::SeekFrom::Start(seek_position)).await?;
    Ok(file)
}

pub struct SimpleCodec;

impl Decoder for SimpleCodec {
    type Item = ClientRequest;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }
        let msg = serde_json::from_slice(src)?;
        src.clear();
        Ok(Some(msg))
    }
}

impl Encoder<ServerResponse> for SimpleCodec {
    type Error = std::io::Error;

    /// We encode the messages as binary
    /// Format:
    /// The first byte is the type code
    /// The next 4 bytes are the length of the metadata if any
    /// The next M bytes specified are the metadata
    /// Since Websocket have size, the next N bytes are the actual data
    /// The protocol may split large messages into frames, but the receiving side will reassemble them into a complete message before passing them to the application
    fn encode(&mut self, item: ServerResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        tracing::info!(message_type=%item.name(), "Encoding message");
        let type_code = item.type_code();
        dst.put_u8(type_code);
        match item {
            ServerResponse::EOF => return Ok(()),
            ServerResponse::Chunk { metadata, data } => {
                if metadata.is_none() {
                    dst.put_u32(0);
                } else {
                    let metadata = serde_json::to_vec(&metadata.unwrap())
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    dst.put_u32(metadata.len() as u32);
                    dst.put_slice(&metadata);
                }

                dst.put_slice(&data);
            }
        };

        Ok(())
    }
}
