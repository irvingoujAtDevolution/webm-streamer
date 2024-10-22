use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use anyhow::Context;
use atomic_option::AtomicOption;
use tokio::sync::Mutex;
use webm_iterable::{
    errors::TagWriterError,
    matroska_spec::{EbmlSpecification, EbmlTag, Master, MatroskaSpec},
    WebmWriter, WriteOptions,
};

use crate::utils;

pub mod stream_parser;

pub struct TimedTagWriter<T>
where
    T: std::io::Write,
{
    writer: Mutex<webm_iterable::WebmWriter<T>>,
    time_offset: AtomicOption<u64>,
}

impl<T> TimedTagWriter<T>
where
    T: std::io::Write,
{
    pub fn new(writer: T) -> Self {
        Self {
            writer: Mutex::new(WebmWriter::new(writer)),
            time_offset: AtomicOption::empty(),
        }
    }

    pub fn write(&self, tag: &MatroskaSpec) -> anyhow::Result<()> {
        let mut writer = self.writer.blocking_lock();

        // Get the name of the tag for context in case of an error
        let tag_name = utils::mastroka::mastroka_spec_name(tag);

        // Special case for Segment start with unknown size
        if matches!(tag, MatroskaSpec::Segment(Master::Start)) {
            writer
                .write_advanced(tag, WriteOptions::is_unknown_sized_element())
                .with_context(|| {
                    format!(
                        "Error writing unknown-sized Segment start tag: {:?}",
                        tag_name
                    )
                })?;
        } else {
            // Handle the Timestamp tag with offset adjustment
            if let MatroskaSpec::Timestamp(timestamp) = *tag {
                let time_offset = self
                    .time_offset
                    .take(Ordering::SeqCst)
                    .unwrap_or(Box::new(0));

                let adjusted_timestamp = timestamp.saturating_sub(*time_offset);

                let updated_tag = MatroskaSpec::Timestamp(adjusted_timestamp);
                writer.write(&updated_tag).with_context(|| {
                    format!("Error writing adjusted Timestamp tag: {:?}", tag_name)
                })?;

                self.time_offset
                    .try_store(Box::new(*time_offset), Ordering::Relaxed);
            } else {
                // Write other tags as-is
                writer
                    .write(tag)
                    .with_context(|| format!("Error writing tag: {:?}", tag_name))?;
            }
        }

        Ok(())
    }
}

mod debug {
    use std::fs::File;

    use anyhow::Context;
    use tokio::sync::Mutex;
    use webm_iterable::{matroska_spec::MatroskaSpec, WebmWriter};

    use crate::{jrec::streaming::std_stream::StdStream, utils};

    pub struct DebugStream<T>
    where
        T: std::io::Write,
    {
        inner: T,
        file: Mutex<File>,
    }

    impl<T> DebugStream<T>
    where
        T: std::io::Write,
    {
        pub fn new(inner: T) -> Self {
            let file_path = "debug.webm";
            Self {
                inner,
                file: Mutex::new(File::create(file_path).expect("Failed to create debug file")),
            }
        }
    }

    impl<T> std::io::Write for DebugStream<T>
    where
        T: std::io::Write,
    {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.file.blocking_lock().write(buf).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Error writing to debug file: {:?}", e),
                )
            })?;
            self.inner.write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.file.blocking_lock().flush().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Error flushing debug file: {:?}", e),
                )
            })?;
            self.inner.flush()
        }
    }
}
