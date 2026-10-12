use super::*;

impl TypeChecker {
    pub(super) fn check_vector_operator(
        &self,
        op: &BinaryOperator,
        lhs: &PrimitiveType,
        rhs: &PrimitiveType,
    ) -> TypeCheckResult<PrimitiveType> {
        match (op, lhs, rhs) {
            (
                BinaryOperator::Addition | BinaryOperator::Subtract,
                PrimitiveType::Vector { .. },
                PrimitiveType::Vector { .. },
            ) => {
                if lhs != rhs {
                    return Err(TypeCheckError::TypeMismatch {
                        expected: lhs.clone(),
                        found: rhs.clone(),
                    });
                }
                Ok(lhs.clone())
            }
            (
                BinaryOperator::Multiply | BinaryOperator::Divide,
                PrimitiveType::Vector { type_kind, .. },
                scalar,
            ) if scalar.is_scalar_type() => {
                Self::check_vector_scalar(type_kind, scalar)?;
                Ok(lhs.clone())
            }
            (BinaryOperator::Multiply, scalar, PrimitiveType::Vector { type_kind, .. })
                if scalar.is_scalar_type() =>
            {
                Self::check_vector_scalar(type_kind, scalar)?;
                Ok(rhs.clone())
            }
            _ => Err(TypeCheckError::InvalidOperation {
                op: format!("{op:?}"),
                reason: concat!(
                    "vectors support vector + vector, vector - vector, ",
                    "vector * scalar, scalar * vector and vector / scalar"
                )
                .into(),
            }),
        }
    }

    fn check_vector_scalar(expected: &PrimitiveType, found: &PrimitiveType) -> TypeCheckResult<()> {
        if expected != found {
            return Err(TypeCheckError::TypeMismatch {
                expected: expected.clone(),
                found: found.clone(),
            });
        }
        Ok(())
    }

    pub(super) fn check_vector_assignment(
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
        let binary = match op {
            AssignOperator::AddAssign => BinaryOperator::Addition,
            AssignOperator::SubAssign => BinaryOperator::Subtract,
            AssignOperator::MulAssign => BinaryOperator::Multiply,
            AssignOperator::DivAssign => BinaryOperator::Divide,
            _ => {
                return Err(TypeCheckError::InvalidOperation {
                    op: format!("{op:?}"),
                    reason: "unsupported vector assignment operator".into(),
                });
            }
        };
        self.check_vector_operator(&binary, &lhs.type_kind, &rhs.type_kind)?;
        Ok(lhs)
    }
}
