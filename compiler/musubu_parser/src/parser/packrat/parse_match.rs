use super::*;
use crate::lexer::{musubu_keywords::MusubuKeyword, token::MusubuOperator};
use alloc::string::ToString;
use musubu_primitive::EnumVariantKind;

impl PackratAndPrattParser {
    pub(super) fn parse_match_expression(&mut self) -> ParseResult {
        let key = self.make_key("MatchExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        if self.tokens.get_keyword() != Some(&MusubuKeyword::Match) {
            return Err(ParseError::NotMatch);
        }
        self.tokens.next();
        let value = self.get_expr(Self::parse_condition_expression)?;
        self.match_delimiter(MusubuOperator::LeftBrace, "`{` after match value")?;
        let allowed = self.allow_struct_literal;
        self.allow_struct_literal = true;
        let result = self.parse_match_arms();
        self.allow_struct_literal = allowed;
        let arms = result?;
        self.make_memo_from(key, Expression::Match { value, arms })
    }

    fn parse_match_arms(&mut self) -> Result<Vec<MatchArm>, ParseError> {
        let mut arms = Vec::new();
        while self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
            let pattern = self.parse_match_pattern()?;
            self.match_delimiter(MusubuOperator::FatArrow, "`=>` after match pattern")?;
            let body = self.get_expr(Self::parse_expression)?;
            let block = matches!(
                body.node.as_ref(),
                Expression::Block(_)
                    | Expression::If { .. }
                    | Expression::Loop(_)
                    | Expression::Match { .. }
            );
            arms.push(MatchArm { pattern, body });
            if self.tokens.get_operator() == Some(&MusubuOperator::Comma) {
                self.tokens.next();
            } else if !block && self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
                return Err(ParseError::Expected {
                    rule: "`,` after a match arm",
                });
            }
        }
        self.tokens.next();
        Ok(arms)
    }

    fn parse_match_pattern(&mut self) -> Result<Spanned<MatchPattern>, ParseError> {
        let key = self.make_key("MatchPattern");
        let mutable = self.tokens.get_keyword() == Some(&MusubuKeyword::Mut);
        if mutable {
            self.tokens.next();
        }
        if self.tokens.get_operator() == Some(&MusubuOperator::Underscore) {
            if mutable {
                return Err(ParseError::Expected {
                    rule: "a binding after `mut`",
                });
            }
            self.tokens.next();
            return Ok(Spanned {
                node: MatchPattern::Wildcard,
                span: self.make_span(&key),
            });
        }
        let name = self
            .tokens
            .get_identifier()
            .ok_or(ParseError::Expected {
                rule: "an enum pattern or binding",
            })?
            .to_string();
        self.tokens.next();
        let mut path = Path::make(name.clone(), self.make_span(&key));
        while self.tokens.get_operator() == Some(&MusubuOperator::Path) {
            self.tokens.next();
            let segment_key = self.make_key("MatchPathSegment");
            let ident = self
                .tokens
                .get_identifier()
                .ok_or(ParseError::Expected {
                    rule: "an enum variant name",
                })?
                .to_string();
            self.tokens.next();
            path.segments.push(Spanned {
                node: PathSegment {
                    ident,
                    arguments: Vec::new(),
                },
                span: self.make_span(&segment_key),
            });
        }
        if path.segments.len() == 1 {
            return Ok(Spanned {
                node: MatchPattern::Binding { name, mutable },
                span: self.make_span(&key),
            });
        }
        if mutable {
            return Err(ParseError::Expected {
                rule: "a binding name after `mut`",
            });
        }
        let path = Spanned {
            node: path,
            span: self.make_span(&key),
        };
        let (kind, fields, rest) = match self.tokens.get_operator() {
            Some(MusubuOperator::LeftParenthesis) => self.parse_match_fields(false)?,
            Some(MusubuOperator::LeftBrace) => self.parse_match_fields(true)?,
            _ => (EnumVariantKind::Unit, Vec::new(), false),
        };
        Ok(Spanned {
            node: MatchPattern::Variant {
                path,
                kind,
                fields,
                rest,
            },
            span: self.make_span(&key),
        })
    }

    fn parse_match_fields(
        &mut self,
        named: bool,
    ) -> Result<
        (
            EnumVariantKind,
            Vec<(Spanned<alloc::string::String>, Spanned<MatchPattern>)>,
            bool,
        ),
        ParseError,
    > {
        self.tokens.next();
        let close = if named {
            MusubuOperator::RightBrace
        } else {
            MusubuOperator::RightParenthesis
        };
        let mut fields = Vec::new();
        let mut rest = false;
        while self.tokens.get_operator() != Some(&close) {
            if named && self.tokens.get_operator() == Some(&MusubuOperator::DotDot) {
                self.tokens.next();
                if self.tokens.get_operator() == Some(&MusubuOperator::Comma) {
                    self.tokens.next();
                }
                rest = true;
                break;
            }
            let key = self.make_key("MatchField");
            let mutable = named && self.tokens.get_keyword() == Some(&MusubuKeyword::Mut);
            if mutable {
                self.tokens.next();
            }
            let name = if named {
                let name = self
                    .tokens
                    .get_identifier()
                    .ok_or(ParseError::Expected {
                        rule: "a field name",
                    })?
                    .to_string();
                self.tokens.next();
                name
            } else {
                fields.len().to_string()
            };
            let field = Spanned {
                node: name.clone(),
                span: self.make_span(&key),
            };
            let pattern = if named && self.tokens.get_operator() != Some(&MusubuOperator::Colon) {
                Spanned {
                    node: MatchPattern::Binding { name, mutable },
                    span: field.span,
                }
            } else {
                if mutable {
                    return Err(ParseError::Expected {
                        rule: "a shorthand field binding after `mut`",
                    });
                }
                if named {
                    self.tokens.next();
                }
                self.parse_match_pattern()?
            };
            fields.push((field, pattern));
            if self.tokens.get_operator() != Some(&MusubuOperator::Comma) {
                break;
            }
            self.tokens.next();
        }
        self.match_delimiter(close, "a closing match pattern delimiter")?;
        Ok((
            if named {
                EnumVariantKind::Struct
            } else {
                EnumVariantKind::Tuple
            },
            fields,
            rest,
        ))
    }

    fn match_delimiter(
        &mut self,
        op: MusubuOperator,
        rule: &'static str,
    ) -> Result<(), ParseError> {
        if self.tokens.get_operator() != Some(&op) {
            return Err(ParseError::Expected { rule });
        }
        self.tokens.next();
        Ok(())
    }
}
