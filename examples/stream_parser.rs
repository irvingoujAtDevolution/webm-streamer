use std::{io::Write, path::Path};

use tokio::io::AsyncReadExt;
use tracing::info;
use webm_streamer::jrec::webm::stream_parser::StreamParser;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_level(true)
        .with_line_number(true)
        .init();

    let span = tracing::span!(tracing::Level::TRACE, "main");
    let _enter = span.enter();

    let path = Path::new("recordings//21_11_41_54.webm");
    let stream_parser = StreamParser::new(path).await?;

    let mut reader = stream_parser.spawn().await?;
    info!("Spawned stream");
    let mut out_file = std::fs::File::create("stream_parser.webm")?;
    info!("Created file");

    tokio::spawn(async move {
        loop {
            let mut buf = [0u8; 1024];
            info!("Reading from stream");
            let read = reader.read(&mut buf).await?;

            if read == 0 {
                break;
            }

            out_file.write_all(&buf[..read])?;

            info!("Read {} bytes", read);
        }

        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}
