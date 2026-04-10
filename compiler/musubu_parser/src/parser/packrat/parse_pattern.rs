use crate::{
    errors::ParseError,
    lexer::{musubu_keywords::MusubuKeyword, token::MusubuOperator},
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, Literal, Pattern};
use musubu_primitive::BinaryOperator;
use musubu_span::Spanned;
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // Pattern ::= `|`? PatternNoTopAlt ( `|` PatternNoTopAlt )*
    pub(in crate::parser) fn parse_pattern(&mut self) -> ParseResult {
        let key = self.make_key("Pattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if let Some(BinaryOperator::Or) = self.tokens.get_binary_operator() {
            self.tokens.next();
        }

        let Ok(first) = self
            .get_node(Self::parse_pattern_no_top_alt)
            .as_deref()
            .cloned()
        else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Pattern(first) = first else {
            unreachable!();
        };

        // ( `|` PatternNoTopAlt )*
        let patterns = self.zero_or_more(|parser: &mut Self| -> ParseResult {
            let Some(BinaryOperator::Or) = parser.tokens.get_binary_operator() else {
                return Err(ParseError::NotMatch);
            };
            parser.tokens.next();
            parser.parse_pattern_no_top_alt()
        });

        // 結合
        let pattern = if patterns.is_empty() {
            first
        } else {
            let mut patterns = patterns
                .into_iter()
                .map(|memo| {
                    let Some(node) = memo.get_node() else {
                        unreachable!();
                    };
                    let ASTNode::Pattern(pattern) = node.as_ref().clone() else {
                        unreachable!()
                    };
                    pattern
                })
                .collect::<Vec<_>>();

            patterns.insert(0, first);

            let span = self.make_span(&key);
            Spanned {
                node: Pattern::Multiply(patterns),
                span,
            }
        };

        self.make_memo_from_node(key, Rc::new(ASTNode::Pattern(pattern)))
    }

    // PatternNoTopAlt ::= PatternWithoutRange | RangePattern
    pub(in crate::parser) fn parse_pattern_no_top_alt(&mut self) -> ParseResult {
        let key = self.make_key("PatternNoTopAlt");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let result = self.parse_pattern_without_range();
        self.make_memo_from_result(key, result)
    }

    // PatternWithoutRange ::= LiteralPattern
    //                       | IDENTIFIERPattern
    //
    fn parse_pattern_without_range(&mut self) -> ParseResult {
        let key = self.make_key("PatternWithoutRange");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        let result = self.or(vec![
            Self::parse_literal_pattern,
            Self::parse_identifier_pattern,
        ]);
        self.make_memo_from_result(key, result)
    }

    // LiteralPattern ::= `true`
    //                  | `false`
    //                  | CHAR_LITERAL
    //                  | STRING_LITERAL
    //                  | INTEGER_LITERAL
    //                  | FLOAT_LITERAL
    fn parse_literal_pattern(&mut self) -> ParseResult {
        let key = self.make_key("LiteralPattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if matches!(self.tokens.get_keyword(), Some(MusubuKeyword::True)) {
            self.tokens.next();
            return self.make_memo_from(key, Pattern::Literal(Literal::Bool(true)));
        }
        if matches!(self.tokens.get_keyword(), Some(MusubuKeyword::False)) {
            self.tokens.next();
            return self.make_memo_from(key, Pattern::Literal(Literal::Bool(false)));
        }

        if let Some(literal) = self.tokens.get_literal().cloned() {
            self.tokens.next();
            return match literal.to_literal() {
                Ok(literal) => self.make_memo_from(key, Pattern::Literal(literal)),
                Err(err) => self.make_memo_from_result(key, Err(err)),
            };
        }

        Err(ParseError::NotMatch)
    }

    // IDENTIFIERPattern ::= `ref`? `mut`? IDENTIFIER (`@` PatternNoTopAlt )?
    fn parse_identifier_pattern(&mut self) -> ParseResult {
        let key = self.make_key("IDENTIFIERPattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let is_ref = matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Ref));
        if is_ref {
            self.tokens.next();
        }

        let is_mut = matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Mut));
        if is_mut {
            self.tokens.next();
        }

        let Some(name) = self.tokens.get_identifier() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let name = name.to_string();
        self.tokens.next();

        let subpattern = if let Some(MusubuOperator::At) = self.tokens.get_operator() {
            self.tokens.next();

            self.get_node(Self::parse_pattern_no_top_alt)
                .ok()
                .and_then(|k| {
                    if let ASTNode::Pattern(p) = k.as_ref() {
                        Some(Box::new(p.clone()))
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        self.make_memo_from(
            key,
            Pattern::Identifier {
                ident: name,
                reference: is_ref,
                mutable: is_mut,
            },
        )
    }
}
