use std::borrow::Cow;
use std::io::Read;
use std::path::PathBuf;

use crate::error::Error;
use crate::set::UFS;

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
    /// Bytes owned by this entry. Used for archive members.
    Owned(Vec<u8>),
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

    /// Owned bytes with the same interface as [`Entry::embedded`].
    ///
    /// Use this when the buffer is not `'static`: a user struct, a decoded
    /// archive member, or anything else built at runtime.
    pub fn bytes(relative: &str, bytes: Vec<u8>) -> Self {
        let (path, filename, depth) = split_relative(relative);
        let size = bytes.len() as u64;
        Self {
            size,
            depth,
            path,
            filename,
            source: Source::Owned(bytes),
        }
    }

    pub(crate) fn owned(path: PathBuf, depth: usize, bytes: Vec<u8>) -> Self {
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let size = bytes.len() as u64;
        Self {
            size,
            depth,
            path,
            filename,
            source: Source::Owned(bytes),
        }
    }

    /// `true` when the file name or payload looks like zip, tar, tar.gz, tar.xz, or 7z.
    pub fn is_archive(&self) -> bool {
        if crate::archive::is_archive_name(&self.filename) {
            return true;
        }
        match &self.source {
            Source::Bytes(bytes) => crate::archive::is_archive_bytes(bytes),
            Source::Owned(bytes) => crate::archive::is_archive_bytes(bytes),
            Source::Disk => sniff_disk(&self.path),
            Source::Compressed(_) | Source::Omitted => false,
        }
    }

    /// Read this entry as an archive. Members are ordinary [`Entry`] values in a [`UFS`].
    ///
    /// Paths look like `{archive}/{path inside}`. Nested archives are expanded
    /// up to 8 levels. The archive file itself is not in the returned list;
    /// [`UFS::archive`] keeps it, and so does `archive_entry = true`.
    pub fn archive(&self) -> Result<UFS, Error> {
        if !self.is_archive() {
            return Err(Error::Archive(format!(
                "{} is not an archive",
                self.path.display()
            )));
        }
        let bytes = self.content()?;
        let nodes = crate::archive::expand(bytes.as_ref(), &self.filename, false)
            .map_err(Error::Archive)?;
        let entries = nodes
            .into_iter()
            .map(|node| {
                let path = PathBuf::from(format!("{}/{}", self.path.display(), node.path));
                let depth = self.depth + 1 + node.path.matches('/').count();
                Entry::owned(path, depth, node.bytes)
            })
            .collect();
        Ok(UFS::from_entries(entries))
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
    /// Uncompressed embeds and owned buffers are borrowed (`Cow::Borrowed`).
    /// Nothing is copied and nothing is leaked. Compressed embeds and on-disk
    /// files return `Cow::Owned` because those bytes are produced on the call.
    /// On-disk reads do not follow symlinks and refuse files larger than
    /// [`MAX_DISK_READ`].
    pub fn content(&self) -> Result<Cow<'_, [u8]>, Error> {
        match &self.source {
            Source::Bytes(bytes) => Ok(Cow::Borrowed(bytes)),
            Source::Owned(bytes) => Ok(Cow::Borrowed(bytes)),
            Source::Compressed(bytes) => {
                let declared = crate::compress::payload_len(bytes).map_err(Error::Decompress)?;
                if declared as u64 != self.size {
                    return Err(Error::Decompress("decompressed size does not match entry"));
                }
                let out = crate::compress::decompress(bytes).map_err(Error::Decompress)?;
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
        buf.try_reserve(meta.len() as usize)
            .map_err(|_| Error::Limit("file exceeds the 512 MiB read limit"))?;
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

fn sniff_disk(path: &std::path::Path) -> bool {
    let mut file = match open_nofollow(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buf = [0u8; 262];
    let Ok(read) = file.read(&mut buf) else {
        return false;
    };
    crate::archive::is_archive_bytes(&buf[..read])
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
        static BLOB: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
        let blob = BLOB.get_or_init(|| crate::compress(raw).unwrap());
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
