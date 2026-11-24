use std::borrow::Cow;
use std::path::Path;

use rayon::prelude::*;

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

    /// Parallel iterator over the same entries as [`UFS::iter`].
    ///
    /// Collected results keep index order. `Entry` is `Sync`, so `content()`
    /// and other shared methods are safe to call from the pool.
    pub fn par_iter(&self) -> rayon::slice::Iter<'_, Entry> {
        self.entries.par_iter()
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
    /// on the rayon pool. Only those results allocate.
    pub fn read_all(&self) -> Result<Vec<Cow<'_, [u8]>>, Error> {
        self.par_iter().map(Entry::content).collect()
    }

    /// Open every archive entry in parallel.
    ///
    /// When `keep` is set, the archive file stays in front of its members.
    /// Other files are left where they are.
    pub fn expand_archives(&mut self, keep: bool) -> Result<(), Error> {
        let opened = self
            .par_iter()
            .map(|entry| {
                if entry.is_archive() {
                    Ok(Some(entry.archive()?))
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>, Error>>()?;
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

impl<'a> IntoParallelIterator for &'a UFS {
    type Item = &'a Entry;
    type Iter = rayon::slice::Iter<'a, Entry>;

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

impl IntoParallelIterator for UFS {
    type Item = Entry;
    type Iter = rayon::vec::IntoIter<Entry>;

    fn into_par_iter(self) -> Self::Iter {
        self.entries.into_par_iter()
    }
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
        let names: Vec<_> = all
            .par_iter()
            .map(|entry| entry.filename.as_str())
            .collect();
        assert_eq!(names, ["note.txt", "scratch.txt"]);
        let bytes = all.read_all().unwrap();
        assert_eq!(bytes[0].as_ref(), b"hello");
        assert_eq!(bytes[1].as_ref(), b"owned");
    }
}
