mod parse_loop_expr;
mod parse_path;

use crate::{
    errors::ParseError,
    lexer::{musubu_keywords::MusubuKeyword, token::MusubuOperator},
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, AssignOperator, Expression, Literal, Statement};
use musubu_span::Spanned;
use std::rc::Rc;

use super::MemoResult;

impl<'a> PackratAndPrattParser<'a> {
    // Expression ::= ExpressionWithoutBlock | ExpressionWithBlock
    pub(super) fn parse_expression(&mut self) -> ParseResult {
        let key = self.make_key("Expression");
        let memo = self.get_memo_uncheck(&key);
        match memo {
            Some(MemoResult::Match { .. }) | Some(MemoResult::NotMatch) => return Ok(memo.unwrap()),
            Some(MemoResult::Pending) => (),
            None => (),
        };

        let Ok(result) = self.or(vec![
            Self::parse_expression_without_block,
            Self::parse_expression_with_block,
        ]) else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };
        let ASTNode::Expression(expr) = node else {
            unreachable!("{node:#?}");
        };

        self.make_memo_from_node(key, Rc::new(ASTNode::Expression(expr)))
    }

    // ExpressionWithoutBlock ::= LiteralExpression
    //                          | PathExpression
    //                          | ContinueExpression
    //                          | BreakExpression
    //                          | RangeExpression
    //                          | ReturnExpression
    fn parse_expression_without_block(&mut self) -> ParseResult {
        let key = self.make_key("ExpressionWithoutBlock");
        let memo = self.get_memo_uncheck(&key);
        match memo {
            Some(MemoResult::Match { .. }) | Some(MemoResult::NotMatch) => return Ok(memo.unwrap()),
            Some(MemoResult::Pending) => {
                let result = self.or(vec![
                    Self::parse_literal_expression,
                    Self::parse_path_expression,
                    Self::parse_continue_expression,
                    Self::parse_break_expression,
                    Self::parse_return_expression,
                ]);
                self.make_memo_from_result(key, result)
            }
            None => {
                // Expression + Expression などの演算子用
                let bp = self.bp_stack.pop().unwrap_or(0);
                let result = self.pratt_parse(bp);
                self.make_memo_from_result(key, result)
            }
        }
    }

    // LiteralExpression ::= CHAR_LITERAL
    fn parse_literal_expression(&mut self) -> ParseResult {
        let key = self.make_key("LiteralExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let literal = if let Some(literal) = self.tokens.get_literal().cloned() {
            self.tokens.next();
            match literal.to_literal() {
                Ok(literal) => literal,
                Err(err) => return self.make_memo_from_result(key, Err(err)),
            }
        } else if let Some(keyword) = self.tokens.get_keyword().cloned() {
            self.tokens.next();
            match keyword {
                MusubuKeyword::True => Literal::Bool(true),
                MusubuKeyword::False => Literal::Bool(false),
                _ => return self.make_memo_from_result(key, Err(ParseError::NotMatch)),
            }
        } else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let span = self.make_span(&key);
        self.make_memo_from(
            key,
            Expression::Literal(Spanned {
                node: literal,
                span,
            }),
        )
    }

    // PathExpression ::= PathInExpression | QualifiedPathInExpression
    fn parse_path_expression(&mut self) -> ParseResult {
        let key = self.make_key("PathExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // PathInExpression | QualifiedPathInExpression
        let Ok(result) = self.or(vec![
            Self::parse_path_in_expression,
            Self::parse_qualified_path_in_expression,
        ]) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Path(path) = node else {
            unreachable!();
        };

        self.make_memo_from(key, Expression::Path(path))
    }

    // ContinueExpression ::= `continue` LIFETIME_OR_LABEL?
    fn parse_continue_expression(&mut self) -> ParseResult {
        let key = self.make_key("ContinueExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `continue`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Continue)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from(key, Expression::Continue { label: None })
    }

    // BreakExpression ::= `break`
    fn parse_break_expression(&mut self) -> ParseResult {
        let key = self.make_key("BreakExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `break`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Break)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from(
            key,
            Expression::Break {
                label: None,
                expression: None,
            },
        )
    }

    // ReturnExpression ::= `return` Expression?
    fn parse_return_expression(&mut self) -> ParseResult {
        let key = self.make_key("ReturnExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Return)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let expr = self
            .option(Self::parse_expression)
            .and_then(|memo| memo.get_node())
            .and_then(|component| {
                let ASTNode::Expression(e) = component.as_ref() else {
                    return None;
                };
                Some(e.clone())
            });

        self.make_memo_from(key, Expression::Return(expr))
    }

    // ExpressionWithBlock ::= BlockExpression
    //                       | LoopExpression
    //                       | IfExpression
    //                       | IfLetExpression
    //                       | MatchExpression
    fn parse_expression_with_block(&mut self) -> ParseResult {
        let key = self.make_key("ExpressionWithBlock");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let result = self.or(vec![
            Self::parse_block_expression,
            Self::parse_loop_expression,
            Self::parse_if_expression,
        ]);

        self.make_memo_from_result(key, result)
    }

    // BlockExpression ::= `{` Statements? `}`
    pub(super) fn parse_block_expression(&mut self) -> ParseResult {
        let key = self.make_key("BlockExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `{`
        let Some(MusubuOperator::LeftBrace) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        // Statements?
        let statements = self
            .option(Self::parse_statements)
            .and_then(|memo| memo.get_node())
            .and_then(|node| {
                let ASTNode::Statements(statements) = node.as_ref().clone() else {
                    unreachable!()
                };
                Some(statements)
            })
            .unwrap_or_default();

        // `}`
        let Some(MusubuOperator::RightBrace) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        self.make_memo_from(key, Expression::Block(statements))
    }

    // LoopExpression ::= LoopLabel? (
    //                    InfiniteLoopExpression
    //                  | PredicateLoopExpression
    //                  | PredicatePatternLoopExpression
    //                  | IteratorLoopExpression
    //                  | LabelBlockExpression
    //                   )
    fn parse_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("LoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(result) = self.or(vec![
            Self::parse_infinite_loop_expression,
            Self::parse_predicate_loop_expression,
            Self::parse_iterator_loop_expression,
        ]) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Loop(loop_expr) = node else {
            unreachable!();
        };

        self.make_memo_from(key, Expression::Loop(loop_expr))
    }

    // IfExpression ::= `if` Expression BlockExpression ( `else` ( BlockExpression | IfExpression | IfLetExpression ) )?
    fn parse_if_expression(&mut self) -> ParseResult {
        let key = self.make_key("IfExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `if`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::If)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Expression
        // 条件式
        let Ok(condition) = self.get_expr(Self::parse_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // BlockExpression
        // thenブロック
        let Ok(then_body) = self.get_expr(Self::parse_block_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // else
        let else_body = if matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Else)) {
            self.tokens.next();

            // elseブロック
            if let Ok(ASTNode::Expression(expr)) = self
                .get_node(Self::parse_block_expression)
                .as_deref()
                .cloned()
            {
                Some(expr)
            } else if let Ok(ASTNode::Expression(expr)) =
                self.get_node(Self::parse_if_expression).as_deref().cloned()
            {
                Some(expr)
            } else {
                return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
            }
        } else {
            None
        };

        self.make_memo_from(
            key,
            Expression::If {
                condition,
                then_body,
                else_body,
            },
        )
    }

    // Statements ::= Statement+
    //              | Statement+ ExpressionWithoutBlock
    //              | ExpressionWithoutBlock
    fn parse_statements(&mut self) -> ParseResult {
        let key = self.make_key("Statements");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // Statement+
        let mut statements = vec![];
        if let Ok(list) = self.one_or_more(Self::parse_statement) {
            for memo in list {
                let Some(node) = memo.get_node() else {
                    return self.make_memo_from_result(key, Err(ParseError::NotMatch));
                };
                let ASTNode::Statement(statement) = node.as_ref() else {
                    unreachable!();
                };
                statements.push(statement.clone());
            }
        }

        // ExpressionWithoutBlock
        if let Some(node) = self
            .option(Self::parse_expression_without_block)
            .map(|memo| memo.get_node())
            .flatten()
        {
            let ASTNode::Expression(expr) = node.as_ref().clone() else {
                unreachable!();
            };
            let span = expr.span;
            statements.push(Spanned {
                node: Statement::Expression(expr),
                span,
            });
        }

        if statements.is_empty() {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }

        self.make_memo_from_node(key, Rc::new(ASTNode::Statements(statements)))
    }

    // Statement ::= `;` | Item | LetStatement | ExpressionStatement
    fn parse_statement(&mut self) -> ParseResult {
        let key = self.make_key("Statement");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `;` | Item | LetStatement | ExpressionStatement
        let result = self.or(vec![
            |parser: &mut Self| -> ParseResult {
                let key = parser.make_key(";");
                let Some(MusubuOperator::Semicolon) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                parser.make_memo_from(key, Statement::Semicolon)
            },
            Self::parse_item,
            Self::parse_let_statement,
            Self::parse_expression_statement,
        ]);
        let Ok(Some(node)) = result.map(|memo| memo.get_node().as_deref().cloned()) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let statement = match node {
            ASTNode::Item { item, .. } => Statement::Item(item),
            ASTNode::Expression(expr) => Statement::Expression(expr),
            ASTNode::Statement(statement) => statement.node,
            _ => unreachable!(),
        };

        self.make_memo_from(key, statement)
    }

    // LetStatement ::= `let` PatternNoTopAlt ( `:` Type )? (`=` Expression )? `;`
    fn parse_let_statement(&mut self) -> ParseResult {
        let key = self.make_key("LetStatement");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `let`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Let)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // PatternNoTopAlt
        let Ok(pattern_kind) = self.get_node(Self::parse_pattern_no_top_alt) else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };
        let ASTNode::Pattern(pattern) = pattern_kind.as_ref() else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };

        // ( `:` Type )?
        let variable_type = if let Some(MusubuOperator::Colon) = self.tokens.get_operator() {
            self.tokens.next();
            self.get_node(Self::parse_type).ok().and_then(|k| {
                let ASTNode::Type(t) = k.as_ref() else {
                    unreachable!();
                };
                Some(t.clone())
            })
        } else {
            None
        };

        // (`=` Expression )?
        let initializer = if let Some(AssignOperator::Assign) = self.tokens.get_assign_operator() {
            self.tokens.next();
            self.get_node(Self::parse_expression).ok().and_then(|node| {
                let ASTNode::Expression(expr) = node.as_ref() else {
                    unreachable!();
                };
                Some(expr.clone())
            })
        } else {
            None
        };

        if self.tokens.get_operator() != Some(&MusubuOperator::Semicolon) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from(
            key,
            Statement::Let {
                name: pattern.clone(),
                variable_type,
                initializer,
                label: None,
            },
        )
    }

    // ExpressionStatement ::= ExpressionWithoutBlock `;` | ExpressionWithBlock `;`?
    fn parse_expression_statement(&mut self) -> ParseResult {
        let key = self.make_key("ExpressionStatement");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        };

        let result = self.or(vec![
            // ExpressionWithoutBlock `;`
            |parser: &mut Self| -> ParseResult {
                let result = parser.parse_expression_without_block()?;
                let Some(MusubuOperator::Semicolon) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                Ok(result)
            },
            // ExpressionWithBlock `;`?
            |parser: &mut Self| -> ParseResult {
                let result = parser.parse_expression_with_block()?;
                if let Some(MusubuOperator::Semicolon) = parser.tokens.get_operator() {
                    parser.tokens.next();
                };
                Ok(result)
            },
        ]);

        let Ok(memo) = result else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = memo.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(expr) = node.as_ref().clone() else {
            unreachable!("");
        };

        self.make_memo_from(key, Statement::Expression(expr))
    }

    // CallParams ::= Expression ( `,` Expression )* `,`?
    pub(super) fn parse_call_params(&mut self) -> ParseResult {
        let key = self.make_key("CallParams");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // Expression
        let Ok(expr) = self.get_expr(Self::parse_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // ( `,` Expression )*
        let params = self.zero_or_more(|parser: &mut Self| -> ParseResult {
            let Some(MusubuOperator::Comma) = parser.tokens.get_operator() else {
                return Err(ParseError::NotMatch);
            };
            parser.tokens.next();
            parser.parse_expression()
        });

        // 変換
        let params = params
            .into_iter()
            .map(|memo| {
                let Some(node) = memo.get_node().as_deref().cloned() else {
                    return Err(ParseError::NotMatch);
                };
                let ASTNode::Expression(expr) = node else {
                    unreachable!();
                };
                Ok(expr)
            })
            .collect::<Result<Vec<_>, _>>();
        let Ok(mut params) = params else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        params.insert(0, expr);

        if let Some(MusubuOperator::Comma) = self.tokens.get_operator() {
            self.tokens.next();
        };

        let node = ASTNode::CallParams(params);
        self.make_memo_from_node(key, Rc::new(node))
    }
}
