use super::*;

impl VM<'_> {
    pub(super) fn eval_vector_operator(
        op: &BinaryOperator,
        lhs: &Value,
        rhs: &Value,
    ) -> VMResult<Value> {
        let (vector, left, right) = match (op, lhs, rhs) {
            (
                BinaryOperator::Addition | BinaryOperator::Subtract,
                Value::Vector(a),
                Value::Vector(b),
            ) if a.to_type() == b.to_type() => (a, a.components(), b.components()),
            (BinaryOperator::Multiply | BinaryOperator::Divide, Value::Vector(a), scalar)
                if scalar_matches(a, scalar) =>
            {
                let elements = a.components();
                let count = elements.len();
                (a, elements, vec![scalar.clone(); count])
            }
            (BinaryOperator::Multiply, scalar, Value::Vector(b)) if scalar_matches(b, scalar) => {
                let elements = b.components();
                (b, vec![scalar.clone(); elements.len()], elements)
            }
            _ => return Err(VMError::InvalidOperand),
        };
        // Component-wise: (a +/- b)[i] = a[i] +/- b[i]; (a * s)[i] = a[i] * s;
        // (s * a)[i] = s * a[i]; (a / s)[i] = a[i] / s (integers truncate toward zero).
        let elements = left
            .iter()
            .zip(&right)
            .map(|(l, r)| {
                if let (Value::Integer(a), Value::Integer(b)) = (l, r) {
                    checked_integer(op, a, b).map(Value::Integer)
                } else {
                    Self::eval_binop(op, l, r)
                }
            })
            .collect::<VMResult<Vec<_>>>()?;
        let PrimitiveType::Vector { type_kind, .. } = vector.to_type() else {
            unreachable!();
        };
        Ok(Value::Vector(Vector::Components {
            elements,
            element_type: *type_kind,
        }))
    }
}

fn scalar_matches(vector: &Vector, scalar: &Value) -> bool {
    let PrimitiveType::Vector { type_kind, .. } = vector.to_type() else {
        return false;
    };
    matches!(scalar, Value::Integer(_) | Value::Float(_)) && scalar.to_type() == *type_kind
}

fn checked_integer(op: &BinaryOperator, lhs: &Integer, rhs: &Integer) -> VMResult<Integer> {
    macro_rules! calculate {
        ($($kind:ident),*) => {
            match (lhs, rhs) {
                $((Integer::$kind(a), Integer::$kind(b)) => {
                    let value = match op {
                        BinaryOperator::Addition => a.checked_add(*b),
                        BinaryOperator::Subtract => a.checked_sub(*b),
                        BinaryOperator::Multiply => a.checked_mul(*b),
                        BinaryOperator::Divide => a.checked_div(*b),
                        _ => return Err(VMError::InvalidOperand),
                    };
                    value.map(Integer::$kind).ok_or(VMError::InvalidOperand)
                }),*
                _ => Err(VMError::InvalidOperand),
            }
        };
    }
    calculate!(Int8, Int16, Int32, Int64, Uint8, Uint16, Uint32, Uint64)
}
