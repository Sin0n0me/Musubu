use core::error::Error;
use core::fmt::{Display, Formatter, Result};

#[derive(Debug)]
pub enum TokenizeError {
    NotSymbol,
    InvalidCharacters { c: char, position: usize },
    UnusableWhitespace { c: char, position: usize },
}

impl Error for TokenizeError {}

impl Display for TokenizeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
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
