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
