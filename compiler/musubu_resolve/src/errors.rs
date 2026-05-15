use alloc::string::String;
use musubu_name_space::errors::NameSpaceError;
use musubu_scope::errors::ScopeError;
use musubu_type_check::errors::TypeCheckError;

#[derive(Debug)]
pub enum ResolveError {
    UnresolvePath { name: String },
    UndefinedVariable { name: String },
    DuplicateDefinition { name: String },
    TypeError,
    InvalidScope,
    InvalidModule,
    InvalidRetrunType,
    TypeCheckError(TypeCheckError),
    ScopeError(ScopeError),
    NameSpaceError(NameSpaceError),
}

impl From<TypeCheckError> for ResolveError {
    fn from(value: TypeCheckError) -> Self {
        Self::TypeCheckError(value)
    }
}

impl From<ScopeError> for ResolveError {
    fn from(value: ScopeError) -> Self {
        Self::ScopeError(value)
    }
}

impl From<NameSpaceError> for ResolveError {
    fn from(value: NameSpaceError) -> Self {
        Self::NameSpaceError(value)
    }
}
