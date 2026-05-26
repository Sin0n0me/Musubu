use alloc::string::String;
use alloc::vec::Vec;
use musubu_desugar::errors::DesugarError;
use musubu_name_space::errors::NameSpaceError;
use musubu_scope::errors::ScopeError;
use musubu_type_check::errors::TypeCheckError;

// TODO Spanによるエラー箇所特定ロジック完成後,
// Fromトレイト実装部分は削除しエラー内容をその場で構築するように変更

#[derive(Debug)]
pub enum ResolveError {
    UnresolvedPath { name: String },
    UndefinedVariable { name: String },
    DuplicateDefinition { name: String },
    UnresolveType { name: String },
    UnresolveTypes { names: Vec<String> },
    InvalidRetrunType,
    InvalidModuleScope,
    ExpectedValuePathButFoundType { name: String },
    TypeCheckError(TypeCheckError),
    ScopeError(ScopeError),
    NameSpaceError(NameSpaceError),
    DesugarError(DesugarError),
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

impl From<DesugarError> for ResolveError {
    fn from(value: DesugarError) -> Self {
        Self::DesugarError(value)
    }
}
