#[derive(Debug)]
pub enum DesugarError {
    InvalidLiteral {
        value: alloc::string::String,
        expected: alloc::string::String,
    },
    Unsupported {
        feature: &'static str,
    },
    UnsupportedAssignTarget,
    NotFunction,
    TypeMismatch,
}

impl core::fmt::Display for DesugarError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidLiteral { value, expected } => write!(
                f,
                "literal `{value}` is invalid or out of range for {expected}"
            ),
            Self::Unsupported { feature } => write!(f, "unsupported feature: {feature}"),
            Self::UnsupportedAssignTarget => {
                write!(f, "invalid assignment target; expected a variable")
            }
            Self::NotFunction => write!(f, "expected a function"),
            Self::TypeMismatch => write!(f, "incompatible types during lowering"),
        }
    }
}
