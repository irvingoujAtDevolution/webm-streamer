use std::{cell::LazyCell, path::PathBuf, sync::Arc};

use anyhow::Context;
use chrono::Local;
use tokio::{
    fs,
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter},
    sync::mpsc,
};
use tracing::info;
use typed_builder::TypedBuilder;
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};

use crate::utils::{recording_manager::RecordingManager, FileWithLoggin};

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
            mut client_stream,
            recording_manager,
        } = self;
        let date = Local::now();
        let recording_file_name = format!("{}.webm", date.format("%d_%H_%M_%S"));
        let recording_file = RECORDING_DIR.as_ref().join(&recording_file_name);

        info!("Recording to file: {:?}", recording_file);
        let mut shutdown_signal = recording_manager
            .start_recording(recording_file.clone())
            .await;

        let mut open_options = fs::OpenOptions::new();

        let open_options = open_options
            .read(false)
            .write(true)
            .truncate(true)
            .create(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);

        info!("Opening file for recording");

        let res = match open_options.open(&recording_file).await {
            Ok(file) => {
                let file = FileWithLoggin::new(file);
                let mut file = BufWriter::new(file);

                let copy_fut = io::copy(&mut client_stream, &mut file);

                tokio::select! {
                    res = copy_fut => {
                        res.context("JREC streaming to file").map(|_| ())
                    },
                    _ = shutdown_signal.wait_for_stop() => {
                        client_stream.shutdown().await.context("shutdown client stream")
                    },
                }
                .inspect_err(|e| {
                    info!("Error in JREC push: {:?}", e);
                })
            }
            Err(e) => Err(anyhow::Error::new(e).context("failed to open file".to_string())),
        };

        res
    }
}
