use crate::errors::TokenStreamParseError;
use std::str::FromStr;

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub(crate) enum MusubuKeyword {
    Fn,
    Let,
    Ref,
    Mut,
    Const,
    Loop,
    For,
    While,
    If,
    Else,
    In,
    Impl,
    Return,
    Break,
    Continue,
    Struct,
    Union,
    Enum,
    Pub,
    Type,
    Match,
    Static,
    Extern,
    True,
    False,
}

impl FromStr for MusubuKeyword {
    type Err = TokenStreamParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let keyword = match s {
            "let" => MusubuKeyword::Let,
            "ref" => MusubuKeyword::Ref,
            "mut" => MusubuKeyword::Mut,
            "pub" => MusubuKeyword::Pub,
            "fn" => MusubuKeyword::Fn,
            "struct" => MusubuKeyword::Struct,
            "enum" => MusubuKeyword::Enum,
            "union" => MusubuKeyword::Union,
            "const" => MusubuKeyword::Const,
            "if" => MusubuKeyword::If,
            "else" => MusubuKeyword::Else,
            "match" => MusubuKeyword::Match,
            "in" => MusubuKeyword::In,
            "for" => MusubuKeyword::For,
            "while" => MusubuKeyword::While,
            "loop" => MusubuKeyword::Loop,
            "break" => MusubuKeyword::Break,
            "continue" => MusubuKeyword::Continue,
            "return" => MusubuKeyword::Return,
            "type" => MusubuKeyword::Type,
            "static" => MusubuKeyword::Static,
            "extern" => MusubuKeyword::Extern,
            "impl" => MusubuKeyword::Impl,
            "true" => MusubuKeyword::True,
            "false" => MusubuKeyword::False,
            _ => return Err(TokenStreamParseError::NotKeyword),
        };

        Ok(keyword)
    }
}
