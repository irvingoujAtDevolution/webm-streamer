use std::{cell::LazyCell, path::PathBuf, sync::Arc};

use chrono::Local;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::info;
use typed_builder::TypedBuilder;

use crate::utils::recording_manager::RecordingManager;

pub const RECORDING_DIR: LazyCell<Arc<PathBuf>> = LazyCell::new(|| {
    let home_dir = dirs::home_dir().expect("home directory");
    let recording_dir = home_dir
        .join("code")
        .join("webm-streamer")
        .join("recordings");
    Arc::new(recording_dir)
});

#[derive(TypedBuilder)]
pub struct ClientPush<S> {
    client_stream: S,
    recording_manager: Arc<RecordingManager>,
}

impl<S> ClientPush<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub async fn run(self) -> anyhow::Result<()> {
        info!("Starting JREC push");
        let Self {
            client_stream,
            recording_manager,
        } = self;
        let date = Local::now();
        let recording_file_name = format!("{}.webm", date.format("%d_%H_%M_%S"));
        let recording_file = RECORDING_DIR.as_ref().join(&recording_file_name);

        info!("Recording to file: {:?}", recording_file);

        recording_manager
            .start_recording(recording_file.clone(), client_stream)
            .await?
            .await?
    }
}
