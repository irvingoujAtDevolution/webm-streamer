use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use futures::lock::Mutex;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter},
    sync::mpsc::{Receiver, Sender},
    task::JoinHandle,
};
use tracing::info;

use crate::{
    jrec::{streaming::std_stream::StdStream, webm::stream_parser::StreamParser},
    utils::FileWithLoggin,
};

use super::file::{open_read, open_write};

struct RecordingControl {
    termination_sender: Sender<()>,
    streamer: Option<StreamParser>,
}

impl RecordingControl {
    async fn terminate(&self) -> anyhow::Result<()> {
        self.termination_sender.send(()).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct RecordingManager {
    recording_map: Mutex<HashMap<PathBuf, RecordingControl>>,
}

impl RecordingManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            recording_map: Mutex::new(HashMap::new()),
        })
    }

    pub async fn start_recording<S>(
        self: Arc<Self>,
        recording_path: PathBuf,
        mut client_stream: S,
    ) -> anyhow::Result<JoinHandle<anyhow::Result<()>>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let file = open_write(&recording_path).await?;
        info!(?recording_path, "Recording started");
        // Debug purposes, I can one click to open the file for streaming
        client_stream
            .write(
                recording_path
                    .file_name()
                    .expect("file_name")
                    .to_str()
                    .expect("to_str")
                    .as_bytes(),
            )
            .await
            .inspect_err(|e| info!(?e, "Failed to write file name"))?;
        client_stream.flush().await?;

        let mut recording_handle = RecordingHandle::new(&recording_path, self.clone()).await;

        let handle = tokio::task::spawn(async move {
            let file = FileWithLoggin::new(file);
            let mut file = BufWriter::new(file);

            let result = tokio::select! {
                res = tokio::io::copy(&mut client_stream, &mut file) => {
                    res.context("JREC streaming to file").map(|_| ())
                }
                _ = recording_handle.wait_for_stop() => {
                    Ok(())
                }
            };

            info!("Recording finished");
            result
        });

        Ok(handle)
    }

    pub async fn is_recording(&self, recording_id: &Path) -> bool {
        let recording_map = self.recording_map.lock().await;
        let recording_id = tokio::fs::canonicalize(recording_id)
            .await
            .expect("recording_id");

        recording_map.contains_key(&recording_id)
    }

    pub async fn start_streaming(&self, recording_path: &Path) -> anyhow::Result<StdStream> {
        let recording_path = tokio::fs::canonicalize(recording_path).await?;

        let mut recording_map = self.recording_map.lock().await;
        let Some(control) = recording_map.get_mut(&recording_path) else {
            // Not being recorded, just stream the file
            let file = open_read(&recording_path).await?;
            return Ok(StdStream::from_file(file).await?);
        };

        if control.streamer.is_none() {
            let stream_parser = StreamParser::new(&recording_path).await?;
            control.streamer = Some(stream_parser);
        };

        let streamer = control.streamer.as_ref().unwrap();

        let stream = streamer.spawn().await?;

        Ok(stream)
    }
}

impl RecordingManager {
    fn try_stop_recording(self: Arc<Self>, recording_id: PathBuf) {
        tokio::spawn(self.stop_recording_inner(recording_id));
    }

    async fn stop_recording_inner(self: Arc<Self>, recording_id: PathBuf) {
        let recording_id = tokio::fs::canonicalize(recording_id)
            .await
            .expect("recording_id");

        let mut recording_map = self.recording_map.lock().await;
        let tx = recording_map.remove(&recording_id);
        if let Some(handle) = tx {
            handle.terminate().await.ok();
        }
    }

    async fn start_recording_inner(&self, recording_id: PathBuf) -> Receiver<()> {
        let mut recording_map = self.recording_map.lock().await;
        let (sender, receiver) = tokio::sync::mpsc::channel(1);

        let recording_id = tokio::fs::canonicalize(recording_id)
            .await
            .expect("recording_id");

        recording_map.insert(
            recording_id,
            RecordingControl {
                termination_sender: sender,
                streamer: None,
            },
        );
        receiver
    }
}

pub struct RecordingHandle {
    recording_id: PathBuf,
    recording_manager: Arc<RecordingManager>,
    recording_signal: Receiver<()>,
}

impl RecordingHandle {
    async fn new(recording_id: &Path, recording_manager: Arc<RecordingManager>) -> Self {
        let recording_signal = recording_manager
            .start_recording_inner(recording_id.to_path_buf())
            .await;
        Self {
            recording_id: recording_id.to_path_buf(),
            recording_signal,
            recording_manager,
        }
    }

    pub async fn wait_for_stop(&mut self) {
        self.recording_signal.recv().await;
    }
}

impl Drop for RecordingHandle {
    fn drop(&mut self) {
        self.recording_manager
            .clone()
            .try_stop_recording(self.recording_id.clone());
    }
}
