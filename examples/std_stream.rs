use anyhow::Context;
use tracing::info;
use webm_iterable::{
    matroska_spec::{Master, MatroskaSpec},
    WebmIterator, WebmWriter,
};
use webm_streamer::jrec::{
    streaming::{blocking::StdStreamingFile, std_stream::StdStream},
    webm::stream_parser::{Read, StreamParser},
};

pub struct InMemoryReader {
    buffer: Vec<u8>,
    position: usize,
}

impl InMemoryReader {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }
}

impl std::io::Read for InMemoryReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_to_copy = std::cmp::min(buf.len(), self.buffer.len() - self.position);
        buf[..bytes_to_copy]
            .copy_from_slice(&self.buffer[self.position..self.position + bytes_to_copy]);
        self.position += bytes_to_copy;
        Ok(bytes_to_copy)
    }
}

pub fn main() -> anyhow::Result<()> {
    let file = StdStreamingFile::open_read(".\\recordings\\21_11_41_54.webm".try_into()?)?;

    let mut tag_iterator = WebmIterator::new(
        file,
        &[
            MatroskaSpec::Ebml(Master::Start),
            MatroskaSpec::Info(Master::Start),
            MatroskaSpec::Tracks(Master::Start),
        ],
    );

    let mut header_tags = vec![];

    for tag in tag_iterator.by_ref() {
        let tag = tag.with_context(|| "Failed to read tag")?;

        println!("Tag: {:?}", tag);
        match tag {
            MatroskaSpec::Ebml(_) | MatroskaSpec::Info(_) | MatroskaSpec::Segment(_) => {
                header_tags.push(tag);
            }
            MatroskaSpec::Tracks(_) => {
                header_tags.push(tag);
            }
            MatroskaSpec::Cluster(data) => {
                info!("Cluster data: {:?}", data);
                break;
            }
            _ => {}
        }
    }

    println!("Header tags: {:?}", header_tags);

    let last_read_position = tag_iterator.last_emitted_tag_offset();

    let mut tag_iterator = StreamParser::reseek(last_read_position, tag_iterator, Read)?;
    let Some(Ok(next)) = tag_iterator.next() else {
        panic!("No next tag found")
    };

    println!("After reseeking, next: {:?}", next);
    println!(
        "last_emitted_tag_offset: {:?}",
        tag_iterator.last_emitted_tag_offset()
    );

    let mut writer = WebmWriter::new(StdStream::new());

    println!("Writing header tags: {:?}", header_tags);

    for tag in header_tags.iter() {
        writer.write(tag)?;
    }

    println!("Writing next tag = {:?}", next);

    writer.write(&next)?;

    let inner = writer.into_inner()?;

    let buf = inner.buffer;
    let buf_clone = buf.blocking_lock().clone();

    let reader = InMemoryReader::new(buf_clone);

    let mut tag_iterator = WebmIterator::new(reader, &[]);

    for tag in tag_iterator.by_ref() {
        let tag = tag.with_context(|| "Failed to read tag")?;

        println!("InMemory Tag: {:?}", tag);
    }

    Ok(())
}
