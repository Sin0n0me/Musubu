use std::num::{ParseFloatError, ParseIntError};

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    TokenStreamError(TokenStreamParseError),

    UnexpectedEof,
    UnexpectedAST,
    UnexpectedOperator,
    Recursed,
    NotMatch,
    FloatErr(ParseFloatError),
    IntErr(ParseIntError),
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenStreamParseError {
    UnexpectedEof,
    NotKeyword,
    InvalidNumber,
    InvalidDecDigit,
    InvalidFloat,
    InvalidFloatExponent,
    InvalidInteger,
    InvalidOperator,
    InvalidIdentifier,
}

impl From<ParseIntError> for ParseError {
    fn from(value: ParseIntError) -> Self {
        ParseError::IntErr(value)
    }
}

impl From<ParseFloatError> for ParseError {
    fn from(value: ParseFloatError) -> Self {
        ParseError::FloatErr(value)
    }
}
