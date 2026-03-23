use crate::{
    errors::TokenStreamParseError,
    lexer::musubu_keywords::MusubuKeyword,
    lexer::token::{FloatLiteral, MusubuLiteral, MusubuOperator, MusubuToken, MusubuTokenKind},
};
use musubu_ast::{AssignOperator, LogicalOperator};
use musubu_lexer::{Tokens, token::*};
use musubu_primitive::*;
use std::{
    iter::{Peekable, from_fn},
    slice::Iter,
    str::FromStr,
};

#[derive(Debug)]
pub(crate) struct TokenStream {
    tokens: Vec<MusubuToken>,
    index: usize,
}

impl TokenStream {
    fn new(tokens: Vec<MusubuToken>) -> Self {
        Self { tokens, index: 0 }
    }

    pub fn set_position(&mut self, position: usize) {
        self.index = position;
    }

    pub fn get_position(&self) -> usize {
        self.index
    }

    pub fn next(&mut self) -> Option<&MusubuToken> {
        let token = self.tokens.get(self.index);
        self.index += 1;
        token
    }

    pub fn get_from_index(&self, index: usize) -> Option<&MusubuToken> {
        self.tokens.get(index)
    }

    pub fn get(&self) -> Option<&MusubuToken> {
        self.tokens.get(self.index)
    }

    pub fn get_keyword(&self) -> Option<&MusubuKeyword> {
        let token = self.get()?;
        let MusubuTokenKind::Keyword(keyword) = &token.token_kind else {
            return None;
        };
        Some(keyword)
    }

    pub fn get_literal(&self) -> Option<&MusubuLiteral> {
        let token = self.get()?;
        let MusubuTokenKind::Literal(literal) = &token.token_kind else {
            return None;
        };
        Some(literal)
    }

    pub fn get_identifier(&self) -> Option<&str> {
        let token = self.get()?;
        let MusubuTokenKind::Identifier(ident) = &token.token_kind else {
            return None;
        };
        Some(ident)
    }

    pub fn get_operator(&self) -> Option<&MusubuOperator> {
        let token = self.get()?;
        let MusubuTokenKind::Operator(op) = &token.token_kind else {
            return None;
        };
        Some(op)
    }

    pub fn get_binary_operator(&self) -> Option<&BinaryOperator> {
        let MusubuOperator::Binary(op) = self.get_operator()? else {
            return None;
        };
        Some(op)
    }

    pub fn get_assign_operator(&self) -> Option<&AssignOperator> {
        let MusubuOperator::Assign(op) = self.get_operator()? else {
            return None;
        };
        Some(op)
    }

    pub fn get_comparison_operator(&self) -> Option<&ComparisonOperator> {
        let MusubuOperator::Comparison(op) = self.get_operator()? else {
            return None;
        };
        Some(op)
    }

    pub fn get_logical_operator(&self) -> Option<&LogicalOperator> {
        let MusubuOperator::Logical(op) = self.get_operator()? else {
            return None;
        };
        Some(op)
    }
}

type ParseIter<'a> = Peekable<Iter<'a, Token<'a>>>;
type ParseResult = Result<MusubuToken, TokenStreamParseError>;

pub(crate) fn tokenize<'a>(tokens: &Tokens<'a>) -> Result<TokenStream, TokenStreamParseError> {
    let mut iter = tokens.iter().peekable();
    let mut new_tokens = Vec::with_capacity(tokens.len() / 2);
    while let Some(token) = iter.peek() {
        let position = token.token_pos;
        let new_token = match &token.token_kind {
            TokenKind::Identifier(ident) => tokenize_identifier(&mut iter, ident, position)?,
            TokenKind::Number(num) => tokenize_number(&mut iter, num, position)?,
            TokenKind::Symbol(symbol) => tokenize_operator(&mut iter, symbol, position)?,
            TokenKind::LineBreak(_) | TokenKind::WhiteSpace(_) => {
                eat_whitespace(&mut iter);
                continue;
            }
        };

        new_tokens.push(new_token);
    }

    Ok(TokenStream::new(new_tokens))
}

// Identifier or Keyword
fn tokenize_identifier(iter: &mut ParseIter, _ident: &str, position: usize) -> ParseResult {
    let ident = eat_identifier(iter)?;

    if let Ok(keyword) = MusubuKeyword::from_str(&ident) {
        return Ok(MusubuToken {
            token_kind: MusubuTokenKind::Keyword(keyword),
            position,
        });
    }

    Ok(MusubuToken {
        token_kind: MusubuTokenKind::Identifier(ident),
        position,
    })
}

fn tokenize_number(iter: &mut ParseIter, _number: &str, position: usize) -> ParseResult {
    let value = eat_dec_digit(iter)?;

    // 次がない場合
    let Some(next_token) = iter.peek() else {
        return Ok(MusubuToken {
            token_kind: MusubuTokenKind::Literal(MusubuLiteral::Integer {
                value,
                suffix: None,
            }),
            position,
        });
    };

    // TODO: 0b, 0o, 0x

    //
    match &next_token.token_kind {
        TokenKind::Identifier(ident) => {
            // eもしくはEであれば指数表記の浮動小数の可能性がある
            if ident.starts_with('e') || ident.starts_with('E') {
                return tokenize_float_literal_exponent(iter, value, position);
            }

            Ok(MusubuToken {
                token_kind: MusubuTokenKind::Literal(MusubuLiteral::Integer {
                    value,
                    suffix: Some(ident.to_string()),
                }),
                position,
            })
        }
        TokenKind::Symbol(Symbol::Dot) => tokenize_float_literal(iter, &value, position),
        _ => Ok(MusubuToken {
            token_kind: MusubuTokenKind::Literal(MusubuLiteral::Integer {
                value,
                suffix: None,
            }),
            position,
        }),
    }
}

// FLOAT_LITERAL  ::= DEC_LITERAL ( `.` DEC_LITERAL )? FLOAT_EXPONENT ( SUFFIX )?
// の以下パターン
// `.` DEC_LITERAL FLOAT_EXPONENT ( SUFFIX )?
fn tokenize_float_literal(iter: &mut ParseIter, prefix: &str, position: usize) -> ParseResult {
    // .がない場合は呼び出し元がおかしい
    let Some(token) = iter.next() else {
        unreachable!();
    };
    if !matches!(token.token_kind, TokenKind::Symbol(Symbol::Dot)) {
        unreachable!();
    }

    eat_whitespace(iter);

    // 小数部
    let fraction = eat_dec_digit(iter)?;
    let mut value = String::with_capacity(prefix.len() + 1 + fraction.len());
    value.push_str(prefix);
    value.push('.');
    value.push_str(&fraction);

    eat_whitespace(iter);

    // suffix
    let suffix = iter
        .next_if(|t| matches!(t.token_kind, TokenKind::Identifier(_)))
        .and_then(|token| {
            let TokenKind::Identifier(ident) = token.token_kind else {
                unreachable!();
            };
            Some(ident.to_string())
        });

    Ok(MusubuToken {
        token_kind: MusubuTokenKind::Literal(MusubuLiteral::Float(FloatLiteral::Float {
            value,
            suffix,
        })),
        position,
    })
}

// 指数表記
// FLOAT_LITERAL  ::= DEC_LITERAL ( `.` DEC_LITERAL )? FLOAT_EXPONENT ( SUFFIX )?
// FLOAT_EXPONENT ::= [eE] ( `+` | `-` )? ( DEC_DIGIT | `_` )* DEC_DIGIT (DEC_DIGIT | `_` )*
fn tokenize_float_literal_exponent(
    iter: &mut ParseIter,
    significand: String,
    position: usize,
) -> ParseResult {
    // eもしくはEであること
    let Some(next_token) = iter.next() else {
        unreachable!();
    };
    let TokenKind::Identifier(ident) = next_token.token_kind else {
        unreachable!();
    };
    if !ident.starts_with('e') && !ident.starts_with('E') {
        unreachable!();
    }

    //
    if ident.len() > 1 {
        return tokenize_float_literal_exponent_ident(
            iter,
            significand,
            ident[1..].to_string(),
            position,
        );
    }

    eat_whitespace(iter);

    // eもしくはEの後に何かしらトークンがなければエラー
    let Some(token) = iter.next() else {
        return Err(TokenStreamParseError::InvalidFloatExponent);
    };

    // + or - or None
    let is_plus_exponent = if let TokenKind::Symbol(symbol) = &token.token_kind {
        match symbol {
            Symbol::Plus => true,
            Symbol::Minus => false,
            _ => return Err(TokenStreamParseError::InvalidFloatExponent),
        }
    } else {
        true
    };

    eat_whitespace(iter);

    // 指数部(省略不可)
    let exponent = eat_dec_digit(iter)?;
    if exponent.is_empty() {
        return Err(TokenStreamParseError::InvalidFloatExponent);
    }

    eat_whitespace(iter);

    //
    let suffix = iter
        .next_if(|t| matches!(t.token_kind, TokenKind::Identifier(_)))
        .and_then(|token| {
            let TokenKind::Identifier(ident) = token.token_kind else {
                unreachable!();
            };
            Some(ident.to_string())
        });

    Ok(MusubuToken {
        token_kind: MusubuTokenKind::Literal(MusubuLiteral::Float(FloatLiteral::Exponent {
            is_plus_exponent,
            significand,
            exponent,
            suffix,
        })),
        position,
    })
}

fn tokenize_float_literal_exponent_ident(
    iter: &mut ParseIter,
    significand: String,
    exponent: String,
    position: usize,
) -> ParseResult {
    // 分割
    let index = exponent
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(exponent.len());
    let (exponent, suffix) = exponent.split_at(index);

    let suffix = if suffix.is_empty() {
        eat_whitespace(iter);
        iter.next_if(|t| matches!(t.token_kind, TokenKind::Identifier(_)))
            .and_then(|token| {
                let TokenKind::Identifier(ident) = token.token_kind else {
                    unreachable!();
                };
                Some(ident.to_string())
            })
    } else {
        Some(suffix.to_string())
    };

    Ok(MusubuToken {
        token_kind: MusubuTokenKind::Literal(MusubuLiteral::Float(FloatLiteral::Exponent {
            is_plus_exponent: true,
            significand,
            exponent: exponent.to_string(),
            suffix,
        })),
        position,
    })
}

fn tokenize_operator(iter: &mut ParseIter, _symbol: &Symbol, position: usize) -> ParseResult {
    // 最も長くマッチしたものに合わせる
    let op = if let Some(op) = get_trinary_operator(iter) {
        op
    } else if let Some(op) = get_binary_operator(iter) {
        op
    } else if let Some(op) = get_unary_operator(iter) {
        op
    } else {
        return Err(TokenStreamParseError::InvalidOperator);
    };

    Ok(MusubuToken {
        token_kind: MusubuTokenKind::Operator(op),
        position,
    })
}

fn get_trinary_operator(iter: &mut ParseIter) -> Option<MusubuOperator> {
    let mut clone_iter = iter.clone();
    let pattern = [
        clone_iter.next()?.get_operator()?,
        clone_iter.next()?.get_operator()?,
        clone_iter.next()?.get_operator()?,
    ];

    let op = match pattern {
        [Symbol::GreaterThan, Symbol::GreaterThan, Symbol::Equal] => {
            MusubuOperator::Assign(AssignOperator::RightShiftAssign)
        }
        [Symbol::LessThan, Symbol::LessThan, Symbol::Equal] => {
            MusubuOperator::Assign(AssignOperator::LeftShiftAssign)
        }
        [Symbol::Dot, Symbol::Dot, Symbol::Dot] => MusubuOperator::DotDotDot,
        _ => return None,
    };

    for _ in pattern {
        iter.next();
    }

    Some(op)
}

fn get_binary_operator(iter: &mut ParseIter) -> Option<MusubuOperator> {
    let mut clone_iter = iter.clone();
    let pattern = [
        clone_iter.next()?.get_operator()?,
        clone_iter.next()?.get_operator()?,
    ];

    let op = match pattern {
        // 代入系
        [Symbol::Plus, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::AddAssign),
        [Symbol::Minus, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::SubAssign),
        [Symbol::Star, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::MulAssign),
        [Symbol::Slash, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::DivAssign),
        [Symbol::Percent, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::ModAssign),
        [Symbol::And, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::AndAssign),
        [Symbol::Or, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::OrAssign),
        [Symbol::Caret, Symbol::Equal] => MusubuOperator::Assign(AssignOperator::XorAssign),

        // 論理系
        [Symbol::And, Symbol::And] => MusubuOperator::Logical(LogicalOperator::And),
        [Symbol::Or, Symbol::Or] => MusubuOperator::Logical(LogicalOperator::Or),

        // 比較系
        [Symbol::Equal, Symbol::Equal] => MusubuOperator::Comparison(ComparisonOperator::Equal),
        [Symbol::Not, Symbol::Equal] => MusubuOperator::Comparison(ComparisonOperator::NotEqual),
        [Symbol::LessThan, Symbol::Equal] => {
            MusubuOperator::Comparison(ComparisonOperator::LessThanEqual)
        }
        [Symbol::GreaterThan, Symbol::Equal] => {
            MusubuOperator::Comparison(ComparisonOperator::GreaterThanEqual)
        }

        // その他
        [Symbol::Dot, Symbol::Dot] => MusubuOperator::DotDot,
        [Symbol::LessThan, Symbol::Minus] => MusubuOperator::LeftArrow,
        [Symbol::Minus, Symbol::GreaterThan] => MusubuOperator::RightArrow,
        [Symbol::Colon, Symbol::Colon] => MusubuOperator::Path,

        _ => return None,
    };

    for _ in pattern {
        iter.next();
    }

    Some(op)
}

fn get_unary_operator(iter: &mut ParseIter) -> Option<MusubuOperator> {
    let pattern = iter.peek()?.get_operator()?;

    let op = match pattern {
        Symbol::Plus => MusubuOperator::Binary(BinaryOperator::Addition),
        Symbol::Minus => MusubuOperator::Binary(BinaryOperator::Subtract),
        Symbol::Star => MusubuOperator::Binary(BinaryOperator::Multiply),
        Symbol::Slash => MusubuOperator::Binary(BinaryOperator::Divide),
        Symbol::Percent => MusubuOperator::Binary(BinaryOperator::Modulo),
        Symbol::And => MusubuOperator::Binary(BinaryOperator::And),
        Symbol::Or => MusubuOperator::Binary(BinaryOperator::Or),
        Symbol::Caret => MusubuOperator::Binary(BinaryOperator::Xor),
        Symbol::Equal => MusubuOperator::Assign(AssignOperator::Assign),

        Symbol::Not => MusubuOperator::Logical(LogicalOperator::Not),

        Symbol::GreaterThan => MusubuOperator::Comparison(ComparisonOperator::GreaterThan),
        Symbol::LessThan => MusubuOperator::Comparison(ComparisonOperator::LessThan),

        Symbol::Question => MusubuOperator::Question,
        Symbol::Dot => MusubuOperator::Dot,

        // 括弧
        Symbol::LeftParenthesis => MusubuOperator::LeftParenthesis,
        Symbol::RightParenthesis => MusubuOperator::RightParenthesis,
        Symbol::LeftBrackets => MusubuOperator::LeftBrackets,
        Symbol::RightBrackets => MusubuOperator::RightBrackets,
        Symbol::LeftBrace => MusubuOperator::LeftBrace,
        Symbol::RightBrace => MusubuOperator::RightBrace,

        // その他
        Symbol::Semicolon => MusubuOperator::Semicolon,
        Symbol::Comma => MusubuOperator::Comma,
        Symbol::Colon => MusubuOperator::Colon,
        Symbol::At => MusubuOperator::At,
        Symbol::Underscore => MusubuOperator::Underscore,
        _ => return None,
    };

    iter.next();

    Some(op)
}

//
fn eat_identifier(iter: &mut ParseIter) -> Result<String, TokenStreamParseError> {
    let Some(token) = iter.peek() else {
        unreachable!();
    };
    if !matches!(
        token.token_kind,
        TokenKind::Identifier(_) | TokenKind::Symbol(Symbol::Underscore)
    ) {
        return Err(TokenStreamParseError::InvalidIdentifier);
    }

    from_fn(|| {
        iter.next_if(|token| {
            matches!(
                token.token_kind,
                TokenKind::Identifier(_)
                    | TokenKind::Number(_)
                    | TokenKind::Symbol(Symbol::Underscore)
            )
        })
    })
    .map(|token| match token.token_kind {
        TokenKind::Identifier(ident) => Ok(ident.to_string()),
        TokenKind::Number(num) => Ok(num.to_string()),
        TokenKind::Symbol(Symbol::Underscore) => Ok("_".to_string()),
        _ => Err(TokenStreamParseError::InvalidIdentifier),
    })
    .collect()
}

fn eat_whitespace(iter: &mut ParseIter) {
    while iter
        .next_if(|t| {
            matches!(
                t.token_kind,
                TokenKind::LineBreak(_) | TokenKind::WhiteSpace(_)
            )
        })
        .is_some()
    {}
}

fn eat_dec_digit(iter: &mut ParseIter) -> Result<String, TokenStreamParseError> {
    eat_digit(iter, |token| match token.token_kind {
        TokenKind::Number(num) => Ok(num.to_string()),
        TokenKind::Symbol(Symbol::Underscore) => Ok("_".to_string()),
        _ => Err(TokenStreamParseError::InvalidDecDigit),
    })
}

fn eat_digit(
    iter: &mut ParseIter,
    func: impl FnMut(&Token<'_>) -> Result<String, TokenStreamParseError>,
) -> Result<String, TokenStreamParseError> {
    let Some(token) = iter.peek() else {
        return Ok(String::new());
    };
    if !matches!(
        token.token_kind,
        TokenKind::Number(_) | TokenKind::Symbol(Symbol::Underscore)
    ) {
        return Err(TokenStreamParseError::InvalidNumber);
    }

    from_fn(|| {
        iter.next_if(|token| {
            matches!(
                token.token_kind,
                TokenKind::Number(_) | TokenKind::Symbol(Symbol::Underscore)
            )
        })
    })
    .map(func)
    .collect()
}

fn is_valid_number(iter: &mut ParseIter) -> bool {
    let Some(token) = iter.peek() else {
        return false;
    };
    if !matches!(
        token.token_kind,
        TokenKind::Number(_) | TokenKind::Symbol(Symbol::Underscore)
    ) {
        return false;
    }

    true
}
