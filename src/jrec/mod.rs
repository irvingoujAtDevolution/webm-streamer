use std::path::PathBuf;

use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use axum::{body::Body, extract::ws::WebSocket};
use axum::{Json, Router};
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use hyper::StatusCode;
use recording::ClientPush;
use streaming::realtime::handle_realtime_stream;
use streaming::test_stream;
use tokio::fs::File;
use tracing::info;
use utils::{find_recording, get_latestest_recording, get_recording_list};
use ws::websocket_compat;

use crate::axum_range::{KnownSize, Ranged};
use crate::utils::state::AppState;

pub mod recording;
pub mod slow_reader;
pub mod streaming;
pub mod utils;
pub mod webm;
pub mod ws;

pub fn make_router() -> Router<AppState> {
    let router = Router::new()
        .route("/push", get(jrec_push))
        .route("/test", get(test))
        .route("/stream-realtime", get(stream_realtime))
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

async fn test(
    ws: WebSocketUpgrade,
    query: Query<RecordingQuery>,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    let path = get_path(query).await?;
    let response =
        ws.on_upgrade(move |socket| test_stream(path, socket, state.recording_manager()));

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

async fn jrec_push(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    tracing::info!("JREC push request");
    let response = ws.on_upgrade(|socket| handle_jrec_push(socket, state));
    Ok(response)
}

async fn handle_jrec_push(ws: WebSocket, state: AppState) {
    tracing::info!("Upgrade to websocket");
    let result = ClientPush::builder()
        .client_stream(websocket_compat(ws))
        .recording_manager(state.recording_manager())
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

async fn stream_realtime(
    query: Query<RecordingQuery>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    // We assume this path is a recording file and exist
    let path = get_path(query).await?;
    let response = ws.on_upgrade(|socket| handle_realtime_stream(path, socket, state));
    Ok(response)
}
