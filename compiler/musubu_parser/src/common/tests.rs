#[cfg(test)]
pub(crate) mod tests {
    use crate::*;
    use musubu_lexer::{
        Tokens,
        token::{Symbol, Token, TokenKind},
    };

    const POS: usize = 0;

    pub fn num<'a>(s: &'a str) -> Token<'a> {
        Token {
            token_kind: TokenKind::Number(s),
            token_pos: POS,
        }
    }

    pub fn ident<'a>(s: &'a str) -> Token<'a> {
        Token {
            token_kind: TokenKind::Identifier(s),
            token_pos: POS,
        }
    }

    pub fn sym<'a>(s: Symbol) -> Token<'a> {
        Token {
            token_kind: TokenKind::Symbol(s),
            token_pos: POS,
        }
    }

    pub fn tokenize_from_vec(tokens: Tokens) -> TokenStream {
        tokenize(&tokens).unwrap()
    }

    pub fn tokenize_from_str<'a>(src: &'a str) -> TokenStream {
        tokenize(&musubu_lexer::tokenize(src).unwrap()).unwrap()
    }

    pub fn parsed<'a>(src: &'a str) -> ASTNode {
        PackratAndPrattParser::new(tokenize_from_str(src))
            .parse()
            .as_deref()
            .unwrap()
            .clone()
    }
}
