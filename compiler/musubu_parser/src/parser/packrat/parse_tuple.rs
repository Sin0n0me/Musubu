use super::*;
use crate::lexer::token::MusubuOperator;
use alloc::rc::Rc;

impl PackratAndPrattParser {
    fn parenthesized<T>(
        &mut self,
        mut element: impl FnMut(&mut Self) -> Result<Spanned<T>, ParseError>,
    ) -> Result<(Vec<Spanned<T>>, bool), ParseError> {
        self.tokens.next();
        let mut elements = Vec::new();
        let mut comma = false;
        while self.tokens.get_operator() != Some(&MusubuOperator::RightParenthesis) {
            elements.push(element(self)?);
            if self.tokens.get_operator() != Some(&MusubuOperator::Comma) {
                break;
            }
            comma = true;
            self.tokens.next();
        }
        if self.tokens.get_operator() != Some(&MusubuOperator::RightParenthesis) {
            return Err(ParseError::Expected {
                rule: "`,` or `)` in tuple",
            });
        }
        self.tokens.next();
        let grouped = elements.len() == 1 && !comma;
        Ok((elements, grouped))
    }

    pub(super) fn parse_tuple_expression(&mut self) -> Result<Rc<ASTNode>, ParseError> {
        let key = self.make_key("TupleExpression");
        let allowed = self.allow_struct_literal;
        self.allow_struct_literal = true;
        let result = self.parenthesized(|s| {
            let expr = s.get_expr(Self::parse_expression)?;
            Ok(Spanned {
                node: *expr.node,
                span: expr.span,
            })
        });
        self.allow_struct_literal = allowed;
        let (mut elements, grouped) = result?;
        if grouped {
            let value = elements.remove(0);
            return Ok(Rc::new(value.node.make_node(value.span)));
        }
        Ok(Rc::new(
            Expression::Tuple(elements).make_node(self.make_span(&key)),
        ))
    }

    pub(super) fn parse_tuple_type(&mut self) -> ParseResult {
        if self.tokens.get_operator() != Some(&MusubuOperator::LeftParenthesis) {
            return Err(ParseError::NotMatch);
        }
        let key = self.make_key("TupleType");
        let (mut elements, grouped) = self.parenthesized(|s| {
            let node = s.get_node(Self::parse_type)?;
            let ASTNode::Type(ty) = node.as_ref() else {
                return Err(ParseError::UnexpectedAST);
            };
            Ok(ty.clone())
        })?;
        let ty = if grouped {
            elements.remove(0).node
        } else {
            TypeKind::Tuple(elements)
        };
        self.make_memo_from(key, ty)
    }

    pub(super) fn parse_tuple_pattern(&mut self) -> ParseResult {
        let key = self.make_key("TuplePattern");
        let (mut elements, grouped) = self.parenthesized(|s| {
            let node = s.get_node(Self::parse_pattern_no_top_alt)?;
            let ASTNode::Pattern(pattern) = node.as_ref() else {
                return Err(ParseError::UnexpectedAST);
            };
            Ok(pattern.clone())
        })?;
        let pattern = if grouped {
            elements.remove(0).node
        } else {
            Pattern::Tuple(elements)
        };
        self.make_memo_from(key, pattern)
    }

    pub(super) fn parse_tuple_match_pattern(
        &mut self,
    ) -> Result<Spanned<MatchPattern>, ParseError> {
        let key = self.make_key("TupleMatchPattern");
        let (mut elements, grouped) = self.parenthesized(Self::parse_match_pattern)?;
        if grouped {
            return Ok(elements.remove(0));
        }
        Ok(Spanned {
            node: MatchPattern::Tuple(elements),
            span: self.make_span(&key),
        })
    }
}
