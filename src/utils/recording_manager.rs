use std::{collections::HashMap, path::PathBuf, sync::Arc};

use futures::lock::Mutex;
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub struct RecordingManager {
    recording_map: Mutex<HashMap<String, Sender<()>>>,
}

impl RecordingManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            recording_map: Mutex::new(HashMap::new()),
        })
    }

    pub async fn start_recording(self: Arc<Self>, recording_id: PathBuf) -> RecordingSignal {
        RecordingSignal::new(recording_id, self.clone()).await
    }

    pub async fn is_recording(&self, recording_id: &PathBuf) -> bool {
        let recording_map = self.recording_map.lock().await;

        let recording_id = recording_id
            .file_name()
            .expect("recording_id")
            .to_string_lossy()
            .to_string();

        return recording_map.contains_key(recording_id.as_str());
    }

    fn try_stop_recording(self: Arc<Self>, recording_id: PathBuf) {
        tokio::spawn(self.stop_recording_inner(recording_id));
    }

    async fn stop_recording_inner(self: Arc<Self>, recording_id: PathBuf) {
        let recording_id = recording_id
            .file_name()
            .expect("recording_id")
            .to_string_lossy()
            .to_string();

        let mut recording_map = self.recording_map.lock().await;
        let tx = recording_map.remove(&recording_id);
        if let Some(tx) = tx {
            tx.send(()).await.ok();
        }
    }

    async fn start_recording_inner(self: &Self, recording_id: PathBuf) -> Receiver<()> {
        let mut recording_map = self.recording_map.lock().await;
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let recording_id = recording_id
            .file_name()
            .expect("recording_id")
            .to_string_lossy()
            .to_string();

        recording_map.insert(recording_id, tx);
        return rx;
    }
}

pub struct RecordingSignal {
    recording_id: PathBuf,
    recording_manager: Arc<RecordingManager>,
    recording_signal: Receiver<()>,
}

impl RecordingSignal {
    async fn new(recording_id: PathBuf, recording_manager: Arc<RecordingManager>) -> Self {
        let recording_signal = recording_manager
            .start_recording_inner(recording_id.clone())
            .await;
        Self {
            recording_id,
            recording_signal,
            recording_manager,
        }
    }

    pub async fn wait_for_stop(&mut self) {
        self.recording_signal.recv().await;
    }
}

impl Drop for RecordingSignal {
    fn drop(&mut self) {
        self.recording_manager
            .clone()
            .try_stop_recording(self.recording_id.clone());
    }
}
