pub mod errors;
pub mod token;

use errors::TokenizeError;
use std::iter::{Peekable, from_fn};
use std::str::CharIndices;
use token::{LineBreak, Space, Symbol, Token, TokenKind};

pub type Tokens<'a> = Vec<Token<'a>>;
type Iter<'a> = Peekable<CharIndices<'a>>;

// 受け取った文字列を最小限の単位毎に切り出す
pub fn tokenize<'a>(source_code: &'a str) -> Result<Tokens<'a>, TokenizeError> {
    let mut iter = source_code.char_indices().peekable();
    let mut token_list = vec![];

    while let Some(&(position, c)) = iter.peek() {
        let token = if c.is_ascii_digit() {
            eat_number(source_code, &mut iter)? // 0-9で始まるものは数値として扱う
        } else if c.is_ascii_whitespace() {
            eat_whitespace(&mut iter)?
        } else if c.is_ascii_punctuation() {
            eat_symbol(&mut iter)? // ASCIIの記号
        } else if c.is_alphabetic() {
            eat_identifier(source_code, &mut iter)? // 日本語などを使用するのでasciiに限定しない
        } else if c == '\0' {
            break; // EOF
        } else {
            return Err(TokenizeError::InvalidCharacters { c, position });
        };

        token_list.push(token);
    }

    Ok(token_list)
}

fn eat_identifier<'a>(source_code: &'a str, iter: &mut Iter) -> Result<Token<'a>, TokenizeError> {
    let Some(&(position, _)) = iter.peek() else {
        unreachable!();
    };

    let code = slice_code(source_code, iter, |c| {
        if c.is_ascii_punctuation() {
            return false;
        }

        c.is_alphabetic() || c.is_ascii_digit()
    })?;

    Ok(Token {
        token_kind: TokenKind::Identifier(code),
        token_pos: position,
    })
}

fn eat_number<'a>(source_code: &'a str, iter: &mut Iter) -> Result<Token<'a>, TokenizeError> {
    let Some(&(position, _)) = iter.peek() else {
        unreachable!();
    };

    let code = slice_code(source_code, iter, |c| c.is_ascii_digit())?;

    Ok(Token {
        token_kind: TokenKind::Number(code),
        token_pos: position,
    })
}

fn eat_symbol<'a>(iter: &mut Iter) -> Result<Token<'a>, TokenizeError> {
    let Some(&(position, c)) = iter.peek() else {
        unreachable!();
    };

    let symbol = Symbol::try_from(c)?;

    iter.next();

    Ok(Token {
        token_kind: TokenKind::Symbol(symbol),
        token_pos: position,
    })
}

fn eat_whitespace<'a>(iter: &mut Iter) -> Result<Token<'a>, TokenizeError> {
    let Some(&(position, c)) = iter.peek() else {
        unreachable!();
    };

    match c {
        ' ' | '\t' => eat_space(iter),
        '\r' | '\n' => eat_line_break(iter),
        _ => Err(TokenizeError::UnusableWhitespace { c, position }),
    }
}

fn eat_space<'a>(iter: &mut Iter) -> Result<Token<'a>, TokenizeError> {
    let Some(&(position, _)) = iter.peek() else {
        unreachable!();
    };

    let code = from_fn(|| iter.next_if(|c| matches!(c.1, ' ' | '\t')))
        .map(|c| match c.1 {
            ' ' => Space::Space,
            '\t' => Space::Tab,
            _ => unreachable!(),
        })
        .collect();

    Ok(Token {
        token_kind: TokenKind::WhiteSpace(code),
        token_pos: position,
    })
}

fn eat_line_break<'a>(iter: &mut Iter) -> Result<Token<'a>, TokenizeError> {
    let Some(&(position, _)) = iter.peek() else {
        unreachable!();
    };

    let code = from_fn(|| iter.next_if(|c| matches!(c.1, '\n' | '\r')))
        .map(|c| match c.1 {
            '\r' => LineBreak::CR,
            '\n' => LineBreak::LF,
            _ => unreachable!(),
        })
        .collect();

    Ok(Token {
        token_kind: TokenKind::LineBreak(code),
        token_pos: position,
    })
}

fn slice_code<'a>(
    source_code: &'a str,
    iter: &mut Iter,
    condition: impl Fn(char) -> bool,
) -> Result<&'a str, TokenizeError> {
    let Some(&(start, _)) = iter.peek() else {
        unreachable!();
    };

    let _ = from_fn(|| iter.next_if(|&(_, c)| condition(c))).count(); // count()によりイテレータを消費
    let end = iter.peek().map(|e| e.0).unwrap_or(source_code.len());

    if start == end {
        unreachable!("slice_code was called without a matching character");
    }

    Ok(&source_code[start..end])
}
