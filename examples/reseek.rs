use anyhow::Context;
use tracing::info;
use webm_iterable::{
    matroska_spec::{Master, MatroskaSpec},
    WebmIterator,
};
use webm_streamer::jrec::{
    streaming::blocking::StdStreamingFile,
    webm::stream_parser::{Read, StreamParser},
};

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
    let next = tag_iterator.next();

    println!("After reseeking, next: {:?}", next);
    println!(
        "last_emitted_tag_offset: {:?}",
        tag_iterator.last_emitted_tag_offset()
    );

    Ok(())
}
