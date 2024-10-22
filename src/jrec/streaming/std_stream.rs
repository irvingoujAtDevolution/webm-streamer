use std::{
    fmt::Debug,
    future::Future,
    pin::Pin,
    sync::{atomic::AtomicI16, Arc},
    task::{Context, Poll},
};

use futures::FutureExt;
use tokio::{
    io::{AsyncReadExt, ReadBuf},
    sync::{mpsc, Mutex},
};
use tracing::{debug, info, warn};

const NAME: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];
const NAME_COUNT: AtomicI16 = AtomicI16::new(0);

pub struct StdStream {
    // pub for debugging purposes
    pub write_buffer: Vec<u8>,
    pub read_buffer: Vec<u8>,

    write_signal: Arc<tokio::sync::Notify>,
    // Debug purposes
    id: &'static str,
}

impl Debug for StdStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdStream").field("id", &self.id).finish()
    }
}

impl Default for StdStream {
    fn default() -> Self {
        Self::new()
    }
}

impl StdStream {
    pub fn new() -> Self {
        Self {
            write_buffer: Vec::new(),
            read_buffer: Vec::new(),
            write_signal: Arc::new(tokio::sync::Notify::new()),
            id: NAME[NAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as usize],
        }
    }

    fn wake_when_write(&self, cx: &mut std::task::Context<'_>) {
        // Clone the `write_signal` so we can use it across threads
        let write_signal = self.write_signal.clone();

        // Create a future that will wait for a notification
        let waker = cx.waker().clone();

        // Spawn a task to wake the waker when the buffer is written to
        tokio::spawn(async move {
            // Wait for notification
            write_signal.notified().await;
            // Wake the task once notification is received
            debug!("StdStream::wake_when_write, notified");
            waker.wake();
        });
    }

    fn wake(&self) {
        // Notify one task that is waiting
        debug!("StdStream::wake");
        self.write_signal.notify_one();
    }
}

impl std::io::Write for StdStream {
    #[tracing::instrument(skip(self, buf))]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[tracing::instrument(skip(self))]
    fn flush(&mut self) -> std::io::Result<()> {
        tracing::debug!(id = self.id, "StdStream::flush");
        self.read_buffer.append(&mut self.write_buffer);
        tracing::debug!(
            id = self.id,
            read_buffer_len = self.read_buffer.len(),
            write_buffer_len = self.write_buffer.len(),
            read_buffer_ptr = self.read_buffer.as_ptr() as usize,
            "StdStream::flush"
        );
        self.write_buffer.clear();
        self.wake();
        Ok(())
    }
}

impl tokio::io::AsyncRead for StdStream {
    #[tracing::instrument(skip(self, cx, read_buf))]
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        read_buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        tracing::debug!(id = self.id, "StdStream::poll_read");

        if !self.read_buffer.is_empty() {
            let len = std::cmp::min(read_buf.remaining(), self.read_buffer.len());
            read_buf.put_slice(&self.read_buffer[..len]);
            self.read_buffer.drain(..len); // Drain only after reading
            tracing::debug!(
                read_buffer_len = self.read_buffer.len(),
                "StdStream::poll_read Ready"
            );
            return Poll::Ready(Ok(()));
        }

        info!(
                read_buffer_len = ?self.read_buffer.len(),
                read_buffer_ptr = self.read_buffer.as_ptr() as usize,
                "Reader Pending");
        self.wake_when_write(cx);
        return std::task::Poll::Pending;
    }
}

impl StdStream {
    pub async fn split(self) -> (BufferWriter, AsyncBufferReader) {
        let (sender, receiver) = mpsc::channel(1024); // Buffer size for the channel

        let writer = BufferWriter {
            buffer: self.write_buffer,
            sender,
        };

        let reader = AsyncBufferReader {
            buffer: self.read_buffer,
            receiver,
        };

        (writer, reader)
    }
}

// Implement the BufferWriter
pub struct BufferWriter {
    buffer: Vec<u8>,
    sender: mpsc::Sender<Vec<u8>>,
}

impl std::io::Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sender.try_send(self.buffer.clone()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to send data to reader")
        })?;

        self.buffer.clear();

        Ok(())
    }
}
#[derive(Debug)]
pub struct AsyncBufferReader {
    buffer: Vec<u8>,
    receiver: mpsc::Receiver<Vec<u8>>,
}

impl tokio::io::AsyncRead for AsyncBufferReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<tokio::io::Result<()>> {
        // Poll the receiver for any available data
        match Pin::new(&mut self.receiver).poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                self.buffer.extend_from_slice(&data);
            }
            _ => {}
        };

        if self.buffer.is_empty() {
            return Poll::Pending;
        } else {
            let len = std::cmp::min(buf.remaining(), self.buffer.len());
            buf.put_slice(&self.buffer[..len]);
            self.buffer.drain(..len);
            return Poll::Ready(Ok(()));
        }
    }
}

impl AsyncBufferReader {
    pub async fn from_file(file: tokio::fs::File) -> std::io::Result<Self> {
        let mut buffer = Vec::new();
        let mut file = tokio::io::BufReader::new(file);
        file.read_to_end(&mut buffer).await?;
        Ok(Self {
            buffer,
            receiver: mpsc::channel(1024).1,
        })
    }
}
