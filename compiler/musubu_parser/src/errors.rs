use alloc::boxed::Box;
use core::fmt;
use core::num::{ParseFloatError, ParseIntError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    TokenStreamError(TokenStreamParseError),
    Located {
        start: usize,
        end: usize,
        error: Box<ParseError>,
    },
    Expected {
        rule: &'static str,
    },
    Unsupported {
        feature: &'static str,
    },

    UnexpectedEof,
    UnexpectedAST,
    UnexpectedOperator,
    Recursed,
    NotMatch,
    FloatErr(ParseFloatError),
    IntErr(ParseIntError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenStreamParseError {
    Located {
        start: usize,
        end: usize,
        error: Box<TokenStreamParseError>,
    },
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

impl ParseError {
    pub fn range(&self) -> Option<(usize, usize)> {
        match self {
            Self::Located { start, end, .. } => Some((*start, *end)),
            Self::TokenStreamError(TokenStreamParseError::Located { start, end, .. }) => {
                Some((*start, *end))
            }
            _ => None,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Located { error, .. } => write!(f, "{error}"),
            Self::TokenStreamError(error) => write!(f, "{error}"),
            Self::Expected { rule } => {
                write!(f, "unexpected token or end of input; expected {rule}")
            }
            Self::Unsupported { feature } => write!(f, "unsupported feature: {feature}"),
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
            Self::UnexpectedAST | Self::NotMatch => write!(f, "unexpected token in syntax"),
            Self::UnexpectedOperator => write!(f, "unexpected operator"),
            Self::Recursed => write!(f, "invalid recursive syntax"),
            Self::FloatErr(error) => write!(f, "invalid floating-point literal: {error}"),
            Self::IntErr(error) => write!(f, "invalid integer literal: {error}"),
        }
    }
}

impl fmt::Display for TokenStreamParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Located { error, .. } => return write!(f, "{error}"),
            Self::UnexpectedEof => "unexpected end of input",
            Self::NotKeyword => "expected a keyword",
            Self::InvalidNumber => "invalid numeric literal",
            Self::InvalidDecDigit => "expected decimal digits",
            Self::InvalidFloat => "invalid floating-point literal",
            Self::InvalidFloatExponent => "invalid floating-point exponent",
            Self::InvalidInteger => "invalid integer literal",
            Self::InvalidOperator => "invalid or unsupported operator",
            Self::InvalidIdentifier => "invalid identifier",
        };
        f.write_str(message)
    }
}
