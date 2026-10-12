use super::*;

impl VM<'_> {
    pub(super) fn eval_matrix_operator(
        op: &BinaryOperator,
        lhs: &Value,
        rhs: &Value,
    ) -> VMResult<Value> {
        let ty = matrix_result_type(op, &lhs.to_type(), &rhs.to_type())
            .ok_or(VMError::InvalidOperand)?;
        let columns = match (lhs, rhs) {
            (Value::Matrix(m), scalar @ (Value::Integer(_) | Value::Float(_))) => {
                scale(op, m, scalar, false)?
            }
            (scalar @ (Value::Integer(_) | Value::Float(_)), Value::Matrix(m)) => {
                scale(op, m, scalar, true)?
            }
            _ => {
                let a = grid(lhs, true)?;
                let b = grid(rhs, false)?;
                if matches!(op, BinaryOperator::Multiply) {
                    product(&a, &b)?
                } else {
                    if a.len() != b.len() {
                        return Err(VMError::InvalidOperand);
                    }
                    a.iter()
                        .zip(&b)
                        .map(|(a, b)| {
                            if a.len() != b.len() {
                                return Err(VMError::InvalidOperand);
                            }
                            // (A +/- B)[c][r] = A[c][r] +/- B[c][r].
                            a.iter().zip(b).map(|(a, b)| scalar_op(op, a, b)).collect()
                        })
                        .collect::<VMResult<Vec<_>>>()?
                }
            }
        };
        match &ty {
            PrimitiveType::Matrix { type_kind, .. } => Ok(Value::Matrix(Matrix::Columns {
                columns: columns
                    .into_iter()
                    .map(|elements| {
                        Value::Vector(Vector::Components {
                            elements,
                            element_type: *type_kind.clone(),
                        })
                    })
                    .collect(),
                matrix_type: ty,
            })),
            PrimitiveType::Vector { type_kind, .. } => Ok(Value::Vector(Vector::Components {
                elements: columns.into_iter().flatten().collect(),
                element_type: *type_kind.clone(),
            })),
            _ => Err(VMError::InvalidOperand),
        }
    }
}

fn grid(value: &Value, left: bool) -> VMResult<Vec<Vec<Value>>> {
    match value {
        Value::Matrix(m) => m
            .columns()
            .iter()
            .map(|c| match c {
                Value::Vector(v) => Ok(v.components()),
                _ => Err(VMError::InvalidOperand),
            })
            .collect(),
        Value::Vector(v) if left => Ok(v.components().into_iter().map(|v| vec![v]).collect()),
        Value::Vector(v) => Ok(vec![v.components()]),
        _ => Err(VMError::InvalidOperand),
    }
}

fn scale(
    op: &BinaryOperator,
    matrix: &Matrix,
    scalar: &Value,
    scalar_left: bool,
) -> VMResult<Vec<Vec<Value>>> {
    let columns = grid(&Value::Matrix(matrix.clone()), true)?;
    // (A * s)[c][r] = A[c][r] * s; (A / s)[c][r] = A[c][r] / s.
    columns
        .iter()
        .map(|c| {
            c.iter()
                .map(|v| {
                    if scalar_left {
                        scalar_op(op, scalar, v)
                    } else {
                        scalar_op(op, v, scalar)
                    }
                })
                .collect()
        })
        .collect()
}

fn product(a: &[Vec<Value>], b: &[Vec<Value>]) -> VMResult<Vec<Vec<Value>>> {
    let rows = a.first().ok_or(VMError::InvalidOperand)?.len();
    if a.iter().any(|c| c.len() != rows) || b.iter().any(|c| c.len() != a.len()) {
        return Err(VMError::InvalidOperand);
    }
    // Column-major product: C[c][r] = sum_k A[k][r] * B[c][k].
    // A vector on the right is a column; one on the left is a row.
    b.iter()
        .map(|column| {
            (0..rows)
                .map(|row| {
                    let mut terms = a
                        .iter()
                        .zip(column)
                        .map(|(a, b)| scalar_op(&BinaryOperator::Multiply, &a[row], b));
                    let first = terms.next().ok_or(VMError::InvalidOperand)??;
                    terms.try_fold(first, |sum, term| {
                        scalar_op(&BinaryOperator::Addition, &sum, &term?)
                    })
                })
                .collect()
        })
        .collect()
}

fn scalar_op(op: &BinaryOperator, a: &Value, b: &Value) -> VMResult<Value> {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => {
            super::vectors::checked_integer(op, a, b).map(Value::Integer)
        }
        (Value::Float(_), Value::Float(_)) => VM::eval_binop(op, a, b),
        _ => Err(VMError::InvalidOperand),
    }
}
