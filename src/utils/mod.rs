use std::pin::Pin;

use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWrite},
};
use tracing::debug;

pub mod state;
pub mod recording_manager;

pub struct FileWithLoggin {
    file: File,
}

impl FileWithLoggin {
    pub fn new(file: File) -> Self {
        Self { file }
    }
}

impl AsyncRead for FileWithLoggin {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().file).poll_read(cx, buf)
    }
}

impl AsyncWrite for FileWithLoggin {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // debug!(len=%buf.len(), "writing to file");
        Pin::new(&mut self.get_mut().file).poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().file).poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().file).poll_shutdown(cx)
    }
}
