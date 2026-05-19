use crate::Rule;
use std::fmt::Debug;
use thiserror::Error;
/// An error that can occur when parsing or evaluating an expr program
#[derive(Error)]
pub enum Error {
    #[error(transparent)]
    PestError(#[from] Box<pest::error::Error<Rule>>),
    #[error("{0}")]
    ParseError(String),
    #[error("{0}")]
    ExprError(String),
    #[error(transparent)]
    RegexError(#[from] regex::Error),
    #[cfg(feature = "serde")]
    #[error("{0}")]
    DeserializeError(String),
    #[cfg(feature = "serde")]
    #[error("{0}")]
    SerializeError(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::ExprError(s)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::PestError(e) => write!(f, "PestError: {}", e),
            Error::ParseError(e) => write!(f, "ParseError: {}", e),
            Error::ExprError(e) => write!(f, "ExprError: {}", e),
            Error::RegexError(e) => write!(f, "RegexError: {}", e),
            #[cfg(feature = "serde")]
            Error::DeserializeError(e) => write!(f, "DeserializeError: {}", e),
            #[cfg(feature = "serde")]
            Error::SerializeError(e) => write!(f, "SerializeError: {}", e),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::ExprError(format!($($arg)*)))
    };
}
