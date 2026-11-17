use crate::{Embeds, Error, Statics};

/// One registered source: compile-time embeds or a runtime directory.
#[derive(Debug)]
pub enum Bind {
    /// Files produced by [`crate::bind_dir`].
    Embeds(Embeds),
    /// A directory walked with [`Statics::walk`](crate::Statics::walk).
    Statics(Statics),
}

impl Bind {
    /// Borrow the embedded collection, if this source is [`Bind::Embeds`].
    pub fn embeds(&self) -> Option<&Embeds> {
        match self {
            Bind::Embeds(embeds) => Some(embeds),
            Bind::Statics(_) => None,
        }
    }

    /// Borrow the runtime directory, if this source is [`Bind::Statics`].
    ///
    /// Call [`Statics::walk`](crate::Statics::walk) or
    /// [`Statics::files`](crate::Statics::files) on the result so IO errors
    /// stay visible. This used to swallow them.
    pub fn statics(&self) -> Option<&Statics> {
        match self {
            Bind::Statics(statics) => Some(statics),
            Bind::Embeds(_) => None,
        }
    }
}

/// Map from a caller-chosen key to a [`Bind`].
///
/// Keys are compared with [`PartialEq`]. The map is a small `Vec`; it is not
/// a concurrent structure. Share it across threads with a lock if you need to.
#[derive(Debug)]
pub struct UFS<T> {
    entries: Vec<(T, Bind)>,
}

impl<T> Default for UFS<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> UFS<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Registered keys, in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|(key, _)| key)
    }
}

impl<T: PartialEq> UFS<T> {
    /// Insert `bind` under `key`.
    ///
    /// Returns [`Error::KeyExists`] when `key` was already inserted. The
    /// previous value is left untouched.
    pub fn add(&mut self, key: T, bind: Bind) -> Result<(), Error> {
        if self.entries.iter().any(|(existing, _)| existing == &key) {
            return Err(Error::KeyExists);
        }
        self.entries.push((key, bind));
        Ok(())
    }

    /// Borrow the source stored under `key`.
    pub fn get(&self, key: &T) -> Result<&Bind, Error> {
        self.entries
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, bind)| bind)
            .ok_or(Error::KeyNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Statics;

    #[test]
    fn keys_are_unique_and_borrowed() {
        let mut files = UFS::new();
        assert!(files.is_empty());
        files
            .add("assets", Bind::Statics(Statics::new("assets")))
            .unwrap();
        assert!(
            files
                .add("assets", Bind::Statics(Statics::new("other")))
                .is_err()
        );
        assert_eq!(files.len(), 1);
        assert!(files.get(&"assets").unwrap().statics().is_some());
        assert!(files.get(&"missing").is_err());
        assert!(files.get(&"assets").unwrap().embeds().is_none());
    }
}
