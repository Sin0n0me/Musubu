use crate::{
    errors::ParseError,
    lexer::musubu_keywords::MusubuKeyword,
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, LoopExpr};

impl<'a> PackratAndPrattParser<'a> {
    // InfiniteLoopExpression ::= `loop` BlockExpression
    pub(super) fn parse_infinite_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("InfiniteLoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `loop`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Loop)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // BlockExpression
        let Ok(result) = self.parse_block_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(body) = node.as_ref().clone() else {
            unreachable!();
        };

        self.make_memo_from(key, LoopExpr::Loop { body })
    }

    // PredicateLoopExpression ::= `while` Expression BlockExpression
    pub(super) fn parse_predicate_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("PredicateLoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `while`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::While)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Expression
        let Ok(result) = self.parse_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(condition) = node.as_ref().clone() else {
            unreachable!();
        };

        // BlockExpression
        let Ok(result) = self.parse_block_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(body) = node.as_ref().clone() else {
            unreachable!();
        };

        self.make_memo_from(key, LoopExpr::While { body, condition })
    }

    // IteratorLoopExpression ::= `for` Pattern `in` BlockExpression
    pub(super) fn parse_iterator_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("IteratorLoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `for`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::For)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Pattern
        let Ok(ASTNode::Pattern(pattern)) = self.get_node(Self::parse_pattern).as_deref().cloned()
        else {
            unreachable!();
        };

        // `in`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::In)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Expression
        // except struct expression
        let Ok(iterator) = self.get_expr(Self::parse_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // Block
        let Ok(body) = self.get_expr(Self::parse_block_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        self.make_memo_from(
            key,
            LoopExpr::For {
                pattern: pattern.clone(),
                iterator,
                body,
            },
        )
    }
}
