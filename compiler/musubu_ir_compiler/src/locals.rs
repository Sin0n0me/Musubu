use musubu_hir::{HIRBlock, HIRExpression, HIRFunction, HIRStatement};

pub(crate) fn register_count(function: &HIRFunction) -> usize {
    let mut count = 0;
    for parameter in &function.params {
        include(&mut count, parameter.argument);
    }
    visit_block(&function.body, &mut count);
    count
}

fn include(count: &mut usize, symbol: usize) {
    // Local register bank size = highest symbol ID + 1; temporaries start after it.
    *count = (*count).max(symbol + 1);
}

fn visit_block(block: &HIRBlock, count: &mut usize) {
    for statement in &block.statements {
        match statement {
            HIRStatement::Let {
                symbol,
                initializer,
                ..
            } => {
                include(count, *symbol);
                if let Some(value) = initializer {
                    visit_expression(value, count);
                }
            }
            HIRStatement::Expr(value) | HIRStatement::Discard(value) => {
                visit_expression(value, count)
            }
        }
    }
}

fn visit_expression(expression: &HIRExpression, count: &mut usize) {
    match expression {
        HIRExpression::Variable { id, .. } => include(count, *id),
        HIRExpression::Store { target, value } => {
            include(count, *target);
            visit_expression(value, count);
        }
        HIRExpression::BinOp { lhs, rhs, .. }
        | HIRExpression::CmpOp { lhs, rhs, .. }
        | HIRExpression::Range {
            start: lhs,
            end: rhs,
            ..
        } => {
            visit_expression(lhs, count);
            visit_expression(rhs, count);
        }
        HIRExpression::Array { elements, .. }
        | HIRExpression::Call {
            arguments: elements,
            ..
        } => {
            for element in elements {
                visit_expression(element, count);
            }
        }
        HIRExpression::ArrayRepeat { value, .. } => visit_expression(value, count),
        HIRExpression::For {
            symbol,
            iterator,
            body,
        } => {
            include(count, *symbol);
            visit_expression(iterator, count);
            visit_block(body, count);
        }
        HIRExpression::Block(body) | HIRExpression::Loop { body, .. } => visit_block(body, count),
        HIRExpression::If {
            cond,
            then_block,
            else_block,
        } => {
            visit_expression(cond, count);
            visit_block(then_block, count);
            if let Some(body) = else_block {
                visit_block(body, count);
            }
        }
        HIRExpression::Return(value) | HIRExpression::Break(value) => {
            if let Some(value) = value {
                visit_expression(value, count);
            }
        }
        HIRExpression::Literal(_) | HIRExpression::Continue => {}
    }
}
