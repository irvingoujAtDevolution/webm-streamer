use webm_iterable::matroska_spec::{Master, MatroskaSpec};
use webm_streamer::jrec::streaming::std_stream::StdStream;

#[tokio::main]
pub async fn main() {
    // let std_stream = StdStream::new();
    // let std_stream_clone = std_stream.clone();
    // tokio::task::spawn_blocking(move || {
    //     let mut writer = webm_iterable::WebmWriter::new(std_stream_clone);

    //     // Write the Segment start tag
    //     writer
    //         .write(&MatroskaSpec::Segment(Master::Start))
    //         .expect("write segment start");

    //     // Now write the Cluster start tag inside the Segment
    //     writer
    //         .write(&MatroskaSpec::Cluster(Master::Start))
    //         .expect("write cluster start");
    // })
    // .await
    // .expect("spawn_blocking");
}
