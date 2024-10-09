use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::FutureExt;
use tokio::io::{self, AsyncRead, ReadBuf};
use tracing::info;

use crate::axum_range::{AsyncSeekStart, RangeBody};

// Candidate for streaming the webm file
pub struct FileReaderCandidate {
    pub path: PathBuf,
    file: tokio::fs::File,
    size: u64,
}

impl FileReaderCandidate {
    pub async fn open(path: PathBuf) -> std::io::Result<Self> {
        let file = tokio::fs::File::open(&path).await?;
        let size = file.metadata().await?.len();
        Ok(FileReaderCandidate { path, file, size })
    }
}

impl RangeBody for FileReaderCandidate {
    fn byte_size(&self) -> Option<u64> {
        info!("Returning byte size: {}", self.size);
        Some(self.size)
    }

    fn max_size_per_request(&self) -> u64 {
        1024 * 100
    }
}

impl AsyncRead for FileReaderCandidate {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        info!("Polling read");
        Pin::new(&mut self.file).poll_read(cx, buf)
    }
}

impl AsyncSeekStart for FileReaderCandidate {
    fn start_seek(mut self: Pin<&mut Self>, position: u64) -> io::Result<()> {
        info!("Seeking to position: {}", position);
        Pin::new(&mut self.file).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Complete the seek operation on the underlying file
        Pin::new(&mut self.file).poll_complete(cx)
    }
}
