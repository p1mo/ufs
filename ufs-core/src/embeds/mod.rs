use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;


use crate::Entry;


pub struct Embeds {
    pub index: AtomicUsize,
    pub entries: Vec<Entry>,
}

impl Embeds {

    pub fn new(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            index: AtomicUsize::new(0),
        }
    }
  
    pub fn iter(&self) -> std::slice::Iter<'_, Entry> {
        self.entries.iter()
    }

}

impl<'a> Iterator for &'a Embeds {

    type Item = &'a Entry;

    fn next(&mut self) -> Option<Self::Item> {
        Some(
            self.entries.get(
                self.index.fetch_add(1, Ordering::Relaxed)
            )?
        )
    }

}