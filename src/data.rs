#[derive(Debug, Clone)]
pub enum Data {
    Embed(&'static [u8]),
    Local(Vec<u8>),
}

impl Data {
    
    pub fn to_str(&self) -> Option<&str> {
        match &self {
            Data::Embed(items) => std::str::from_utf8(items).ok(),
            Data::Local(items) => std::str::from_utf8(&items).ok(),
        }
    }
    
    pub fn data(&self) -> &[u8] {
        match &self {
            Data::Embed(items) => items,
            Data::Local(items) => items,
        }
    }
    
    pub fn len(&self) -> usize {
        self.data().len()
    }

}