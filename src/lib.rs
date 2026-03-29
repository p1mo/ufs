pub use ufs_macros::bind_dir;

mod errors;
pub use errors::Error;
pub use errors::Result;

mod data;
pub use data::Data;

mod file;
pub use file::File;

mod path;
pub use path::Path;










#[derive(Debug, Clone)]
pub struct UnifiedFS {
    pub total: u64,
    pub files: Vec<file::File>,
}



impl UnifiedFS {

    pub fn walk<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let mut s = UnifiedFS { ..Default::default() };
        s.walker(path)?;
        Ok(s)
    }



    pub fn size(&self) -> u64 {
        self.total
    }
                
    pub fn entries(&self) -> &[file::File] {
        &self.files
    }
                
    pub fn count(&self) -> usize {
        self.files.len()
    }
                
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }


                
    pub fn get(&self, path: &str) -> Option<&file::File> {
        self.files.iter().find(|f| f.path.to_str() == path)
    }
                
    pub fn as_str(&self, path: &str) -> Option<&str> {
        self.get(path).and_then(|f| f.content.to_str())
    }
                
    pub fn exists(&self, path: &str) -> bool {
        self.files.iter().any(|f| f.path.to_str() == path)
    }
                
    pub fn from_ext(&self, ext: &str) -> Vec<&file::File> {
        self.files.iter().filter(|f| f.path.ext() == Some(ext)).collect()
    }
                
    pub fn iter(&self) -> std::slice::Iter<'_, file::File> {
        self.files.iter()
    }

}


impl UnifiedFS {
    
    fn walker<P>(&mut self, path: P) -> std::io::Result<()>
    where 
        P: AsRef<std::path::Path>,
    {
        let read_dir = std::fs::read_dir(path)?;

        for dir_entry in read_dir.into_iter().filter_map(|e| e.ok()) {

            if dir_entry.metadata()?.is_dir() {
                self.walker(dir_entry.path())?;
                continue;
            }

            let path = dir_entry.path().display().to_string().replace('\\', "/");

            let efs_path = path::Path::Local(path.into());
            let efs_data = data::Data::Local(std::fs::read(efs_path.to_str())?);
            let size = efs_data.len() as u64;
            let depth = efs_path.depth();

            let meta = efs_path.to_path_buf().metadata()?;

            let epoch = std::time::UNIX_EPOCH;
            
            self.files.push(file::File {
                    path: efs_path,
                    content: efs_data,
                    size: size,
                    created: meta.created()?.duration_since(epoch).unwrap().as_secs(),
                    accessed: meta.accessed()?.duration_since(epoch).unwrap().as_secs(),
                    modified: meta.modified()?.duration_since(epoch).unwrap().as_secs(),
                    depth: depth
            });

            self.total += size;
            
        }
        Ok(())
    }

}



impl Default for UnifiedFS {
    fn default() -> Self {
        Self {
            total: 0,
            files: vec![],
        }
    }
}




impl std::ops::Index<&str> for UnifiedFS {
    type Output = file::File;
    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).expect("file not found")
    }
}




impl<'a> IntoIterator for &'a UnifiedFS {
    type Item = &'a file::File;
    type IntoIter = std::slice::Iter<'a, file::File>;
    fn into_iter(self) -> Self::IntoIter {
        self.files.iter()
    }
}
