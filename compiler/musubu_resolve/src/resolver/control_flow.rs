use musubu_hir::{HIRBlock, HIRExpression, HIRStatement};

// Track paths reaching the next expression and breaks targeting the current loop.
// Returns, continues and infinite loops cannot reach the following expression.
#[derive(Clone, Copy)]
struct Flow {
    next: bool,
    breaks: bool,
}

impl Flow {
    const NEXT: Self = Self {
        next: true,
        breaks: false,
    };
    fn then(self, rhs: Self) -> Self {
        Self {
            next: self.next && rhs.next,
            breaks: self.breaks || (self.next && rhs.breaks),
        }
    }
}

pub(super) fn can_complete(block: &HIRBlock) -> bool {
    block_flow(block).next
}

fn block_flow(block: &HIRBlock) -> Flow {
    block.statements.iter().fold(Flow::NEXT, |flow, statement| {
        let expr = match statement {
            HIRStatement::Expr(expr) | HIRStatement::Discard(expr) => Some(expr),
            HIRStatement::Let { initializer, .. } => initializer.as_ref(),
        };
        flow.then(expr.map(expression_flow).unwrap_or(Flow::NEXT))
    })
}

fn expression_flow(expr: &HIRExpression) -> Flow {
    match expr {
        HIRExpression::Return(value) | HIRExpression::Break(value) => {
            let flow = value.as_deref().map(expression_flow).unwrap_or(Flow::NEXT);
            Flow {
                next: false,
                breaks: flow.breaks || (flow.next && matches!(expr, HIRExpression::Break(_))),
            }
        }
        HIRExpression::Continue => Flow {
            next: false,
            breaks: false,
        },
        HIRExpression::Block(body) => block_flow(body),
        HIRExpression::Loop { body, .. } => Flow {
            next: block_flow(body).breaks,
            breaks: false,
        },
        HIRExpression::For { iterator, .. } => expression_flow(iterator),
        HIRExpression::Match { value, arms, .. } => {
            let mut alternatives = Flow {
                next: false,
                breaks: false,
            };
            for arm in arms {
                let flow = expression_flow(&arm.body);
                alternatives.next |= flow.next;
                alternatives.breaks |= flow.breaks;
            }
            expression_flow(value).then(alternatives)
        }
        HIRExpression::If {
            cond,
            then_block,
            else_block,
        } => {
            let left = block_flow(then_block);
            let right = else_block.as_ref().map(block_flow).unwrap_or(Flow::NEXT);
            expression_flow(cond).then(Flow {
                next: left.next || right.next,
                breaks: left.breaks || right.breaks,
            })
        }
        HIRExpression::Field { parent, .. } => expression_flow(parent),
        HIRExpression::Store { value, .. }
        | HIRExpression::StoreField { value, .. }
        | HIRExpression::ArrayRepeat { value, .. } => expression_flow(value),
        HIRExpression::BinOp { lhs, rhs, .. }
        | HIRExpression::CmpOp { lhs, rhs, .. }
        | HIRExpression::Range {
            start: lhs,
            end: rhs,
            ..
        } => expression_flow(lhs).then(expression_flow(rhs)),
        HIRExpression::Array { elements, .. }
        | HIRExpression::Call {
            arguments: elements,
            ..
        } => elements
            .iter()
            .fold(Flow::NEXT, |flow, expr| flow.then(expression_flow(expr))),
        HIRExpression::Struct { fields, .. } | HIRExpression::Enum { fields, .. } => {
            fields.iter().fold(Flow::NEXT, |flow, (_, expr)| {
                flow.then(expression_flow(expr))
            })
        }
        HIRExpression::Literal(_) | HIRExpression::Variable { .. } => Flow::NEXT,
    }
}
