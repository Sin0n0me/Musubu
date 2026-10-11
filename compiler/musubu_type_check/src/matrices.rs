use super::*;

impl TypeChecker {
    pub(super) fn check_matrix_operator(
        &self,
        op: &BinaryOperator,
        lhs: &PrimitiveType,
        rhs: &PrimitiveType,
    ) -> TypeCheckResult<PrimitiveType> {
        matrix_result_type(op, lhs, rhs).ok_or_else(|| TypeCheckError::InvalidOperation {
            op: format!("{op:?}"),
            reason: format!("incompatible matrix operation between {} and {} (check dimensions and component types)", lhs.to_string(), rhs.to_string()),
        })
    }

    pub(super) fn check_matrix_assignment(
        &self,
        op: &AssignOperator,
        lhs: TypeSymbol,
        rhs: TypeSymbol,
    ) -> TypeCheckResult<TypeSymbol> {
        if !lhs.is_mutable() {
            return Err(TypeCheckError::NotMutable {
                name: lhs.type_kind.to_string(),
            });
        }
        let op = match op {
            AssignOperator::AddAssign => BinaryOperator::Addition,
            AssignOperator::SubAssign => BinaryOperator::Subtract,
            AssignOperator::MulAssign => BinaryOperator::Multiply,
            AssignOperator::DivAssign => BinaryOperator::Divide,
            _ => {
                return Err(TypeCheckError::InvalidOperation {
                    op: format!("{op:?}"),
                    reason: "unsupported matrix assignment operator".into(),
                });
            }
        };
        let result = self.check_matrix_operator(&op, &lhs.type_kind, &rhs.type_kind)?;
        if result != lhs.type_kind {
            return Err(TypeCheckError::TypeMismatch {
                expected: lhs.type_kind,
                found: result,
            });
        }
        Ok(lhs)
    }
}
