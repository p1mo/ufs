use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use crate::{Entry, Error};

/// Stop a walk that is nested far deeper than real asset trees.
const MAX_DEPTH: usize = 64;
/// Stop a walk that would otherwise try to index an entire disk.
const MAX_FILES: usize = 100_000;

/// A directory that is walked when the program runs, not at compile time.
#[derive(Debug, Clone)]
pub struct Statics {
    path: PathBuf,
}

impl Statics {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Directory passed to [`Statics::new`], before canonicalization.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lazy walk. Yields files only; directories are not entries.
    ///
    /// Symlinks are skipped so a link cannot pull the walk outside `path`.
    /// The root itself must already be a real directory, not a symlink.
    /// Each item is a [`Result`] so a single unreadable file does not panic
    /// and is not silently dropped.
    pub fn walk(&self) -> Result<Walker, Error> {
        let meta = std::fs::symlink_metadata(&self.path)?;
        if meta.file_type().is_symlink() {
            return Err(Error::InvalidPath(format!(
                "refusing to follow symlink {}",
                self.path.display()
            )));
        }
        if !meta.is_dir() {
            return Err(Error::NotADirectory(self.path.clone()));
        }
        let root = self.path.canonicalize()?;
        let reader = std::fs::read_dir(&root)?;
        Ok(Walker {
            root,
            stack: vec![reader],
            yielded: 0,
        })
    }

    /// Collect the walk and sort by absolute path.
    pub fn files(&self) -> Result<Vec<Entry>, Error> {
        let mut files = self.walk()?.collect::<Result<Vec<_>, _>>()?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }
}

/// Depth-first iterator produced by [`Statics::walk`].
pub struct Walker {
    root: PathBuf,
    stack: Vec<ReadDir>,
    yielded: usize,
}

impl Iterator for Walker {
    type Item = Result<Entry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let reader = self.stack.last_mut()?;
            match reader.next() {
                None => {
                    self.stack.pop();
                }
                Some(Err(err)) => return Some(Err(err.into())),
                Some(Ok(dir_entry)) => {
                    let file_type = match dir_entry.file_type() {
                        Ok(file_type) => file_type,
                        Err(err) => return Some(Err(err.into())),
                    };
                    // `DirEntry::file_type` does not follow symlinks.
                    if file_type.is_symlink() {
                        continue;
                    }
                    let path = dir_entry.path();
                    if file_type.is_dir() {
                        if self.stack.len() >= MAX_DEPTH {
                            return Some(Err(Error::Limit(
                                "directory nests deeper than 64 levels",
                            )));
                        }
                        match std::fs::read_dir(&path) {
                            Ok(child) => self.stack.push(child),
                            Err(err) => return Some(Err(err.into())),
                        }
                        continue;
                    }
                    if !file_type.is_file() {
                        continue;
                    }
                    if !path.starts_with(&self.root) {
                        return Some(Err(Error::InvalidPath(format!(
                            "{} is outside {}",
                            path.display(),
                            self.root.display()
                        ))));
                    }
                    if self.yielded >= MAX_FILES {
                        return Some(Err(Error::Limit("directory has more than 100000 files")));
                    }
                    let rel = match path.strip_prefix(&self.root) {
                        Ok(rel) => rel,
                        Err(_) => {
                            return Some(Err(Error::InvalidPath(path.display().to_string())));
                        }
                    };
                    let depth = rel.components().count().saturating_sub(1);
                    let size = match dir_entry.metadata() {
                        Ok(meta) => meta.len(),
                        Err(err) => return Some(Err(err.into())),
                    };
                    self.yielded += 1;
                    return Some(Ok(Entry::local(path, depth, size)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ufs-walk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn walks_nested_files_and_skips_symlinks() {
        let root = scratch();
        std::fs::write(root.join("a.txt"), b"alpha").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("b.txt"), b"beta").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("a.txt", root.join("link.txt")).unwrap();

        let files = Statics::new(&root).files().unwrap();
        let names: Vec<_> = files.iter().map(|entry| entry.filename.as_str()).collect();
        assert_eq!(names, ["a.txt", "b.txt"]);
        assert_eq!(files[0].depth, 0);
        assert_eq!(files[1].depth, 1);
        assert_eq!(files[0].content().unwrap().as_ref(), b"alpha");
        assert_eq!(files[1].content().unwrap().as_ref(), b"beta");
        assert!(files[0].metadata().is_ok());
        assert!(files[0].is_absolute());

        assert!(Statics::new(root.join("a.txt")).files().is_err());
        assert!(Statics::new(root.join("missing")).walk().is_err());

        #[cfg(unix)]
        {
            let link = root.join("dirlink");
            std::os::unix::fs::symlink(&root, &link).unwrap();
            assert!(Statics::new(&link).walk().is_err());
        }

        let _ = std::fs::remove_dir_all(&root);
    }
}
