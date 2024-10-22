use std::{
    fs::{self},
    io::Seek,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Context;
use tracing::{debug, error, info, span};
use webm_iterable::{
    errors::TagIteratorError,
    matroska_spec::{Master, MatroskaSpec},
    WebmIterator, WebmWriter, WriteOptions,
};

use crate::{
    jrec::streaming::{
        blocking::StdStreamingFile,
        std_stream::{AsyncBufferReader, BufferWriter, StdStream},
    },
    utils,
};

use super::TimedTagWriter;

// Because of the nature of the webm_iterable crate, we need to do everything synchronously
#[derive(Clone)]
pub struct StreamParser {
    output_writer: Arc<Mutex<Vec<TimedTagWriter<BufferWriter>>>>,
    header: Arc<Vec<MatroskaSpec>>, // readonly
    source_file_absolute_path: PathBuf,
    stop_signal: Arc<std::sync::atomic::AtomicBool>,
}

impl PartialEq for StreamParser {
    fn eq(&self, other: &Self) -> bool {
        self.source_file_absolute_path == other.source_file_absolute_path
    }
}

impl StreamParser {
    pub async fn new(source_file_path: &Path) -> anyhow::Result<Self> {
        let source_file = StdStreamingFile::open_read(source_file_path.to_path_buf())?;

        let (tag_itr, header) =
            tokio::task::spawn_blocking(move || Self::init_read(source_file)).await??;

        let output_writer = Arc::new(Mutex::<Vec<TimedTagWriter<BufferWriter>>>::new(vec![]));
        let writer_clone = output_writer.clone();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_signal = stop.clone();

        // we assume that we have already jumped to the first cluster (i.e tag_itr.next() will return Cluster(Master::Start))
        tokio::task::spawn_blocking(move || {
            let res = {
                let span = span!(tracing::Level::INFO, "StreamParser Writer Loop");
                let _enter: span::Entered<'_> = span.enter();
                let mut last_successful_read_position = tag_itr.last_emitted_tag_offset(); // This will give the postion of the first cluster start tag
                let mut tag_itr_holder = Some(tag_itr); // for passing the tag_itr to the next iteration

                let mut cluster_positions = vec![];
                'main: while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    cluster_positions.push(last_successful_read_position);
                    info!(last_successful_read_position, "starting loop");
                    // This should give us the iterator that starts from the next cluster
                    let mut tag_itr = Self::reseek(
                        last_successful_read_position,
                        tag_itr_holder.take().unwrap(),
                        Read,
                    )?;
                    let next = tag_itr.next();
                    // assert next is Some(Ok(Cluster(Master::Start)))
                    let next = next.with_context(|| "Failed to read next tag")??;
                    let next_tag_name = utils::mastroka::mastroka_spec_name(&next);
                    info!(next_tag_name, "Next tag");
                    if !matches!(next, MatroskaSpec::Cluster(Master::Start)) {
                        error!("Expected Cluster(Master::Start) but got {:?}", next);
                        break 'main;
                    }

                    // Cluster interior should consist of all the tags inside the cluster
                    // From Cluster(Master::Start) to Cluster(Master::End) and everything in between
                    let mut cluster_inteior = vec![];
                    // we push Cluster(Master::Start) to the cluster interior
                    cluster_inteior.push(next);

                    'cluster: while let Some(tag) = tag_itr.next() {
                        let tag = match tag {
                            Ok(tag) => tag,
                            Err(e) => {
                                // ok, if parse failed here, we should just continue,
                                // the last_successful_read_position will give the position of current failed cluster
                                if let TagIteratorError::UnexpectedEOF { .. } = e {
                                    // we should not be sleeping here,
                                    // we should have some sort of signal to tell us that the file is
                                    // 1. still being written to
                                    // 2. the file is not being written to anymore
                                    std::thread::sleep(std::time::Duration::from_secs(1));
                                    continue;
                                }
                                error!(?e, "Failed to read tag, skipping");
                                break 'main;
                            }
                        };
                        let is_end = matches!(tag, MatroskaSpec::Cluster(Master::End));
                        cluster_inteior.push(tag);
                        if is_end {
                            let the_next_cluster_start = tag_itr.next();
                            // assert the_next_cluster_start is Some(Ok(Cluster(Master::Start)))
                            if let Some(Ok(MatroskaSpec::Cluster(Master::Start))) =
                                the_next_cluster_start
                            {
                                // last_successful_read_position will give the position that the iterator read, but we constantly reseeking it, so it become relative
                                // to the last successful read position, so we need to add the last_successful_read_position to the last cluster position to
                                // get the absolute position
                                // we have reached the next cluster, which is what we want
                                last_successful_read_position = tag_itr.last_emitted_tag_offset()
                                    + cluster_positions.last().unwrap();
                            } else {
                                // posiblity one: we have reached the end of the file
                                // posiblity two: it just happens that the next cluster is not appended yet
                                todo!("no next cluster found yet");
                            }

                            break 'cluster;
                        }
                    }
                    let writers = writer_clone.lock().expect("trying to get writer");

                    for writer in writers.iter() {
                        for tag in cluster_inteior.iter() {
                            writer.write(tag)?;
                        }
                    }

                    tag_itr_holder = Some(tag_itr);
                }

                Ok::<(), anyhow::Error>(())
            };

            if let Err(ref e) = res {
                error!("Error in the writer loop: {:?}", e);
            };

            res
        });

        Ok(StreamParser {
            output_writer,
            header: Arc::new(header),
            source_file_absolute_path: fs::canonicalize(source_file_path)?,
            stop_signal,
        })
    }

    #[tracing::instrument(skip(self), level = "trace")]
    pub async fn spawn(&self) -> anyhow::Result<AsyncBufferReader> {
        let header = self.header.clone();
        info!(header_len = ?header.len(), "Spawning stream");
        let stream = StdStream::new();
        let (write, read) = stream.split().await;
        let writer = tokio::task::spawn_blocking(move || {
            let timed_writter = TimedTagWriter::new(write);

            for tag in header.iter() {
                timed_writter.write(tag).inspect_err(|e| {
                    let tag_name = utils::mastroka::mastroka_spec_name(tag);
                    error!(tag_name, "Error in writing the tag: {:?}", e);
                })?;
            }

            Ok::<_, anyhow::Error>(timed_writter)
        })
        .await??;

        self.output_writer.lock().expect("wont happen").push(writer);

        Ok(read)
    }

    pub fn reseek(
        postion: usize,
        tag_itr: WebmIterator<StdStreamingFile>,
        read_or_write: impl ReadOrWrite,
    ) -> anyhow::Result<WebmIterator<StdStreamingFile>> {
        let mut inner = tag_itr.into_inner();
        if read_or_write.is_read() {
            inner.reopen_read()?;
        } else {
            inner.reopen_write()?;
        }
        inner
            .seek(std::io::SeekFrom::Start(postion as u64))
            .context("Failed to seek")?;
        Ok(WebmIterator::new(inner, &[]))
    }

    pub fn stop(&self) {
        self.output_writer
            .lock()
            .expect("trying to clear output_writter")
            .clear();
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn init_read(
        streaming_file: StdStreamingFile,
    ) -> anyhow::Result<(WebmIterator<StdStreamingFile>, Vec<MatroskaSpec>)> {
        let mut tag_iterator = WebmIterator::new(
            streaming_file,
            &[
                MatroskaSpec::Ebml(Master::Start),
                MatroskaSpec::Info(Master::Start),
                MatroskaSpec::Tracks(Master::Start),
            ],
        );

        let mut header_tags = vec![];

        for tag in tag_iterator.by_ref() {
            let tag = tag.with_context(|| "Failed to read tag")?;

            match tag {
                MatroskaSpec::Ebml(_)
                | MatroskaSpec::Info(_)
                | MatroskaSpec::Segment(_)
                | MatroskaSpec::Tracks(_) => {
                    header_tags.push(tag);
                }
                MatroskaSpec::Cluster(data) => {
                    if matches!(data, Master::Full(_)) || matches!(data, Master::End) {
                        panic!("Cluster should not be full or end");
                    }
                    // break when we reach the first Cluster(Master::Start)
                    break;
                }
                _ => {}
            }
        }

        info!(header_tags = ?header_tags, "Read header tags");

        Ok((tag_iterator, header_tags))
    }
}

pub fn write_tag(
    writer: &mut WebmWriter<impl std::io::Write>,
    tag: &MatroskaSpec,
) -> anyhow::Result<()> {
    let tag_name = utils::mastroka::mastroka_spec_name(tag);
    if matches!(tag, MatroskaSpec::Segment(Master::Start)) {
        writer.write_advanced(tag, WriteOptions::is_unknown_sized_element())
    } else {
        writer.write(tag)
    }
    .inspect_err(|e| {
        error!(tag_name, "Error in writing the tag: {:?}", e);
    })?;

    Ok(())
}
pub trait ReadOrWrite {
    fn is_read(&self) -> bool;
}

pub struct Read;

impl ReadOrWrite for Read {
    fn is_read(&self) -> bool {
        true
    }
}

pub struct Write;

impl ReadOrWrite for Write {
    fn is_read(&self) -> bool {
        false
    }
}
