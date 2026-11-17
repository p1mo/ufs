use std::fmt;
use std::path::PathBuf;

/// Failure returned by registry, filesystem, and decompression operations.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem call failed.
    Io(std::io::Error),
    /// [`crate::UFS::add`] was called with a key that is already present.
    KeyExists,
    /// [`crate::UFS::get`] was called with a key that was never added.
    KeyNotFound,
    /// [`crate::Entry::content`] was called on an entry embedded with `read = false`.
    MissingContent,
    /// A runtime root exists but is not a directory.
    NotADirectory(PathBuf),
    /// Metadata was requested for an embedded entry, which has no disk path.
    NotOnDisk,
    /// A compressed payload was truncated, oversized, or inconsistent.
    Decompress(&'static str),
    /// A path was a symlink, escaped its root, or was not a regular file.
    InvalidPath(String),
    /// A file-count or file-size safety limit was hit.
    Limit(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{err}"),
            Error::KeyExists => write!(f, "key already exists"),
            Error::KeyNotFound => write!(f, "key not found"),
            Error::MissingContent => {
                write!(f, "file contents were not embedded (read = false)")
            }
            Error::NotADirectory(path) => write!(f, "not a directory: {}", path.display()),
            Error::NotOnDisk => write!(f, "entry is not a filesystem path"),
            Error::Decompress(msg) => write!(f, "decompress failed: {msg}"),
            Error::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Error::Limit(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
