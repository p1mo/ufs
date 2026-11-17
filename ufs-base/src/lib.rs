

#[cfg(archives)]
mod archives;

#[cfg(archives)]
pub use archives::*;

mod embeds;
pub use embeds::*;

mod statics;
pub use statics::*;

mod entry;
pub use entry::*;






pub enum Bind {
    /// embed an dir
    Embeds(Embeds),
    /// read local filesystem
    Statics(Statics),
}

impl Bind {
    
    pub fn embeds(&self) -> Option<&Embeds> {
        match self {
            Bind::Embeds(embeds) => Some(embeds.into_iter()),
            Bind::Statics(_) => None,
        }
    }
    
    pub fn statics(&self) -> Option<Walker> {
        match self {
            Bind::Statics(statics) => statics.walk().ok(),
            Bind::Embeds(_) => None,
        }
    }

}





pub struct UFS<T>
where 
    T: PartialEq + Send + Sync
{
    entrys: Vec<(T, Bind)>
}

impl<T: PartialEq + Send + Sync> UFS<T> {

    pub fn new() -> Self {
        Self {
            entrys: Vec::new()
        }
    }
    
    pub fn add(&mut self, entry_key: T, bind: Bind) -> Result<(), Error> {
        if let Some(_) = self.entrys.iter().find(|(key, _)| *key == entry_key) {
            return Err(Error::KeyExists);
        }
        Ok(self.entrys.push((entry_key, bind)))
    }
    
    pub fn get(&mut self, entry_key: T) -> Result<&Bind, Error> {
        self.entrys.iter().find(|i| i.0 == entry_key).map(|(_, bind)| bind).ok_or(Error::KeyNotFound)
    }

}











//
// Errors
//

#[derive(Debug, thiserror::Error)]
pub enum Error {

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    StripPrefixError(#[from] std::path::StripPrefixError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[cfg(zip)]
    #[error(transparent)]
    ZipError(#[from] zip::result::ZipError),

    #[cfg(sevenz)]
    #[error(transparent)]
    SevenzError(#[from] sevenz_rust2::Error),

    //#[error(transparent)]
    //ReqwestError(#[from] reqwest::Error),

    #[error("unknown error: {0}")]
    Unknown(String),

    #[error("Parse error")]
    Parse(#[from] std::num::ParseIntError),

    #[error("key already exists.")]
    KeyExists,
    #[error("key not found.")]
    KeyNotFound,
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}