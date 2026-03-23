use crate::errors::TokenizeError;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token<'a> {
    pub token_kind: TokenKind<'a>,
    pub token_pos: usize,
}

impl<'a> Token<'a> {
    pub fn get_operator(&'a self) -> Option<&'a Symbol> {
        let TokenKind::Symbol(symbol) = &self.token_kind else {
            return None;
        };

        Some(symbol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind<'a> {
    Identifier(&'a str),
    Number(&'a str),
    Symbol(Symbol),
    LineBreak(Vec<LineBreak>),
    WhiteSpace(Vec<Space>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LineBreak {
    CR,
    LF,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Space {
    Space,
    Tab,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    LeftParenthesis,  // (
    RightParenthesis, // )
    LeftBrackets,     // [
    RightBrackets,    // ]
    LeftBrace,        // {
    RightBrace,       // }

    Plus,            // +
    Minus,           // -
    Star,            // *
    Slash,           // /
    Percent,         // %
    Equal,           // =
    Caret,           // ^
    Not,             // !
    And,             // &
    Or,              // |
    GreaterThan,     //  >
    LessThan,        // <
    At,              // @
    Dot,             // .
    Comma,           // ,
    Colon,           // :
    Semicolon,       // ;
    Underscore,      // _
    Pound,           // #
    Dollar,          // $
    Question,        // ?
    Tilde,           // ~
    SingleQuotation, // '
    DoubleQuotation, // "
    BackSlash,       // \
    Backtick,        // `
}

impl TryFrom<char> for Symbol {
    type Error = TokenizeError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        let symbol = match value {
            '+' => Symbol::Plus,
            '-' => Symbol::Minus,
            '*' => Symbol::Star,
            '/' => Symbol::Slash,
            '%' => Symbol::Percent,
            '=' => Symbol::Equal,
            '^' => Symbol::Caret,
            '!' => Symbol::Not,
            '&' => Symbol::And,
            '|' => Symbol::Or,
            '>' => Symbol::GreaterThan,
            '<' => Symbol::LessThan,
            '@' => Symbol::At,
            '.' => Symbol::Dot,
            ',' => Symbol::Comma,
            ':' => Symbol::Colon,
            ';' => Symbol::Semicolon,
            '#' => Symbol::Pound,
            '$' => Symbol::Dollar,
            '?' => Symbol::Question,
            '~' => Symbol::Tilde,
            '(' => Symbol::LeftParenthesis,
            ')' => Symbol::RightParenthesis,
            '[' => Symbol::LeftBrackets,
            ']' => Symbol::RightBrackets,
            '{' => Symbol::LeftBrace,
            '}' => Symbol::RightBrace,
            '\'' => Symbol::SingleQuotation,
            '"' => Symbol::DoubleQuotation,
            '\\' => Symbol::BackSlash,
            '_' => Symbol::Underscore,
            '`' => Symbol::Backtick,
            _ => return Err(TokenizeError::NotSymbol),
        };

        Ok(symbol)
    }
}

impl FromStr for Symbol {
    type Err = TokenizeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some(c) = s.chars().next() else {
            return Err(TokenizeError::NotSymbol);
        };

        Self::try_from(c)
    }
}
