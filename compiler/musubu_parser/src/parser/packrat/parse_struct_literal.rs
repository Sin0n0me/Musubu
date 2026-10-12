use super::{PackratAndPrattParser, ParseResult};
use crate::{errors::ParseError, lexer::token::MusubuOperator};
use alloc::{rc::Rc, string::ToString, vec::Vec};
use musubu_ast::{ASTNode, Expression, NodeMaker, Path};
use musubu_span::{Span, Spanned};

impl PackratAndPrattParser {
    pub(super) fn parse_struct_member(
        &mut self,
        lhs: &Rc<ASTNode>,
        min_bp: u16,
        key: &super::MemoKey,
    ) -> Result<Option<Rc<ASTNode>>, ParseError> {
        let ASTNode::Expression(expr) = lhs.as_ref() else {
            return Ok(None);
        };
        if self.allow_struct_literal
            && self.tokens.get_operator() == Some(&MusubuOperator::LeftBrace)
        {
            if let Expression::Path(path) = expr.node.as_ref() {
                return self.parse_struct_literal(path.clone(), expr.span).map(Some);
            }
        }
        let dot = MusubuOperator::Dot;
        let Some((left_bp, _)) = dot.get_infix_binding_power() else {
            return Ok(None);
        };
        if self.tokens.get_operator() != Some(&dot) || left_bp < min_bp {
            return Ok(None);
        }
        self.tokens.next();
        let field_name = if let Some(name) = self.tokens.get_identifier() {
            name.to_string()
        } else if let Some(crate::lexer::token::MusubuLiteral::Integer {
            value,
            suffix: None,
        }) = self.tokens.get_literal()
        {
            if !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ParseError::Expected {
                    rule: "a decimal tuple index after `.`",
                });
            }
            value.clone()
        } else {
            return Err(ParseError::Expected {
                rule: "a field name or tuple index after `.`",
            });
        };
        self.tokens.next();
        Ok(Some(Rc::new(
            Expression::FieldAccess {
                parent: expr.clone(),
                field_name,
            }
            .make_node(self.make_span(key)),
        )))
    }

    pub(super) fn parse_condition_expression(&mut self) -> ParseResult {
        let allowed = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let result = self.parse_expression();
        self.allow_struct_literal = allowed;
        result
    }

    pub(super) fn parse_struct_literal(
        &mut self,
        path: Spanned<Path>,
        start: Span,
    ) -> Result<Rc<ASTNode>, ParseError> {
        self.tokens.next();
        let mut fields = Vec::new();
        while self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
            let key = self.make_key("StructLiteralField");
            let name = self
                .tokens
                .get_identifier()
                .map(ToString::to_string)
                .ok_or(ParseError::Expected {
                    rule: "a struct field name",
                })?;
            self.tokens.next();
            let span = self.make_span(&key);
            let value = if self.tokens.get_operator() == Some(&MusubuOperator::Colon) {
                self.tokens.next();
                self.get_expr(Self::parse_expression)?
            } else {
                let ASTNode::Expression(value) = Expression::Path(Spanned {
                    node: Path::make(name.clone(), span),
                    span,
                })
                .make_node(span) else {
                    unreachable!()
                };
                value
            };
            fields.push((Spanned { node: name, span }, value));
            if self.tokens.get_operator() != Some(&MusubuOperator::Comma) {
                break;
            }
            self.tokens.next();
        }
        if self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
            return Err(ParseError::Expected {
                rule: "`,` or `}` after a struct field",
            });
        }
        let end = self.tokens.token_end(self.tokens.get_position()) as u32;
        self.tokens.next();
        Ok(Rc::new(
            Expression::StructLiteral { path, fields }.make_node(Span { end, ..start }),
        ))
    }
}
