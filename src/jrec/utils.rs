use std::path::PathBuf;

use hyper::StatusCode;

use super::recording::RECORDING_DIR;

pub async fn get_latestest_recording() -> Result<PathBuf, StatusCode> {
    let mut recordings = tokio::fs::read_dir(RECORDING_DIR.as_ref())
        .await
        .map_err(|e| {
            tracing::error!("Error reading recording directory: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut latest_recording = None;

    while let Ok(Some(recording)) = recordings.next_entry().await {
        let metadata = recording.metadata().await.map_err(|e| {
            tracing::error!("Error reading recording metadata: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if !metadata.is_file() {
            continue;
        }

        let time = metadata
            .created()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some((latest_time, _)) = latest_recording {
            if time > latest_time {
                latest_recording = Some((time, recording));
            }
        } else {
            latest_recording = Some((time, recording));
        }
    }

    let (_, recording) = latest_recording.ok_or(StatusCode::NOT_FOUND)?;

    Ok(recording.path())
}

pub async fn get_recording_list() -> Result<Vec<(String, String)>, StatusCode> {
    let mut recordings = tokio::fs::read_dir(RECORDING_DIR.as_ref())
        .await
        .map_err(|e| {
            tracing::error!("Error reading recording directory: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut recording_list = Vec::new();

    while let Ok(Some(recording)) = recordings.next_entry().await {
        let metadata = recording.metadata().await.map_err(|e| {
            tracing::error!("Error reading recording metadata: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if !metadata.is_file() {
            continue;
        }

        let time = metadata
            .created()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let time: u64 = time
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .as_secs();

        let name = recording.file_name().to_string_lossy().to_string();

        if !name.ends_with(".webm") {
            continue;
        }

        recording_list.push((name, time));
    }

    //sort the recording list by time, newest first
    recording_list.sort_by(|a, b| b.1.cmp(&a.1));

    let recording_list = recording_list
        .into_iter()
        .map(|(name, time)| {
            let time = chrono::DateTime::from_timestamp(time as i64, 0).unwrap();
            (name, time.to_string())
        })
        .collect();

    Ok(recording_list)
}

pub async fn find_recording(file_name: &str) -> Result<PathBuf, StatusCode> {
    let mut recordings = tokio::fs::read_dir(RECORDING_DIR.as_ref())
        .await
        .map_err(|e| {
            tracing::error!("Error reading recording directory: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    while let Ok(Some(recording)) = recordings.next_entry().await {
        let metadata = recording.metadata().await.map_err(|e| {
            tracing::error!("Error reading recording metadata: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if !metadata.is_file() {
            continue;
        }

        let name = recording.file_name().to_string_lossy().to_string();

        // Check if the current file matches the requested file name
        if name == file_name {
            return Ok(recording.path());
        }
    }

    // If no matching file is found, return a 404 status
    Err(StatusCode::NOT_FOUND)
}
