use std::path::PathBuf;

pub use ufs_macros::bind_dir;

mod errors;
pub use errors::Error;
pub use errors::Result;

#[cfg(archives)]
mod archives;

#[cfg(archives)]
pub use archives::{ ARCHIVE_EXTS, AchiveExt, Archive };

mod entry;
pub use entry::FsEntry;




pub struct UnifiedFS {
    pub entries: Vec<FsEntry>,
}


 
impl Default for UnifiedFS {
    
    fn default() -> Self {
        Self {
            entries: vec![],
        }
    }

}


 
impl<'a> UnifiedFS {

    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
  
    pub fn iter(&self) -> std::slice::Iter<'_, FsEntry> {
        self.entries.iter()
    }

    pub fn walk<P: AsRef<std::path::Path>>(&self, path: P) -> FsWalker {
        if let Ok(reader) = std::fs::read_dir(path.as_ref()) {
            return FsWalker {
                stack: vec![reader],
            };
        }
        FsWalker { stack: vec![] }
    }

}





impl<'a> IntoIterator for &'a UnifiedFS {
    type Item = &'a FsEntry;
    type IntoIter = std::slice::Iter<'a, FsEntry>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}




pub struct FsWalker {
    stack : Vec<std::fs::ReadDir>,
}

impl<'a> Iterator for FsWalker {
    type Item = FsEntry;
    fn next(&mut self) -> Option<FsEntry> {
        while let Some(rdr) = self.stack.last_mut() {
            match rdr.next() {
                Some(Ok(entry)) => {
                    if entry.path().is_dir() {
                        if let Ok(rd) = std::fs::read_dir(entry.path()) {
                            self.stack.push(rd);
                        }
                    } else {
                        let pathbuf = entry.path().to_path_buf();

                        let filename = pathbuf.file_name().map(|s| s.display().to_string()).unwrap();
                        let path = PathBuf::from(pathbuf.display().to_string().replace('\\', "/"));

                        let size = pathbuf.metadata().map(|m| m.len()).unwrap_or(0);
                        let depth = path.display().to_string().split("/").count() - 1;

                        return Some(FsEntry { size, depth, path, filename, content: None });
                    }                  
                },
                Some(Err(_)) => continue,
                None => { self.stack.pop(); },
            }
        }
        None
    }
}