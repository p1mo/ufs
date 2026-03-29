pub type Result<T> = std::result::Result<T, Error>;

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

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error("unknown error: {0}")]
    Unknown(String),

    #[error("Parse error")]
    Parse(#[from] std::num::ParseIntError),

}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}