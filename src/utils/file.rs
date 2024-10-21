use std::path::Path;

use tokio::{fs::OpenOptions, io};
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};

pub async fn open_read(path: &Path) -> io::Result<tokio::fs::File> {
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
        .open(&path)
        .await?;

    Ok(file)
}

pub async fn open_write(path: &Path) -> io::Result<tokio::fs::File> {
    let file = OpenOptions::new()
        .read(false)
        .write(true)
        .truncate(true)
        .create(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(path)
        .await?;

    Ok(file)
}

pub async fn create_time(file: &tokio::fs::File) -> io::Result<std::time::SystemTime> {
    let metadata = file.metadata().await?;
    let created = metadata.created()?;
    Ok(created)
}
