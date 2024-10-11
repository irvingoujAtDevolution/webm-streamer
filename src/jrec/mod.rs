use std::path::PathBuf;

use axum::extract::{Query, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use axum::{body::Body, extract::ws::WebSocket};
use axum::{Json, Router};
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use hyper::StatusCode;
use recording::ClientPush;
use slow_reader::FileReaderCandidate;
use streaming::test_stream;
use tokio::fs::File;
use tracing::info;
use utils::{find_recording, get_latestest_recording, get_recording_list};
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
use ws::websocket_compat;

use crate::axum_range::{KnownSize, Ranged};

pub mod recording;
pub mod slow_reader;
pub mod streaming;
pub mod utils;
pub mod ws;

pub fn make_router() -> Router {
    let router = Router::new()
        .route("/push", get(jrec_push))
        .route("/test", get(test))
        .route("/stream-file", get(stream_file))
        .route("/list-recording", get(list_recording))
        .route("/pull", get(pull_recording_file));

    Router::new().nest("/jet/jrec", router)
}

pub async fn list_recording() -> Result<Json<Vec<(String, String)>>, StatusCode> {
    let recording_list = get_recording_list().await?;
    Ok(Json(recording_list))
}

#[derive(serde::Deserialize)]
pub struct RecordingQuery {
    pub recording: Option<String>,
}

async fn get_path(query: Query<RecordingQuery>) -> Result<PathBuf, StatusCode> {
    if let Some(recording) = query.0.recording {
        find_recording(&recording).await
    } else {
        get_latestest_recording().await
    }
}

async fn test(ws: WebSocketUpgrade, query: Query<RecordingQuery>) -> Result<Response, StatusCode> {
    let path = get_path(query).await?;

    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
        .open(&path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = ws.on_upgrade(move |socket| test_stream(file, socket));

    Ok(response)
}

async fn stream_file(
    range: Option<TypedHeader<Range>>,
    query: Query<RecordingQuery>,
) -> Result<Ranged<KnownSize<File>>, StatusCode> {
    let file = get_path(query).await?;

    let file = File::open(file)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file = KnownSize::file(file)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let range = range.map(|TypedHeader(range)| range);

    Ok(Ranged::new(range, file))
}

async fn jrec_push(ws: WebSocketUpgrade) -> Result<Response, StatusCode> {
    tracing::info!("JREC push request");
    let response = ws.on_upgrade(handle_jrec_push);
    Ok(response)
}

async fn handle_jrec_push(ws: WebSocket) {
    let (shutdown_signal, receiver) = tokio::sync::mpsc::channel(1);
    tracing::info!("Upgrade to websocket");
    let result = ClientPush::builder()
        .client_stream(websocket_compat(ws))
        .shutdown_signal(receiver)
        .build()
        .run()
        .await;

    if let Err(e) = result {
        tracing::error!("Error in jrec push: {:?}", e);
    }
}

async fn pull_recording_file(query: Query<RecordingQuery>) -> Result<Response, StatusCode> {
    info!("Pulling recording file: {:?}", query.recording);
    let path = get_path(query).await?;
    info!("Serving recording: {:?}", path);
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(
        tokio::fs::File::open(path).await.expect("file not found"),
    ));

    Ok(Response::new(body))
}
