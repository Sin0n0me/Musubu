use super::*;

pub(super) fn has_index(value: &HIRExpression) -> bool {
    match value {
        HIRExpression::Index { .. } => true,
        HIRExpression::Field { parent, .. } => has_index(parent),
        _ => false,
    }
}

pub(super) fn lower_assignment(
    operator: AssignOperator,
    lhs: HIRExpression,
    rhs: HIRExpression,
) -> DesugarResult<HIRExpression> {
    let mut path = Vec::new();
    let target = index_place(lhs, &mut path)?;
    let operator = match operator {
        AssignOperator::Assign => None,
        AssignOperator::AddAssign => Some(BinaryOperator::Addition),
        AssignOperator::SubAssign => Some(BinaryOperator::Subtract),
        AssignOperator::MulAssign => Some(BinaryOperator::Multiply),
        AssignOperator::DivAssign => Some(BinaryOperator::Divide),
        _ => {
            return Err(DesugarError::Unsupported {
                feature: "this indexed assignment operator",
            });
        }
    };
    Ok(HIRExpression::StoreIndex {
        target,
        path,
        value: Box::new(rhs),
        operator,
    })
}

fn index_place(value: HIRExpression, path: &mut Vec<HIRExpression>) -> DesugarResult<usize> {
    match value {
        HIRExpression::Variable { id, .. } => Ok(id),
        HIRExpression::Field { parent, index, .. } => {
            let root = index_place(*parent, path)?;
            path.push(HIRExpression::Literal(Value::Integer(Integer::Uint64(
                index as u64,
            ))));
            Ok(root)
        }
        HIRExpression::Index { parent, index, .. } => {
            let root = index_place(*parent, path)?;
            path.push(*index);
            Ok(root)
        }
        _ => Err(DesugarError::UnsupportedAssignTarget),
    }
}
