use std::path::PathBuf;


use super::Result;


#[derive(Debug, Clone)]
pub struct FsEntry {
    pub size: u64,
    pub depth: usize,
    pub path: PathBuf,
    pub filename: String,
    pub content: Option<&'static [u8]>,
}

impl FsEntry {
    
    pub fn is_absolute(&self) -> bool {
        self.path.is_absolute()
    }
    
    pub fn metadata(&self) -> Result<std::fs::Metadata> {
        Ok(self.path.metadata()?)
    }

    pub fn is_local(&self) -> bool {
        if self.content.is_none() && self.path.exists() {
            return true;
        }
        false
    }

    pub fn content(&self) -> Result<&'static [u8]> {
        if !self.is_local() {
            return Ok(self.content.unwrap());
        } 
        Ok(Box::leak(std::fs::read(&self.path)?.into_boxed_slice()))
    }

}
