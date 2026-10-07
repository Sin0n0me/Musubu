use super::{PackratAndPrattParser, ParseResult};
use crate::{errors::ParseError, lexer::token::MusubuOperator};
use alloc::vec::Vec;
use musubu_ast::{ArrayElements, Expression};

impl PackratAndPrattParser {
    pub(super) fn parse_array_expression(&mut self) -> ParseResult {
        let key = self.make_key("ArrayExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        self.tokens.next();
        let allowed = self.allow_struct_literal;
        self.allow_struct_literal = true;
        let result = self.parse_array_elements();
        self.allow_struct_literal = allowed;
        match result {
            Ok(elements) => self.make_memo_from(key, Expression::Array { elements }),
            Err(error) => self.make_memo_from_result(key, Err(error)),
        }
    }

    fn parse_array_elements(&mut self) -> Result<ArrayElements, ParseError> {
        if self.tokens.get_operator() == Some(&MusubuOperator::RightBrackets) {
            self.tokens.next();
            return Ok(ArrayElements::List(Vec::new()));
        }
        let value = self.get_expr(Self::parse_expression)?;
        if self.tokens.get_operator() == Some(&MusubuOperator::Semicolon) {
            self.tokens.next();
            let count = self.get_expr(Self::parse_expression)?;
            self.close_array()?;
            return Ok(ArrayElements::Repeat { value, count });
        }
        let mut elements = alloc::vec![value];
        while self.tokens.get_operator() == Some(&MusubuOperator::Comma) {
            self.tokens.next();
            if self.tokens.get_operator() == Some(&MusubuOperator::RightBrackets) {
                break;
            }
            elements.push(self.get_expr(Self::parse_expression)?);
        }
        self.close_array()?;
        Ok(ArrayElements::List(
            elements
                .into_iter()
                .map(|element| element.unbox())
                .collect(),
        ))
    }

    fn close_array(&mut self) -> Result<(), ParseError> {
        if self.tokens.get_operator() != Some(&MusubuOperator::RightBrackets) {
            return Err(ParseError::Expected { rule: "`]`" });
        }
        self.tokens.next();
        Ok(())
    }
}
