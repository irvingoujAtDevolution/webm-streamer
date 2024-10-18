use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;
use tokio::{
    fs,
    io::{self, AsyncRead},
    time::Sleep,
};
use tracing::{error, info};
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};

use crate::axum_range::{AsyncSeekStart, RangeBody};

// Candidate for streaming the webm file
pub struct FileReaderCandidate {
    file: tokio::fs::File,
    size: u64,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl FileReaderCandidate {
    pub async fn open(path: PathBuf) -> std::io::Result<Self> {
        let mut open_options = fs::OpenOptions::new();

        let file = open_options
            .read(true)
            .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
            .open(&path)
            .await?;

        let size = file.metadata().await?.len();
        Ok(FileReaderCandidate {
            file,
            size,
            sleep: None,
        })
    }
}

const MAX_SIZE_PER_REQUEST: u64 = 1024 * 10;

impl RangeBody for FileReaderCandidate {
    fn byte_size(&self) -> Option<u64> {
        info!("Returning byte size: {}", self.size);
        Some(self.size)
    }

    fn max_size_per_request(&self) -> u64 {
        MAX_SIZE_PER_REQUEST
    }
}

impl AsyncRead for FileReaderCandidate {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if let Some(sleep) = self.sleep.as_mut() {
            match sleep.as_mut().poll_unpin(cx) {
                Poll::Ready(()) => {
                    self.sleep = None;
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }

        let file = Pin::new(&mut self.file);
        let lenth_before_fill = buf.filled().len();
        let read = file.poll_read(cx, buf);

        match read {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => {
                error!("Error reading file: {:?}", e);
                Poll::Ready(Err(e))
            }
            Poll::Ready(Ok(())) => {
                // end of file, we don't want this, we want to keep the connection open so we can stream the file
                if buf.filled().is_empty()
                    || buf.filled().len() == lenth_before_fill
                    || buf.filled().len() < MAX_SIZE_PER_REQUEST as usize
                {
                    info!("Sleeping for 1 second, waiting for more data");
                    let sleep = tokio::time::sleep(std::time::Duration::from_secs(1));
                    let mut sleep = Box::pin(sleep);
                    let poll_sleep = sleep.poll_unpin(cx);
                    self.sleep = Some(sleep);

                    if !poll_sleep.is_pending() {
                        panic!("Sleep should always be pending");
                    }

                    Poll::Pending
                } else {
                    info!("Read {} bytes", buf.filled().len());
                    Poll::Ready(Ok(()))
                }
            }
        }
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
