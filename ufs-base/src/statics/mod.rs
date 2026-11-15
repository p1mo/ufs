use std::path::PathBuf;


use crate::{Entry, Error};


pub struct Statics {
    pub path: PathBuf,
}

impl Statics {

    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf()
        }
    }

    pub fn walk(&self) -> Result<Walker, Error> {
        match std::fs::read_dir(&self.path) {
            Ok(reader) => Ok(Walker { stack: vec![ reader ] }),
            Err(error) => Err(Error::Unknown(error.to_string())),
        }
    }
    
}

impl IntoIterator for Statics {

    type Item = Entry;
    type IntoIter = Walker;

    fn into_iter(self) -> Self::IntoIter {
        self.walk().unwrap()
    }

}



pub struct Walker {
    stack: Vec<std::fs::ReadDir>,
}

impl Iterator for Walker {

    type Item = Entry;

    fn next(&mut self) -> Option<Self::Item> {

        while let Some(rdr) = self.stack.last_mut() {

            match rdr.next() {

                Some(Ok(ref entry)) => {

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

                        return Some(Entry { size, depth, path, filename, content: None });

                    }

                },

                Some(Err(_)) => continue,

                None => {
                    self.stack.pop();
                },

            }

        }

        None

    }

}