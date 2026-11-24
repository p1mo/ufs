use std::borrow::Cow;
use std::path::Path;

use crate::{Entry, Error, Statics};

/// Files from `bind_dir!`, a disk walk, one embedded buffer, or an archive.
///
/// Every constructor returns this type. Disk walks and archives are [`Result`]s
/// because they can fail; the others are infallible.
#[derive(Debug, Clone, Default)]
pub struct UFS {
    entries: Vec<Entry>,
}

impl UFS {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Build a collection. Order is preserved; `bind_dir!` sorts by path.
    pub fn from_entries(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    /// One `'static` file. It is not expanded, even if the bytes are an archive.
    pub fn embedded(path: &str, bytes: &'static [u8]) -> Self {
        Self::from_entries(vec![Entry::embedded(path, bytes)])
    }

    /// Same as [`UFS::embedded`].
    pub fn entry(path: &str, bytes: &'static [u8]) -> Self {
        Self::embedded(path, bytes)
    }

    /// One owned buffer. Same entry interface as [`UFS::embedded`].
    pub fn bytes(path: &str, bytes: Vec<u8>) -> Self {
        Self::from_entries(vec![Entry::bytes(path, bytes)])
    }

    /// The archive file plus the files inside it.
    ///
    /// Members use `{path}/{path inside}`. Nested archives are expanded.
    /// `content()` on every entry, including members, needs no extra step.
    pub fn archive(path: &str, bytes: &'static [u8]) -> Result<Self, Error> {
        let entry = Entry::embedded(path, bytes);
        if !entry.is_archive() {
            return Err(Error::Archive(format!("{path} is not an archive")));
        }
        let mut files = Self::from_entries(vec![entry]);
        files.expand_archives(true)?;
        Ok(files)
    }

    /// Walk `path` in parallel and return the files, sorted by absolute path.
    ///
    /// Symlinks are skipped. The root must be a real directory. Archive files
    /// stay as single entries; call [`UFS::expand_archives`] to open them.
    ///
    /// `static` is a keyword, so this is `statics`. [`UFS::r#static`] is the same.
    pub fn statics(path: impl AsRef<Path>) -> Result<Self, Error> {
        Statics::new(path).load()
    }

    /// [`UFS::statics`], spelled with a raw identifier.
    pub fn r#static(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::statics(path)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Entry> {
        self.entries.iter()
    }

    /// Find an entry by its stored path.
    ///
    /// Embedded paths use `/` and compare equal. Archive members use the full
    /// `{archive}/{inside}` path.
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = path.as_ref();
        self.entries.iter().find(|entry| entry.path == path)
    }

    /// Move `other`'s files onto the end of this collection.
    pub fn append(&mut self, mut other: UFS) {
        self.entries.append(&mut other.entries);
    }

    /// Read every file, in order.
    ///
    /// Embedded and owned bytes are borrowed. Disk reads and decompression run
    /// in parallel, up to one worker per CPU, and only those results allocate.
    pub fn read_all(&self) -> Result<Vec<Cow<'_, [u8]>>, Error> {
        let heavy: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_local() || entry.is_compressed())
            .map(|(index, _)| index)
            .collect();
        if heavy.is_empty() {
            return self.entries.iter().map(Entry::content).collect();
        }
        let loaded = load_heavy(&self.entries, &heavy)?;
        let mut loaded = loaded.into_iter();
        let mut out = Vec::with_capacity(self.entries.len());
        for (index, entry) in self.entries.iter().enumerate() {
            if heavy.binary_search(&index).is_ok() {
                out.push(Cow::Owned(loaded.next().expect("heavy read")));
            } else {
                out.push(entry.content()?);
            }
        }
        Ok(out)
    }

    /// Open every archive entry in parallel.
    ///
    /// When `keep` is set, the archive file stays in front of its members.
    /// Other files are left where they are.
    pub fn expand_archives(&mut self, keep: bool) -> Result<(), Error> {
        let opened = parallel_map(&self.entries, |entry| {
            if entry.is_archive() {
                Ok(Some(entry.archive()?))
            } else {
                Ok(None)
            }
        })?;
        let mut entries = Vec::with_capacity(self.entries.len());
        for (entry, files) in self.entries.drain(..).zip(opened) {
            match files {
                Some(files) => {
                    if keep {
                        entries.push(entry);
                    }
                    entries.extend(files);
                }
                None => entries.push(entry),
            }
        }
        self.entries = entries;
        Ok(())
    }
}

fn load_heavy(entries: &[Entry], heavy: &[usize]) -> Result<Vec<Vec<u8>>, Error> {
    if heavy.len() <= 1 {
        return heavy
            .iter()
            .map(|&index| Ok(entries[index].content()?.into_owned()))
            .collect();
    }
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, heavy.len());
    let chunk = heavy.len().div_ceil(workers);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for start in (0..heavy.len()).step_by(chunk) {
            let end = (start + chunk).min(heavy.len());
            let indexes = &heavy[start..end];
            handles.push(scope.spawn(move || {
                indexes
                    .iter()
                    .map(|&index| Ok(entries[index].content()?.into_owned()))
                    .collect::<Result<Vec<_>, Error>>()
            }));
        }
        let mut out = Vec::with_capacity(heavy.len());
        for handle in handles {
            out.extend(handle.join().expect("read panicked")?);
        }
        Ok(out)
    })
}

fn parallel_map<T, R>(
    items: &[T],
    map: impl Fn(&T) -> Result<R, Error> + Sync + Send,
) -> Result<Vec<R>, Error>
where
    T: Sync,
    R: Send,
{
    let n = items.len();
    if n <= 1 {
        return items.iter().map(&map).collect();
    }
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, n);
    let chunk = n.div_ceil(workers);
    let map = &map;
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for start in (0..n).step_by(chunk) {
            let end = (start + chunk).min(n);
            let slice = &items[start..end];
            handles.push(scope.spawn(move || slice.iter().map(map).collect::<Result<Vec<_>, _>>()));
        }
        let mut out = Vec::with_capacity(n);
        for handle in handles {
            out.extend(handle.join().expect("worker panicked")?);
        }
        Ok(out)
    })
}

impl<'a> IntoIterator for &'a UFS {
    type Item = &'a Entry;
    type IntoIter = std::slice::Iter<'a, Entry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl IntoIterator for UFS {
    type Item = Entry;
    type IntoIter = std::vec::IntoIter<Entry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl Extend<Entry> for UFS {
    fn extend<I: IntoIterator<Item = Entry>>(&mut self, iter: I) {
        self.entries.extend(iter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_share_one_type() {
        let note = UFS::embedded("note.txt", b"hello");
        let owned = UFS::bytes("scratch.txt", b"owned".to_vec());
        assert_eq!(
            note.get("note.txt").unwrap().content().unwrap().as_ref(),
            b"hello"
        );
        assert_eq!(
            owned
                .get("scratch.txt")
                .unwrap()
                .content()
                .unwrap()
                .as_ref(),
            b"owned"
        );

        let pack =
            UFS::archive("pack.zip", include_bytes!("../examples/archives/pack.zip")).unwrap();
        assert!(pack.get("pack.zip").unwrap().is_archive());
        assert_eq!(
            pack.get("pack.zip/hello.txt")
                .unwrap()
                .content()
                .unwrap()
                .as_ref(),
            b"hello-zip"
        );

        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/archives");
        let disk = UFS::statics(root).unwrap();
        assert!(disk.iter().any(|entry| entry.filename == "pack.zip"));
        let again = UFS::r#static(root).unwrap();
        assert_eq!(again.len(), disk.len());

        let mut all = note;
        all.append(owned);
        let bytes = all.read_all().unwrap();
        assert_eq!(bytes[0].as_ref(), b"hello");
        assert_eq!(bytes[1].as_ref(), b"owned");
    }
}
