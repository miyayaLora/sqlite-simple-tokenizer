use crate::tokenizer::RegisterTokenizerError;
use std::fmt::Debug;

#[derive(Debug)]
pub enum Error {
    RegisterTokenizerFailure(RegisterTokenizerError),
    SimpleQueryInputTypeIncorrect(String),
    Utf8Error(std::str::Utf8Error),
    RusqliteError(rusqlite::Error),
}

impl From<RegisterTokenizerError> for Error {
    fn from(value: RegisterTokenizerError) -> Self {
        Self::RegisterTokenizerFailure(value)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(value: std::str::Utf8Error) -> Self {
        Self::Utf8Error(value)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::RusqliteError(value)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::RegisterTokenizerFailure(err) => std::fmt::Display::fmt(&err, f),
            Error::SimpleQueryInputTypeIncorrect(ty) => {
                write!(f, "input data must be text, got {ty}")
            }
            Error::Utf8Error(err) => std::fmt::Display::fmt(&err, f),
            Error::RusqliteError(err) => std::fmt::Display::fmt(&err, f),
        }
    }
}

impl std::error::Error for Error {}
