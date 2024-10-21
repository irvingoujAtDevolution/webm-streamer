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
            MatroskaSpec::Ebml(Master::Start),
            MatroskaSpec::Info(Master::Start),
            MatroskaSpec::Tracks(Master::Start),
        ],
    );

    while let Some(tag) = tag_iterator.next() {
        let tag = tag.with_context(|| "Failed to read tag")?;
        println!(
            "last_emitted_tag_offset: {:?}; tag is {:?}",
            tag_iterator.last_emitted_tag_offset(),
            tag
        );
        match tag {
            MatroskaSpec::Ebml(_) | MatroskaSpec::Info(_) | MatroskaSpec::Segment(_) => {}
            MatroskaSpec::Tracks(ref data) => {
                if matches!(data, Master::End) || matches!(data, Master::Full(_)) {
                    break;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
