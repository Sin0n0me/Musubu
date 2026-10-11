use super::*;

pub fn matrix_result_type(
    op: &BinaryOperator,
    lhs: &PrimitiveType,
    rhs: &PrimitiveType,
) -> Option<PrimitiveType> {
    use PrimitiveType::{Matrix, Vector};
    match (op, lhs, rhs) {
        (BinaryOperator::Addition | BinaryOperator::Subtract, Matrix { .. }, Matrix { .. })
            if lhs == rhs =>
        {
            Some(lhs.clone())
        }
        (
            BinaryOperator::Multiply,
            Matrix {
                columns: ac,
                rows: ar,
                type_kind: at,
            },
            Matrix {
                columns: bc,
                rows: br,
                type_kind: bt,
            },
        ) if ac == br && at == bt => Some(Matrix {
            columns: *bc,
            rows: *ar,
            type_kind: at.clone(),
        }),
        (
            BinaryOperator::Multiply,
            Matrix {
                columns,
                rows,
                type_kind,
            },
            Vector {
                dimension,
                type_kind: vt,
            },
        ) if columns == dimension && type_kind == vt => Some(Vector {
            dimension: *rows,
            type_kind: type_kind.clone(),
        }),
        (
            BinaryOperator::Multiply,
            Vector {
                dimension,
                type_kind: vt,
            },
            Matrix {
                columns,
                rows,
                type_kind,
            },
        ) if rows == dimension && type_kind == vt => Some(Vector {
            dimension: *columns,
            type_kind: type_kind.clone(),
        }),
        (BinaryOperator::Multiply | BinaryOperator::Divide, Matrix { type_kind, .. }, scalar)
            if type_kind.as_ref() == scalar =>
        {
            Some(lhs.clone())
        }
        (BinaryOperator::Multiply, scalar, Matrix { type_kind, .. })
            if type_kind.as_ref() == scalar =>
        {
            Some(rhs.clone())
        }
        _ => None,
    }
}

impl ToPrimitiveType for Matrix {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Columns { matrix_type, .. } => matrix_type.clone(),
            Self::Matrix3(_) => PrimitiveType::Matrix {
                columns: 3,
                rows: 3,
                type_kind: Box::new(PrimitiveType::default_float()),
            },
            Self::Matrix4(_) => PrimitiveType::Matrix {
                columns: 4,
                rows: 4,
                type_kind: Box::new(PrimitiveType::default_float()),
            },
        }
    }
}

impl Matrix {
    pub fn columns(&self) -> Vec<Value> {
        match self {
            Self::Columns { columns, .. } => columns.clone(),
            Self::Matrix3(matrix) => matrix
                .column_iter()
                .map(|c| float_column(c.iter().copied()))
                .collect(),
            Self::Matrix4(matrix) => matrix
                .column_iter()
                .map(|c| float_column(c.iter().copied()))
                .collect(),
        }
    }

    pub fn columns_mut(&mut self) -> &mut [Value] {
        if !matches!(self, Self::Columns { .. }) {
            *self = Self::Columns {
                columns: self.columns(),
                matrix_type: self.to_type(),
            };
        }
        let Self::Columns { columns, .. } = self else {
            unreachable!();
        };
        columns
    }
}

fn float_column(values: impl Iterator<Item = f32>) -> Value {
    Value::Vector(Vector::Components {
        elements: values.map(|v| Value::Float(Float::Float32(v))).collect(),
        element_type: PrimitiveType::default_float(),
    })
}

impl PartialEq for Matrix {
    fn eq(&self, other: &Self) -> bool {
        let left = self.columns();
        let right = other.columns();
        self.to_type() == other.to_type()
            && left.len() == right.len()
            && left.iter().zip(&right).all(|(a, b)| {
                let (Value::Vector(a), Value::Vector(b)) = (a, b) else {
                    return false;
                };
                let a = a.components();
                let b = b.components();
                a.len() == b.len()
                    && a.iter().zip(&b).all(|(a, b)| match (a, b) {
                        (Value::Integer(a), Value::Integer(b)) => a == b,
                        (Value::Float(a), Value::Float(b)) => a == b,
                        _ => false,
                    })
            })
    }
}
