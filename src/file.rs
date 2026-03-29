#[derive(Debug, Clone)]
pub struct File {
    pub path: super::path::Path,
    pub content: super::data::Data,
    pub size: u64,
    pub created: u64,
    pub accessed: u64,
    pub modified: u64,
    pub depth: usize,
}