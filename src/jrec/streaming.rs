use axum::extract::ws::WebSocket;

use super::slow_reader::FileReaderCandidate;

pub async fn test_stream(
    reader: tokio::fs::File,
    ws: WebSocket,
){
    // reader.rea
}