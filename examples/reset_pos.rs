use std::io::Seek;

use anyhow::Context;
use tracing::info;
use webm_iterable::{
    matroska_spec::{Master, MatroskaSpec},
    WebmIterator,
};

pub fn main() -> anyhow::Result<()> {
    let file = std::fs::File::open(".\\recordings\\21_11_41_54.webm").expect("open file");
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
    println!("Last read position: {:?}", last_read_position);

    let mut file = std::fs::File::open(".\\recordings\\10_14_44_13.webm").expect("open file");
    file.seek(std::io::SeekFrom::Start(last_read_position as u64))
        .expect("seek");

    let mut tag_iterator = WebmIterator::new(
        file,
        &[
            MatroskaSpec::Ebml(Master::Start),
            MatroskaSpec::Info(Master::Start),
            MatroskaSpec::Tracks(Master::Start),
        ],
    );

    let next = tag_iterator.next();

    if let Some(Ok(MatroskaSpec::Cluster(data))) = next {
        let name = match data {
            Master::Start => "Cluster start",
            Master::End => "Cluster end",
            Master::Full(vec) => "Cluster full",
        };

        info!("Next: {:?}", name);
    }

    Ok(())
}
