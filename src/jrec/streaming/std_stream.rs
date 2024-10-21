use std::{
    fmt::Debug,
    future::Future,
    sync::{atomic::AtomicI16, Arc},
};

use tokio::{io::AsyncReadExt, sync::Mutex};
use tracing::debug;

const NAME: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];
const NAME_COUNT: AtomicI16 = AtomicI16::new(0);

#[derive(Clone)]
pub struct StdStream {
    // pub for debugging purposes
    pub buffer: Arc<Mutex<Vec<u8>>>,
    from_file: bool,

    write_signal: Arc<tokio::sync::Notify>,
    // Debug purposes
    id: &'static str,
}

impl Debug for StdStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdStream")
            .field("id", &self.id)
            .field("from_file", &self.from_file)
            .finish()
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
            buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            from_file: false,
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
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // debug!(buf = buf.len(), "StdStream::write");
        let mut buffer = self.buffer.blocking_lock();
        // debug!(buffer = buffer.len(), "StdStream::write locked");
        buffer.extend_from_slice(buf);
        // debug!(buffer = buffer.len(), "StdStream::write unlocked");
        self.wake();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        debug!("StdStream::flush");
        Ok(())
    }
}

impl tokio::io::AsyncRead for StdStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        read_buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Lock the buffer
        let lock_future = self.buffer.lock();
        tracing::debug!(
            id = self.id,
            "StdStream::poll_read, trying to locking buffer"
        );

        // Pin the future
        futures::pin_mut!(lock_future);

        // Poll the future
        let result = match lock_future.poll(cx) {
            std::task::Poll::Ready(mut internal_buffer) => {
                debug!(
                    id = self.id,
                    internal_buffer = internal_buffer.len(),
                    "StdStream::poll_read, buffer locked"
                );
                if internal_buffer.is_empty() {
                    if self.from_file {
                        // EOF
                        std::task::Poll::Ready(Ok(()))
                    } else {
                        // I should pass the context to a notifier
                        self.wake_when_write(cx);
                        std::task::Poll::Pending
                    }
                } else {
                    let len = std::cmp::min(read_buf.remaining(), internal_buffer.len());
                    read_buf.put_slice(&internal_buffer[..len]);
                    internal_buffer.drain(..len);
                    debug!(
                        internal_buffer = internal_buffer.len(),
                        "StdStream::poll_read Ready"
                    );
                    std::task::Poll::Ready(Ok(()))
                }
            }
            std::task::Poll::Pending => {
                debug!("StdStream::poll_read pending");
                std::task::Poll::Pending
            }
        };

        debug!(result = ?result, "StdStream::poll_read releasing lock");
        result
    }
}

impl StdStream {
    pub async fn from_file(file: tokio::fs::File) -> std::io::Result<Self> {
        let mut buffer = Vec::new();
        let mut file = tokio::io::BufReader::new(file);
        file.read_to_end(&mut buffer).await?;
        Ok(Self {
            buffer: Arc::new(Mutex::new(buffer)),
            from_file: true,
            id: NAME[NAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as usize],
            write_signal: Arc::new(tokio::sync::Notify::new()),
        })
    }
}
