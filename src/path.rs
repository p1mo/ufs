use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Path {
    Embed(&'static str),
    Local(PathBuf),
}

impl Path {
    
    pub fn to_str(&self) -> &str {
        match &self {
            Path::Embed(str) => str,
            Path::Local(str) => str.to_str().unwrap(),
        }
    }
                
    pub fn to_path_buf(&self) -> PathBuf {
        std::path::Path::new(self.to_str()).to_path_buf()
    }
                
    pub fn file_name(&self) -> Option<&str> {
        std::path::Path::new(self.to_str()).file_name().and_then(|n| n.to_str())
    }
                
    pub fn ext(&self) -> Option<&str> {
        std::path::Path::new(self.to_str()).extension().and_then(|e| e.to_str())
    }
    
    pub fn depth(&self) -> usize {
        self.to_str().strip_suffix(&self.file_name().unwrap()).unwrap().split("/").collect::<Vec<&str>>().len()
    }

}