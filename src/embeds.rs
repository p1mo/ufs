use std::path::Path;

use crate::Entry;

/// Files embedded by [`crate::bind_dir`].
#[derive(Debug, Clone)]
pub struct Embeds {
    entries: Vec<Entry>,
}

impl Embeds {
    /// Build a collection. Order is preserved; `bind_dir!` sorts by path.
    pub fn from_entries(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Borrowed iterator. It does not consume the collection and can be
    /// started again, unlike the old shared cursor.
    pub fn iter(&self) -> std::slice::Iter<'_, Entry> {
        self.entries.iter()
    }

    /// Find an entry by the relative path stored at embed time (`/` separators).
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = path.as_ref();
        self.entries.iter().find(|entry| entry.path == path)
    }
}

impl<'a> IntoIterator for &'a Embeds {
    type Item = &'a Entry;
    type IntoIter = std::slice::Iter<'a, Entry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl IntoIterator for Embeds {
    type Item = Entry;
    type IntoIter = std::vec::IntoIter<Entry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
