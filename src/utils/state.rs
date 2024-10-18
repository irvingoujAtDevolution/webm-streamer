use std::sync::Arc;

use super::recording_manager::RecordingManager;

#[derive(Debug, Clone)]
pub struct AppState {
    recording_manager: Arc<RecordingManager>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            recording_manager: RecordingManager::new(),
        }
    }

    pub fn recording_manager(&self) -> Arc<RecordingManager> {
        self.recording_manager.clone()
    }
}
