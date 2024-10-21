use std::path::Path;

use tokio::io::AsyncReadExt;
use webm_streamer::jrec::webm::stream_parser::StreamParser;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_level(true)
        .with_line_number(true)
        .init();

    let path = Path::new("recordings//21_11_41_54.webm");
    let stream_parser = StreamParser::new(path).await?;

    let mut stream = stream_parser.spawn().await?;

    loop {
        let mut buf = [0u8; 1024];
        let read = stream.read(&mut buf).await?;

        if read == 0 {
            break;
        }

        println!("Read: {:?}", read);
    }

    Ok(())
}
