use super::*;
use alloc::vec;

#[test]
fn empty_input_has_no_tokens() {
    assert!(tokenize("").unwrap().is_empty());
}

#[test]
fn unicode_identifiers_use_byte_offsets() {
    let tokens = tokenize("変数2+次").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].token_kind, TokenKind::Identifier("変数2"));
    assert_eq!((tokens[0].token_pos, tokens[0].end()), (0, 7));
    assert_eq!(tokens[1].token_kind, TokenKind::Symbol(Symbol::Plus));
    assert_eq!((tokens[1].token_pos, tokens[1].end()), (7, 8));
    assert_eq!(tokens[2].token_kind, TokenKind::Identifier("次"));
    assert_eq!((tokens[2].token_pos, tokens[2].end()), (8, 11));
}

#[test]
fn numeric_suffix_and_underscore_are_separate_raw_tokens() {
    let tokens = tokenize("123u8_name").unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|t| t.token_kind.clone())
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Number("123"),
            TokenKind::Identifier("u8"),
            TokenKind::Symbol(Symbol::Underscore),
            TokenKind::Identifier("name"),
        ]
    );
    assert_eq!(
        tokens
            .iter()
            .map(|t| (t.token_pos, t.end()))
            .collect::<Vec<_>>(),
        [(0, 3), (3, 5), (5, 6), (6, 10)]
    );
}

#[test]
fn whitespace_and_line_breaks_preserve_every_byte() {
    let tokens = tokenize(" \t\r\n\nx").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(
        tokens[0].token_kind,
        TokenKind::WhiteSpace(vec![Space::Space, Space::Tab])
    );
    assert_eq!(
        tokens[1].token_kind,
        TokenKind::LineBreak(vec![LineBreak::CR, LineBreak::LF, LineBreak::LF])
    );
    assert_eq!((tokens[0].token_pos, tokens[0].end()), (0, 2));
    assert_eq!((tokens[1].token_pos, tokens[1].end()), (2, 5));
    assert_eq!((tokens[2].token_pos, tokens[2].end()), (5, 6));
}

#[test]
fn invalid_character_after_unicode_reports_byte_position() {
    assert!(matches!(
        tokenize("名前😀"),
        Err(TokenizeError::InvalidCharacters {
            c: '😀',
            position: 6
        })
    ));
}

#[test]
fn unsupported_ascii_whitespace_reports_position() {
    assert!(matches!(
        tokenize("x\u{000c}"),
        Err(TokenizeError::UnusableWhitespace {
            c: '\u{000c}',
            position: 1
        })
    ));
}

#[test]
fn raw_token_spans_cover_source_without_gaps() {
    for source in [
        "fn main() {}",
        "日本語123\r\n\t=42;",
        "0..=255u8",
        "([{}]);,:_+-*/%&|^!<>=",
    ] {
        let tokens = tokenize(source).unwrap();
        let mut end = 0;
        for token in tokens {
            assert_eq!(token.token_pos, end, "{source}");
            assert!(source.is_char_boundary(token.end()));
            assert!(token.end() > token.token_pos);
            end = token.end();
        }
        assert_eq!(end, source.len());
    }
}
