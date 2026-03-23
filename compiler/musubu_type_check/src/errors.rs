use musubu_primitive::*;

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticError {
    UndefinedVariable {
        name: String,
    },
    DuplicateDefinition {
        name: String,
    },
    TypeMismatch {
        expected: PrimitiveType,
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
    ArgumentCountMismatch {
        expected: usize,
        found: usize,
    },
    NotCallable {
        found: PrimitiveType,
    },
    InvalidScope {},
    InvalidPath {
        name: String,
    },
}
