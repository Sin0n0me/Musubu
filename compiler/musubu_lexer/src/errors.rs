use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum TokenizeError {
    NotSymbol,
    InvalidCharacters { c: char, position: usize },
    UnusableWhitespace { c: char, position: usize },
}

impl Error for TokenizeError {}

impl Display for TokenizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizeError::InvalidCharacters { c, position: _ } => {
                write!(f, "Invalid characters were used: {c}")
            }
            TokenizeError::UnusableWhitespace { c, position: _ } => {
                write!(f, "Unusable whitespace: {c}")
            }
            TokenizeError::NotSymbol => write!(f, "Not a symbol"),
        }
    }
}
