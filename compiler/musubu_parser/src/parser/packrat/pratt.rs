use super::MemoResult;
use crate::{
    errors::ParseError,
    lexer::token::MusubuOperator,
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, AssignOperator, Expression, LogicalOperator, NodeMaker};
use musubu_primitive::{BinaryOperator, ComparisonOperator};
use musubu_span::{Span, SpannedBox};
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // Pratt Parsing
    // メモ化はしない
    pub(in crate::parser) fn pratt_parse(&mut self, min_bp: u16) -> ParseResult {
        let key = self.make_key("<Pratt>");
        self.bp_stack.push(min_bp);

        // 前置演算子or式
        let Ok(mut lhs) = self.expr_or_prefix_op() else {
            return Err(ParseError::NotMatch);
        };

        // 演算子に応じた判定
        loop {
            let Some(op) = self.tokens.get_operator().cloned() else {
                break;
            };

            // 後置演算子
            if let Some(left_bp) = op.get_postfix_binding_power() {
                if left_bp < min_bp {
                    break;
                }
                self.tokens.next();

                lhs = if let Some(pair) = op.counterpart_of() {
                    // 中身だけを判断
                    let rhs = self.postfix_pair_inside(&op)?;
                    if Some(&pair) != self.tokens.get_operator() {
                        return Err(ParseError::NotMatch);
                    }
                    self.tokens.next();

                    let span = self.make_span(&key);
                    make_ast_from_operator(span, &op, Some(lhs), None, Some(rhs))?
                } else {
                    lhs
                };
                continue;
            }

            // 中置演算子
            if let Some((left_bp, right_bp)) = op.get_infix_binding_power() {
                if left_bp < min_bp {
                    break;
                }
                self.tokens.next();

                lhs = if let Some(pair) = op.counterpart_of() {
                    let Some(mhs) = self.pratt_parse(0)?.get_node() else {
                        return Err(ParseError::UnexpectedAST);
                    };
                    if Some(&pair) != self.tokens.get_operator() {
                        return Err(ParseError::NotMatch);
                    }
                    self.tokens.next();

                    let Some(rhs) = self.pratt_parse(right_bp)?.get_node() else {
                        return Err(ParseError::UnexpectedAST);
                    };

                    let span = self.make_span(&key);
                    make_ast_from_operator(span, &op, Some(lhs), Some(mhs), Some(rhs))?
                } else {
                    let Some(rhs) = self.pratt_parse(right_bp)?.get_node() else {
                        return Err(ParseError::UnexpectedAST);
                    };
                    let span = self.make_span(&key);

                    make_ast_from_operator(span, &op, Some(lhs), None, Some(rhs))?
                };

                continue;
            }

            break;
        }

        self.bp_stack.pop();
        Ok(MemoResult::from_node(lhs, self.tokens.get_position()))
    }

    fn expr_or_prefix_op(&mut self) -> Result<Rc<ASTNode>, ParseError> {
        // 演算子以外はExpressionとしてパース
        let Some(op) = self.tokens.get_operator() else {
            return self
                .parse_expression()?
                .get_node()
                .ok_or(ParseError::NotMatch);
        };

        // ペア
        if let Some(pair) = op.counterpart_of() {
            self.tokens.next();
            let lhs = self.pratt_parse(0)?.get_node();
            if self.tokens.get_operator() == Some(&pair) {
                return Err(ParseError::NotMatch);
            }
            self.tokens.next();
            return lhs.ok_or(ParseError::NotMatch);
        }

        // その他演算子
        let Some(r_bp) = op.get_prefix_binding_power() else {
            return Err(ParseError::UnexpectedOperator);
        };
        self.tokens.next();
        let Some(lhs) = self.pratt_parse(r_bp)?.get_node() else {
            return Err(ParseError::UnexpectedOperator);
        };

        Ok(lhs)
    }

    fn postfix_pair_inside(&mut self, left_op: &MusubuOperator) -> Result<Rc<ASTNode>, ParseError> {
        match left_op {
            MusubuOperator::LeftParenthesis => self
                .parse_call_params()?
                .get_node()
                .ok_or(ParseError::NotMatch),

            _ => self.pratt_parse(0)?.get_node().ok_or(ParseError::NotMatch),
        }
    }
}

fn make_ast_from_operator(
    span: Span,
    op: &MusubuOperator,
    lhs: Option<Rc<ASTNode>>,
    mhs: Option<Rc<ASTNode>>,
    rhs: Option<Rc<ASTNode>>,
) -> Result<Rc<ASTNode>, ParseError> {
    // ()など特殊な文
    match op {
        MusubuOperator::LeftParenthesis => return make_brackets_ast(span, lhs, mhs, rhs),
        _ => (),
    };

    let convert = |node: Option<Rc<ASTNode>>| -> Option<_> {
        let ASTNode::Expression(expr) = node?.as_ref().clone() else {
            return None;
        };
        Some(expr)
    };
    let lhs = convert(lhs);
    let mhs = convert(mhs);
    let rhs = convert(rhs);

    match [lhs, mhs, rhs] {
        [Some(lhs), None, None] => {
            unimplemented!()
        }
        [None, None, Some(rhs)] => {
            unimplemented!()
        }
        [Some(lhs), None, Some(rhs)] => match op {
            MusubuOperator::Binary(op) => make_binary_op_ast(span, op, lhs, rhs),
            MusubuOperator::Assign(op) => make_assign_op_ast(span, op, lhs, rhs),
            MusubuOperator::Comparison(op) => make_comparison_op_ast(span, op, lhs, rhs),
            MusubuOperator::Logical(op) => make_logical_op_ast(span, op, lhs, rhs),
            _ => Err(ParseError::NotMatch),
        },
        [Some(lhs), Some(mhs), Some(rhs)] => {
            unimplemented!()
        }
        _ => unreachable!(),
    }
}

fn make_binary_op_ast(
    span: Span,
    operator: &BinaryOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Binary {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_assign_op_ast(
    span: Span,
    operator: &AssignOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Assign {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_comparison_op_ast(
    span: Span,
    operator: &ComparisonOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Comparison {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_logical_op_ast(
    span: Span,
    operator: &LogicalOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Logical {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_brackets_ast(
    span: Span,
    lhs: Option<Rc<ASTNode>>,
    mhs: Option<Rc<ASTNode>>,
    rhs: Option<Rc<ASTNode>>,
) -> Result<Rc<ASTNode>, ParseError> {
    let lhs = lhs.as_deref().cloned();
    let mhs = mhs.as_deref().cloned();
    let rhs = rhs.as_deref().cloned();

    let node = match [lhs, mhs, rhs] {
        [
            Some(ASTNode::Expression(function)),
            None,
            Some(ASTNode::CallParams(arguments)),
        ] => Expression::Call {
            function,
            arguments,
        }
        .make_node(span),
        _ => unreachable!(),
    };

    Ok(Rc::new(node))
}
