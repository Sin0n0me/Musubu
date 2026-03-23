#[cfg(test)]
mod tests {
    use crate::common::tests::tests::*;
    use crate::errors::TokenStreamParseError;
    use crate::lexer::token::MusubuOperator;
    use crate::{
        lexer::token::{FloatLiteral, MusubuLiteral},
        *,
    };
    use musubu_ast::*;
    use musubu_lexer::{Tokens, token::*};
    use musubu_primitive::*;

    fn check_vec(src: &str, input_tokens: &Tokens) {
        let tokens = musubu_lexer::tokenize(src).unwrap();
        assert_eq!(tokens.len(), input_tokens.len());

        // このテストで位置はチェックしない
        // このテスト見る責任ではないので
        for (index, token1) in input_tokens.iter().enumerate() {
            let token2 = tokens.get(index).unwrap();
            assert_eq!(token1.token_kind, token2.token_kind);
        }
    }

    fn check_tokenize_err(tokens: Tokens, expect_err: TokenStreamParseError) {
        match tokenize(&tokens) {
            Err(err) => {
                assert_eq!(err, expect_err);
            }
            Ok(tokens) => {
                panic!("expected error: {tokens:#?}");
            }
        };
    }

    fn get_float_literal(stream: &TokenStream) -> FloatLiteral {
        let Some(MusubuLiteral::Float(literal)) = stream.get_literal().cloned() else {
            panic!("expected float");
        };
        literal
    }

    // 以下整数

    // 123
    #[test]
    fn test_integer_basic() {
        let check = |stream: TokenStream| match stream.get_literal().unwrap() {
            MusubuLiteral::Integer { value, suffix } => {
                assert_eq!(value, "123");
                assert!(suffix.is_none());
            }
            _ => panic!("expected integer"),
        };

        let tokens = vec![num("123")];
        let value = "123";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 123i32
    #[test]
    fn test_integer_with_suffix() {
        let check = |stream: TokenStream| match stream.get_literal().unwrap() {
            MusubuLiteral::Integer { value, suffix } => {
                assert_eq!(value, "123");
                assert_eq!(suffix.as_deref(), Some("i32"));
            }
            _ => panic!("expected integer with suffix"),
        };

        let tokens = vec![num("123"), ident("i32")];
        let value = "123i32";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 以下少数(正常)

    // 1.23
    #[test]
    fn test_float_basic() {
        let check = |stream: TokenStream| {
            let literal = get_float_literal(&stream);
            match literal {
                FloatLiteral::Float { value, suffix } => {
                    assert_eq!(value, "1.23");
                    assert!(suffix.is_none());
                }
                _ => panic!("expected float"),
            }
        };

        let tokens = vec![num("1"), sym(Symbol::Dot), num("23")];
        let value = "1.23";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1.23f32
    #[test]
    fn test_float_with_suffix() {
        let check = |stream: TokenStream| {
            let literal = get_float_literal(&stream);
            match literal {
                FloatLiteral::Float { value, suffix } => {
                    assert_eq!(value, "1.23");
                    assert_eq!(suffix.as_deref(), Some("f32"));
                }
                _ => panic!("expected float with suffix"),
            }
        };

        let tokens = vec![num("1"), sym(Symbol::Dot), num("23"), ident("f32")];
        let value = "1.23f32";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1e10
    #[test]
    fn test_exponent_simple() {
        let check = |stream: TokenStream| {
            let literal = get_float_literal(&stream);
            match literal {
                FloatLiteral::Exponent {
                    is_plus_exponent,
                    significand,
                    exponent,
                    suffix,
                } => {
                    assert!(is_plus_exponent);
                    assert_eq!(significand, "1");
                    assert_eq!(exponent, "10");
                    assert!(suffix.is_none());
                }
                _ => panic!("expected exponent"),
            }
        };

        let tokens = vec![num("1"), ident("e10")];
        let value = "1e10";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1e-10
    #[test]
    fn test_exponent_negative() {
        let check = |stream: TokenStream| {
            let literal = get_float_literal(&stream);
            match literal {
                FloatLiteral::Exponent {
                    is_plus_exponent,
                    significand,
                    exponent,
                    ..
                } => {
                    assert!(!is_plus_exponent);
                    assert_eq!(exponent, "10");
                    assert_eq!(significand, "1");
                }
                _ => panic!("expected exponent"),
            }
        };

        let tokens = vec![num("1"), ident("e"), sym(Symbol::Minus), num("10")];
        let value = "1e-10";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1e10f32
    #[test]
    fn test_exponent_with_suffix() {
        let check = |stream: TokenStream| {
            let literal = get_float_literal(&stream);
            match literal {
                FloatLiteral::Exponent {
                    exponent, suffix, ..
                } => {
                    assert_eq!(exponent, "10");
                    assert_eq!(suffix.as_deref(), Some("f32"));
                }
                _ => panic!("expected exponent with suffix"),
            }
        };
        let tokens = vec![num("1"), ident("e10f32")];
        let value = "1e10f32";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1.
    #[test]
    fn test_float_no_fraction() {
        let check = |stream: TokenStream| {
            let literal = get_float_literal(&stream);
            match literal {
                FloatLiteral::Float { value, .. } => {
                    assert_eq!(value, "1.");
                }
                _ => panic!("expected float"),
            }
        };

        let tokens = vec![num("1"), sym(Symbol::Dot)];
        let value = "1.";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1E10
    #[test]
    fn test_exponent_uppercase() {
        let check = |stream: TokenStream| match get_float_literal(&stream) {
            FloatLiteral::Exponent { exponent, .. } => {
                assert_eq!(exponent, "10");
            }
            _ => panic!("expected exponent"),
        };

        let tokens = vec![num("1"), ident("E10")];
        let value = "1E10";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1e+10
    #[test]
    fn test_exponent_plus() {
        let check = |stream: TokenStream| match get_float_literal(&stream) {
            FloatLiteral::Exponent {
                is_plus_exponent,
                exponent,
                ..
            } => {
                assert!(is_plus_exponent);
                assert_eq!(exponent, "10");
            }
            _ => panic!("expected exponent"),
        };

        let tokens = vec![num("1"), ident("e"), sym(Symbol::Plus), num("10")];
        let value = "1e+10";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1e10f64
    #[test]
    fn test_exponent_split_suffix() {
        let check = |stream: TokenStream| match get_float_literal(&stream) {
            FloatLiteral::Exponent {
                exponent, suffix, ..
            } => {
                assert_eq!(exponent, "10");
                assert_eq!(suffix.as_deref(), Some("f64"));
            }
            _ => panic!("expected exponent"),
        };

        let tokens = vec![num("1"), ident("e10f64")];
        let value = "1e10f64";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 以下少数(異常)

    // .5
    // .と5に分解されていればOK
    #[test]
    fn test_float_invalid_prefix() {
        let check = |mut stream: TokenStream| {
            assert_eq!(stream.get_operator(), Some(&MusubuOperator::Dot));
            stream.next();
            assert_eq!(
                stream.get_literal(),
                Some(&MusubuLiteral::Integer {
                    value: "5".to_string(),
                    suffix: None,
                })
            );
        };

        let tokens = vec![sym(Symbol::Dot), num("5")];
        let value = ".5";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 1e
    #[test]
    fn test_error_exponent_no_value() {
        let tokens = vec![num("1"), ident("e")];
        check_tokenize_err(tokens, TokenStreamParseError::InvalidFloatExponent);
    }

    // 1e.
    #[test]
    fn test_error_exponent_invalid_symbol() {
        let tokens = vec![num("1"), ident("e"), sym(Symbol::Dot)];
        check_tokenize_err(tokens, TokenStreamParseError::InvalidFloatExponent);
    }

    // e+の後に数字がなく終了している
    // 1e+
    #[test]
    fn test_error_exponent_sign_only() {
        let tokens = vec![num("1"), ident("e"), sym(Symbol::Plus)];
        check_tokenize_err(tokens, TokenStreamParseError::InvalidFloatExponent);
    }

    // e-の後に数値ではなく.が来ている
    // 1e-.
    #[test]
    fn test_error_exponent_minus_invalid() {
        let tokens = vec![num("1"), ident("e"), sym(Symbol::Minus), sym(Symbol::Dot)];
        check_tokenize_err(tokens, TokenStreamParseError::InvalidNumber);
    }

    // 1.の後に数値ではなく.が来ている
    // 1..
    #[test]
    fn test_error_float_invalid_fraction() {
        let tokens = vec![num("1"), sym(Symbol::Dot), sym(Symbol::Dot)];
        check_tokenize_err(tokens, TokenStreamParseError::InvalidNumber);
    }

    // 以下Identifier

    // abc
    #[test]
    fn test_identifier_alpha() {
        let check = |stream: TokenStream| {
            assert_eq!(stream.get_identifier(), Some("abc"));
        };

        let tokens = vec![ident("abc")];
        let value = "abc";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // abc123
    #[test]
    fn test_identifier_alnum() {
        let check = |stream: TokenStream| {
            assert_eq!(stream.get_identifier(), Some("abc123"));
        };

        let tokens = vec![ident("abc123")];
        let value = "abc123";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // a_b_c
    #[test]
    fn test_identifier_underscore() {
        let check = |stream: TokenStream| {
            assert_eq!(stream.get_identifier(), Some("a_b_c"));
        };

        let tokens = vec![
            ident("a"),
            sym(Symbol::Underscore),
            ident("b"),
            sym(Symbol::Underscore),
            ident("c"),
        ];
        let value = "a_b_c";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // 以下演算子(正常)

    // >>=
    #[test]
    fn test_operator_trinary_shift_assign() {
        let check = |stream: TokenStream| {
            assert_eq!(
                stream.get_assign_operator(),
                Some(&AssignOperator::RightShiftAssign)
            );
        };

        let tokens = vec![
            sym(Symbol::GreaterThan),
            sym(Symbol::GreaterThan),
            sym(Symbol::Equal),
        ];
        let value = ">>=";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // ==
    #[test]
    fn test_operator_binary_equal() {
        let check = |stream: TokenStream| {
            assert_eq!(
                stream.get_comparison_operator(),
                Some(&ComparisonOperator::Equal)
            );
        };

        let tokens = vec![sym(Symbol::Equal), sym(Symbol::Equal)];
        let value = "==";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // +
    #[test]
    fn test_operator_unary_plus() {
        let check = |stream: TokenStream| {
            assert_eq!(
                stream.get_binary_operator(),
                Some(&BinaryOperator::Addition)
            );
        };

        let tokens = vec![sym(Symbol::Plus)];
        let value = "+";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // .+
    #[test]
    fn test_error_invalid_operator_sequence() {
        let check = |mut stream: TokenStream| {
            assert_eq!(stream.get_operator(), Some(&MusubuOperator::Dot));
            stream.next();
            assert_eq!(
                stream.get_binary_operator(),
                Some(&BinaryOperator::Addition)
            );
        };

        let tokens = vec![sym(Symbol::Dot), sym(Symbol::Plus)];
        let value = ".+";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // @@@
    #[test]
    fn test_error_invalid_trinary_operator() {
        let check = |mut stream: TokenStream| {
            assert_eq!(stream.get_operator(), Some(&MusubuOperator::At));
            stream.next();
            assert_eq!(stream.get_operator(), Some(&MusubuOperator::At));
            stream.next();
            assert_eq!(stream.get_operator(), Some(&MusubuOperator::At));
        };

        let tokens = vec![sym(Symbol::At), sym(Symbol::At), sym(Symbol::At)];
        let value = "@@@";
        check_vec(value, &tokens);
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }

    // "  1  "
    #[test]
    fn test_whitespace_ignored() {
        let check = |stream: TokenStream| match stream.get_literal().unwrap() {
            MusubuLiteral::Integer { value, .. } => {
                assert_eq!(value, "1");
            }
            _ => panic!(),
        };

        let tokens = vec![num("1")];
        let value = "  1  ";
        check_vec("1", &tokens); // lexerは空白を除去するため
        check(tokenize_from_vec(tokens));
        check(tokenize_from_str(value));
    }
}
