use std::{io, os::windows::fs::OpenOptionsExt};

use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};

use super::StreamFile;

pub struct StdStreamingFile {
    inner: std::fs::File,
    path: std::path::PathBuf,
}

impl StdStreamingFile {
    pub fn open_read(path: std::path::PathBuf) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
            .open(&path)?;

        Ok(StdStreamingFile { inner: file, path })
    }

    pub fn open_write(path: std::path::PathBuf) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
            .open(&path)?;

        Ok(StdStreamingFile { inner: file, path })
    }

    pub fn reopen_read(&mut self) -> io::Result<()> {
        self.inner = Self::open_read(self.path.clone())?.inner;

        Ok(())
    }

    pub fn reopen_write(&mut self) -> io::Result<()> {
        self.inner = Self::open_write(self.path.clone())?.inner;

        Ok(())
    }

    pub fn to_async(self) -> StreamFile {
        StreamFile::from_std(self)
    }

    pub fn destruct(self) -> (std::fs::File, std::path::PathBuf) {
        (self.inner, self.path)
    }
}

impl std::io::Read for StdStreamingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl std::io::Seek for StdStreamingFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl std::io::Write for StdStreamingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
