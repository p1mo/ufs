use std::collections::VecDeque;
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
    expand_archives: bool,
    archive_entry: bool,
}

impl Statics {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            expand_archives: false,
            archive_entry: true,
        }
    }

    /// When set, zip/tar/tar.gz/tar.xz/7z files are expanded into entries.
    ///
    /// Nested archives are expanded up to 8 levels. The archive file itself is
    /// kept when [`Statics::archive_entry`] is set (the default).
    pub fn archives(mut self, enabled: bool) -> Self {
        self.expand_archives = enabled;
        self
    }

    /// Keep an archive as its own entry in addition to the files inside it.
    ///
    /// Only applies when [`Statics::archives`] is enabled. Defaults to `true`.
    pub fn archive_entry(mut self, keep: bool) -> Self {
        self.archive_entry = keep;
        self
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
            pending: VecDeque::new(),
            expand_archives: self.expand_archives,
            archive_entry: self.archive_entry,
        })
    }

    /// Collect the walk into a [`UFS`](crate::UFS), same as [`UFS::statics`](crate::UFS::statics).
    pub fn load(&self) -> Result<crate::set::UFS, Error> {
        Ok(crate::set::UFS::from_entries(self.files()?))
    }

    /// Collect the walk and sort by absolute path.
    ///
    /// Plain walks read directories in parallel. Archive expansion stays on the
    /// lazy walker so a bad archive is still one [`Result`] item.
    pub fn files(&self) -> Result<Vec<Entry>, Error> {
        let mut files = if self.expand_archives {
            self.walk()?.collect::<Result<Vec<_>, _>>()?
        } else {
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
            collect_parallel(&self.path.canonicalize()?)?
        };
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }
}

fn collect_parallel(root: &Path) -> Result<Vec<Entry>, Error> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            dirs.push(entry.path());
        } else if file_type.is_file() {
            files.push(local_file(root, entry.path())?);
        }
    }
    if dirs.is_empty() {
        return capped(files);
    }
    if dirs.len() == 1 {
        files.extend(collect_sync(root, &dirs[0], 1)?);
        return capped(files);
    }
    let nested = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(dirs.len());
        for dir in &dirs {
            handles.push(scope.spawn(|| collect_sync(root, dir, 1)));
        }
        let mut nested = Vec::with_capacity(dirs.len());
        for handle in handles {
            nested.push(handle.join().expect("directory walk panicked")?);
        }
        Ok::<_, Error>(nested)
    })?;
    for group in nested {
        files.extend(group);
    }
    capped(files)
}

fn collect_sync(root: &Path, dir: &Path, depth: usize) -> Result<Vec<Entry>, Error> {
    if depth > MAX_DEPTH {
        return Err(Error::Limit("directory nests deeper than 64 levels"));
    }
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            dirs.push(entry.path());
        } else if file_type.is_file() {
            files.push(local_file(root, entry.path())?);
        }
    }
    for dir in dirs {
        files.extend(collect_sync(root, &dir, depth + 1)?);
    }
    Ok(files)
}

fn local_file(root: &Path, path: PathBuf) -> Result<Entry, Error> {
    if !path.starts_with(root) {
        return Err(Error::InvalidPath(format!(
            "{} is outside {}",
            path.display(),
            root.display()
        )));
    }
    let rel = path
        .strip_prefix(root)
        .map_err(|_| Error::InvalidPath(path.display().to_string()))?;
    let depth = rel.components().count().saturating_sub(1);
    let meta = std::fs::symlink_metadata(&path)?;
    if !meta.file_type().is_file() {
        return Err(Error::InvalidPath(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    Ok(Entry::local(path, depth, meta.len()))
}

fn capped(files: Vec<Entry>) -> Result<Vec<Entry>, Error> {
    if files.len() > MAX_FILES {
        return Err(Error::Limit("directory has more than 100000 files"));
    }
    Ok(files)
}

/// Depth-first iterator produced by [`Statics::walk`].
pub struct Walker {
    root: PathBuf,
    stack: Vec<ReadDir>,
    yielded: usize,
    pending: VecDeque<Result<Entry, Error>>,
    expand_archives: bool,
    archive_entry: bool,
}

impl Iterator for Walker {
    type Item = Result<Entry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.pending.pop_front() {
            if item.is_ok() {
                self.yielded += 1;
            }
            return Some(item);
        }
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
                    let entry = Entry::local(path, depth, size);
                    if self.expand_archives && entry.is_archive() {
                        match expand_archive(&entry, self.archive_entry) {
                            Ok(members) => {
                                if self.archive_entry {
                                    self.pending.push_back(Ok(entry));
                                }
                                for member in members {
                                    if self.yielded + self.pending.len() >= MAX_FILES {
                                        self.pending.push_back(Err(Error::Limit(
                                            "directory has more than 100000 files",
                                        )));
                                        break;
                                    }
                                    self.pending.push_back(Ok(member));
                                }
                                if self.pending.is_empty() {
                                    continue;
                                }
                                if let Some(item) = self.pending.pop_front() {
                                    if item.is_ok() {
                                        self.yielded += 1;
                                    }
                                    return Some(item);
                                }
                                continue;
                            }
                            Err(err) => return Some(Err(err)),
                        }
                    }
                    self.yielded += 1;
                    return Some(Ok(entry));
                }
            }
        }
    }
}

fn expand_archive(entry: &Entry, archive_entry: bool) -> Result<Vec<Entry>, Error> {
    let bytes = entry.content()?;
    let nodes = crate::archive::expand(bytes.as_ref(), &entry.filename, archive_entry)
        .map_err(Error::Archive)?;
    Ok(nodes
        .into_iter()
        .map(|node| {
            let path = PathBuf::from(format!("{}/{}", entry.path.display(), node.path));
            let depth = entry.depth + 1 + node.path.matches('/').count();
            Entry::owned(path, depth, node.bytes)
        })
        .collect())
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
