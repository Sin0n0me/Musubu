use crate::{MusubuValue, function_ffi};
use alloc::{boxed::Box, format, string::String, vec::Vec};
use musubu_primitive::{Matrix, PrimitiveType, ToPrimitiveType, Value, Vector};

/// Enum descriptors are resolved against the called function's signature, so no
/// reference to a compiler cache or engine is retained by a host-owned value.
#[derive(Clone)]
pub(super) enum HostValue {
    Plain(Value),
    Enum {
        name: String,
        variant: String,
        fields: Vec<HostValue>,
    },
}

pub(super) fn supported(ty: &PrimitiveType) -> bool {
    match ty {
        PrimitiveType::Vector { type_kind, .. } | PrimitiveType::Matrix { type_kind, .. } => {
            numeric(type_kind) && function_ffi::supported(type_kind)
        }
        PrimitiveType::NamedEnum { variants, .. } => variants
            .iter()
            .all(|variant| variant.fields.iter().all(|(_, ty)| supported(ty))),
        _ => function_ffi::supported(ty),
    }
}

fn numeric(ty: &PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::Integer { .. } | PrimitiveType::Float { .. }
    )
}

impl HostValue {
    pub(super) fn vector(elements: &[MusubuValue]) -> Result<Self, String> {
        if elements.is_empty() || u32::try_from(elements.len()).is_err() {
            return Err(String::from(
                "vector dimension must be between 1 and u32::MAX",
            ));
        }
        let elements = elements
            .iter()
            .map(|v| v.to_value())
            .collect::<Result<Vec<_>, _>>()?;
        let element_type = elements[0].to_type();
        if !numeric(&element_type) || elements.iter().any(|v| v.to_type() != element_type) {
            return Err(String::from(
                "vector components must have the same numeric type",
            ));
        }
        Ok(Self::Plain(Value::Vector(Vector::Components {
            elements,
            element_type,
        })))
    }

    pub(super) fn matrix(columns: Vec<Self>) -> Result<Self, String> {
        let count =
            u32::try_from(columns.len()).map_err(|_| String::from("too many matrix columns"))?;
        let Some(Self::Plain(Value::Vector(first))) = columns.first() else {
            return Err(String::from("matrix requires one or more vector columns"));
        };
        let column_type = first.to_type();
        let PrimitiveType::Vector {
            dimension: rows,
            type_kind,
        } = column_type.clone()
        else {
            unreachable!();
        };
        let columns = columns
            .into_iter()
            .map(|column| match column {
                Self::Plain(Value::Vector(vector)) if vector.to_type() == column_type => {
                    Ok(Value::Vector(vector))
                }
                _ => Err(String::from(
                    "matrix columns must have identical vector types",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::Plain(Value::Matrix(Matrix::Columns {
            columns,
            matrix_type: PrimitiveType::Matrix {
                columns: count,
                rows,
                type_kind,
            },
        })))
    }

    pub(super) fn to_runtime(&self, expected: &PrimitiveType) -> Result<Value, String> {
        match self {
            Self::Plain(value) if value.to_type() == *expected => Ok(value.clone()),
            Self::Plain(value) => Err(format!(
                "expected {expected:?}, found {:?}",
                value.to_type()
            )),
            Self::Enum {
                name,
                variant,
                fields,
            } => {
                let PrimitiveType::NamedEnum {
                    name: expected_name,
                    variants,
                } = expected
                else {
                    return Err(String::from("enum value requires an enum parameter"));
                };
                if name != expected_name {
                    return Err(format!("expected enum `{expected_name}`, found `{name}`"));
                }
                let index = variants
                    .iter()
                    .position(|v| v.name == *variant)
                    .ok_or_else(|| format!("enum `{name}` has no variant `{variant}`"))?;
                let definition = &variants[index];
                if fields.len() != definition.fields.len() {
                    return Err(format!(
                        "variant `{variant}` expects {} fields, found {}",
                        definition.fields.len(),
                        fields.len()
                    ));
                }
                let fields = fields
                    .iter()
                    .zip(&definition.fields)
                    .enumerate()
                    .map(|(i, (v, (_, ty)))| {
                        v.to_runtime(ty)
                            .map_err(|error| format!("field {i}: {error}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Enum {
                    variant: index,
                    fields,
                    enum_type: expected.clone(),
                })
            }
        }
    }

    pub(super) fn from_runtime(value: Value) -> Result<Self, String> {
        match value {
            Value::Enum {
                variant,
                fields,
                enum_type,
            } => {
                let PrimitiveType::NamedEnum { name, variants } = enum_type else {
                    return Err(String::from("invalid VM enum type"));
                };
                let variant = variants
                    .get(variant)
                    .ok_or_else(|| String::from("invalid VM enum variant"))?
                    .name
                    .clone();
                let fields = fields
                    .into_iter()
                    .map(Self::from_runtime)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Enum {
                    name,
                    variant,
                    fields,
                })
            }
            Value::Vector(_) | Value::Matrix(_) => Ok(Self::Plain(value)),
            _ => {
                MusubuValue::from_value(value.clone())?;
                Ok(Self::Plain(value))
            }
        }
    }

    pub(super) fn kind(&self) -> u32 {
        match self {
            Self::Enum { .. } => MusubuValue::ENUM,
            Self::Plain(Value::Vector(_)) => MusubuValue::VECTOR,
            Self::Plain(Value::Matrix(_)) => MusubuValue::MATRIX,
            Self::Plain(value) => MusubuValue::from_value(value.clone())
                .map(|v| v.kind)
                .unwrap_or(MusubuValue::INVALID),
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Enum { fields, .. } => fields.len(),
            Self::Plain(Value::Vector(v)) => v.component_count(),
            Self::Plain(Value::Matrix(m)) => m.column_count(),
            _ => 0,
        }
    }

    pub(super) fn element(&self, index: usize) -> Result<Self, String> {
        let value = match self {
            Self::Enum { fields, .. } => fields.get(index).cloned(),
            Self::Plain(Value::Vector(v)) => v.component(index).map(Self::Plain),
            Self::Plain(Value::Matrix(m)) => m.column(index).map(Self::Plain),
            _ => return Err(String::from("scalar values have no elements")),
        };
        value.ok_or_else(|| format!("element index {index} is out of bounds"))
    }
}

/// Opaque, immutable, independently owned FFI value. Only value_free releases it.
pub struct MusubuValueHandle(pub(super) HostValue);

impl MusubuValueHandle {
    pub(super) fn owned(value: HostValue) -> *mut Self {
        Box::into_raw(Box::new(Self(value)))
    }
}
