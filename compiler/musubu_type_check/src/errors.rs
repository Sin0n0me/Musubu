use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use musubu_name_space::errors::NameSpaceError;
use musubu_primitive::*;
use musubu_scope::errors::ScopeError;

#[derive(Debug)]
pub enum TypeCheckError {
    TypeMismatch {
        expected: PrimitiveType,
        found: PrimitiveType,
    },
    NotIterable {
        found: PrimitiveType,
    },
    NotMutable {
        name: String,
    },
    InvalidOperation {
        op: String,
        reason: String,
    },
    InvalidConditionType {
        found: PrimitiveType,
    },
    FunctionReturnMismatch {
        expected: PrimitiveType,
        found: PrimitiveType,
    },
    TupleCountMismatch {
        expected: usize,
        found: usize,
    },
    ArgumentCountMismatch {
        expected: usize,
        found: usize,
    },
    NotCallable {
        found: PrimitiveType,
    },
    InvalidPath {
        name: String,
    },
    InvalidReturnScope,
    DuplicateDefinition {
        name: String,
    },
    InferenceFailure {
        names: Vec<String>,
    },
    UnknownVariable {
        name: String,
    },
    UnknownPattern {
        name: String,
    },
    UnknownType {
        name: String,
    },
    ScopeError(ScopeError),
    NameSpaceError(NameSpaceError),
}

impl From<NameSpaceError> for TypeCheckError {
    fn from(value: NameSpaceError) -> Self {
        TypeCheckError::NameSpaceError(value)
    }
}

impl From<ScopeError> for TypeCheckError {
    fn from(value: ScopeError) -> Self {
        TypeCheckError::ScopeError(value)
    }
}

impl core::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TypeMismatch { expected, found } => {
                write!(
                    f,
                    "type mismatch: expected {}, found {}",
                    expected.to_string(),
                    found.to_string()
                )
            }
            Self::FunctionReturnMismatch { expected, found } => write!(
                f,
                "return type mismatch: expected {}, found {}",
                expected.to_string(),
                found.to_string()
            ),
            Self::NotIterable { found } => write!(f, "type {} is not iterable", found.to_string()),
            Self::NotMutable { name } => write!(f, "cannot assign to immutable value `{name}`"),
            Self::InvalidOperation { op, reason } => {
                write!(f, "invalid operation `{op}`: {reason}")
            }
            Self::InvalidConditionType { found } => {
                write!(f, "condition must be bool, found {}", found.to_string())
            }
            Self::TupleCountMismatch { expected, found } => write!(
                f,
                "pattern element count mismatch: expected {expected}, found {found}"
            ),
            Self::ArgumentCountMismatch { expected, found } => write!(
                f,
                "argument count mismatch: expected {expected}, found {found}"
            ),
            Self::NotCallable { found } => {
                write!(f, "cannot call value of type {}", found.to_string())
            }
            Self::InvalidPath { name } => write!(f, "invalid path `{name}`"),
            Self::InvalidReturnScope => write!(f, "`return` is only allowed inside a function"),
            Self::DuplicateDefinition { name } => write!(f, "duplicate definition of `{name}`"),
            Self::InferenceFailure { names } => {
                write!(f, "cannot infer types of {}", names.join(", "))
            }
            Self::UnknownVariable { name } => write!(f, "unknown variable `{name}`"),
            Self::UnknownPattern { name } => write!(f, "invalid or unsupported pattern `{name}`"),
            Self::UnknownType { name } => write!(f, "unknown type `{name}`"),
            Self::ScopeError(error) => write!(f, "{error}"),
            Self::NameSpaceError(error) => write!(f, "{error}"),
        }
    }
}
