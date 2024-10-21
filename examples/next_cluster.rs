use anyhow::Context;
use webm_iterable::{
    matroska_spec::{Master, MatroskaSpec},
    WebmIterator,
};

pub fn main() -> anyhow::Result<()> {
    let file = std::fs::File::open(".\\recordings\\21_11_41_54.webm").expect("open file");
    let mut tag_iterator = WebmIterator::new(
        file,
        &[
            // MatroskaSpec::Ebml(Master::Start),
            // MatroskaSpec::Info(Master::Start),
            MatroskaSpec::Tracks(Master::Start),
            MatroskaSpec::Cluster(Master::Start),
        ],
    );

    let mut header_tags = vec![];

    while let Some(tag) = tag_iterator.next() {
        let tag = tag.with_context(|| "Failed to read tag")?;
        println!(
            "last_emitted_tag_offset: {:?}; tag is {:?}",
            tag_iterator.last_emitted_tag_offset(),
            tag
        );
        match tag {
            MatroskaSpec::Ebml(_) | MatroskaSpec::Info(_) | MatroskaSpec::Segment(_) => {
                header_tags.push(tag);
            }
            MatroskaSpec::Tracks(ref data) => {
                if matches!(data, Master::End) || matches!(data, Master::Full(_)) {
                    header_tags.push(tag);
                    break;
                } else {
                    header_tags.push(tag);
                }
            }
            _ => {}
        }
    }

    println!("Header tags: {:?}", header_tags);

    let next = tag_iterator.next();
    // println!("Next: {:?}", next);
    println!(
        "last_emitted_tag_offset: {:?}",
        tag_iterator.last_emitted_tag_offset()
    );

    Ok(())
}
