use std::borrow::Cow;
use std::io::Read;
use std::path::PathBuf;

use crate::error::Error;

/// Largest file [`Entry::content`] will read from disk in one call.
pub const MAX_DISK_READ: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
enum Source {
    /// Uncompressed bytes baked into the binary.
    Bytes(&'static [u8]),
    /// ULZ1 payload. `Entry::size` is the uncompressed length.
    Compressed(&'static [u8]),
    /// Read from `Entry::path` when [`Entry::content`] is called.
    Disk,
    /// Discovered at compile time with `read = false`.
    Omitted,
}

/// One file, either embedded at compile time or found on disk.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Uncompressed size in bytes, captured when the entry was discovered.
    pub size: u64,
    /// Number of parent directories in the entry's relative path.
    ///
    /// `demo.txt` is depth `0`. `nested/demo.txt` is depth `1`.
    /// For on-disk entries this is relative to the walk root, not the
    /// absolute path.
    pub depth: usize,
    /// Relative path for embedded entries, absolute path for on-disk entries.
    /// Embedded paths always use `/` separators.
    pub path: PathBuf,
    /// Final path component.
    pub filename: String,
    source: Source,
}

impl Entry {
    /// Embedded, uncompressed file. `size` is `bytes.len()`.
    pub fn embedded(relative: &str, bytes: &'static [u8]) -> Self {
        let (path, filename, depth) = split_relative(relative);
        Self {
            size: bytes.len() as u64,
            depth,
            path,
            filename,
            source: Source::Bytes(bytes),
        }
    }

    /// Embedded ULZ1 payload. `raw_size` is the uncompressed length.
    pub fn compressed(relative: &str, raw_size: u64, bytes: &'static [u8]) -> Self {
        let (path, filename, depth) = split_relative(relative);
        Self {
            size: raw_size,
            depth,
            path,
            filename,
            source: Source::Compressed(bytes),
        }
    }

    /// Path and size only. [`Entry::content`] returns [`Error::MissingContent`].
    pub fn omitted(relative: &str, size: u64) -> Self {
        let (path, filename, depth) = split_relative(relative);
        Self {
            size,
            depth,
            path,
            filename,
            source: Source::Omitted,
        }
    }

    pub(crate) fn local(absolute: PathBuf, depth: usize, size: u64) -> Self {
        let filename = absolute
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            size,
            depth,
            path: absolute,
            filename,
            source: Source::Disk,
        }
    }

    /// `true` when the bytes live on disk and are read by [`Entry::content`].
    pub fn is_local(&self) -> bool {
        matches!(self.source, Source::Disk)
    }

    /// `true` when the embedded payload is ULZ1-compressed.
    pub fn is_compressed(&self) -> bool {
        matches!(self.source, Source::Compressed(_))
    }

    /// `true` when [`Entry::path`] is absolute.
    ///
    /// Embedded paths are relative, so this is `false` for them.
    pub fn is_absolute(&self) -> bool {
        self.path.is_absolute()
    }

    /// Filesystem metadata for an on-disk entry.
    ///
    /// Embedded entries return [`Error::NotOnDisk`] instead of stating a
    /// relative path, which could otherwise match an unrelated local file.
    pub fn metadata(&self) -> Result<std::fs::Metadata, Error> {
        if !self.is_local() {
            return Err(Error::NotOnDisk);
        }
        let meta = std::fs::symlink_metadata(&self.path)?;
        if !meta.file_type().is_file() {
            return Err(Error::InvalidPath(format!(
                "{} is not a regular file",
                self.path.display()
            )));
        }
        Ok(meta)
    }

    /// File bytes.
    ///
    /// Uncompressed embeds borrow the binary (`Cow::Borrowed`). Compressed
    /// embeds and on-disk files return `Cow::Owned`. On-disk reads do not
    /// follow symlinks and refuse files larger than [`MAX_DISK_READ`].
    pub fn content(&self) -> Result<Cow<'static, [u8]>, Error> {
        match self.source {
            Source::Bytes(bytes) => Ok(Cow::Borrowed(bytes)),
            Source::Compressed(bytes) => {
                let out = crate::compress::decompress(bytes).map_err(Error::Decompress)?;
                if out.len() as u64 != self.size {
                    return Err(Error::Decompress("decompressed size does not match entry"));
                }
                Ok(Cow::Owned(out))
            }
            Source::Omitted => Err(Error::MissingContent),
            Source::Disk => Ok(Cow::Owned(self.read_disk()?)),
        }
    }

    fn read_disk(&self) -> Result<Vec<u8>, Error> {
        let meta = std::fs::symlink_metadata(&self.path)?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            return Err(Error::InvalidPath(format!(
                "refusing to follow symlink {}",
                self.path.display()
            )));
        }
        if !kind.is_file() {
            return Err(Error::InvalidPath(format!(
                "{} is not a regular file",
                self.path.display()
            )));
        }
        if meta.len() > MAX_DISK_READ {
            return Err(Error::Limit("file exceeds the 512 MiB read limit"));
        }
        let mut file = open_nofollow(&self.path)?;
        let mut buf = Vec::new();
        file.by_ref().take(meta.len()).read_to_end(&mut buf)?;
        Ok(buf)
    }
}

fn split_relative(relative: &str) -> (PathBuf, String, usize) {
    let relative = relative.trim_matches('/');
    let depth = if relative.is_empty() {
        0
    } else {
        relative.matches('/').count()
    };
    let filename = relative.rsplit('/').next().unwrap_or("").to_string();
    (PathBuf::from(relative), filename, depth)
}

fn open_nofollow(path: &std::path::Path) -> Result<std::fs::File, Error> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    // Linux O_NOFOLLOW. A swapped-in symlink fails the open instead of being read.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000);
    }
    options.open(path).map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_entries_do_not_touch_disk() {
        let entry = Entry::embedded("nested/demo.txt", b"hi");
        assert_eq!(entry.filename, "demo.txt");
        assert_eq!(entry.depth, 1);
        assert_eq!(entry.size, 2);
        assert!(!entry.is_local());
        assert!(matches!(entry.content().unwrap(), Cow::Borrowed(bytes) if bytes == b"hi"));
        assert!(matches!(entry.metadata(), Err(Error::NotOnDisk)));

        let omitted = Entry::omitted("demo.txt", 4);
        assert!(matches!(omitted.content(), Err(Error::MissingContent)));
    }

    #[test]
    fn compressed_roundtrip_and_reject() {
        let raw = b"aaaaaaa-aaaaaaa-aaaaaaa";
        let blob = crate::compress(raw).unwrap();
        let blob: &'static [u8] = Box::leak(blob.into_boxed_slice());
        let entry = Entry::compressed("a.txt", raw.len() as u64, blob);
        assert!(entry.is_compressed());
        assert_eq!(entry.content().unwrap().as_ref(), raw);

        let bad = Entry::compressed("a.txt", 1, b"not-ulz1");
        assert!(bad.content().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn disk_read_refuses_symlink() {
        let dir = std::env::temp_dir().join(format!("ufs-entry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.txt");
        std::fs::write(&real, b"secret").unwrap();
        let link = dir.join("link.txt");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let entry = Entry::local(link, 0, 6);
        assert!(entry.content().is_err());
        assert!(entry.metadata().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
