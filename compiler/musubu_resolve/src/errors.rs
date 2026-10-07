use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use musubu_desugar::errors::DesugarError;
use musubu_name_space::errors::NameSpaceError;
use musubu_scope::errors::ScopeError;
use musubu_span::Span;
use musubu_type_check::errors::TypeCheckError;

// TODO Spanによるエラー箇所特定ロジック完成後,
// Fromトレイト実装部分は削除しエラー内容をその場で構築するように変更

#[derive(Debug)]
pub enum ResolveError {
    InvalidTuple {
        message: String,
    },
    InvalidEnum {
        message: String,
    },
    InvalidStruct {
        message: String,
    },
    Located {
        span: Span,
        error: Box<ResolveError>,
    },
    Unsupported {
        feature: &'static str,
    },
    InvalidBreakValue,
    InvalidArrayLength,
    IllegalBreak,
    IllegalContinue,
    UnresolvedPath {
        name: String,
    },
    UndefinedVariable {
        name: String,
    },
    DuplicateDefinition {
        name: String,
    },
    UnresolvedType {
        name: String,
    },
    UnresolvedTypes {
        names: Vec<String>,
    },
    InvalidModuleScope,
    ExpectedValuePathButFoundType {
        name: String,
    },
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

impl ResolveError {
    pub fn at(self, span: Span) -> Self {
        match self {
            Self::Located { .. } => self,
            error => Self::Located {
                span,
                error: Box::new(error),
            },
        }
    }

    pub fn span(&self) -> Option<Span> {
        match self {
            Self::Located { span, .. } => Some(*span),
            _ => None,
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Located { error, .. } => write!(f, "{error}"),
            Self::InvalidEnum { message } => write!(f, "{message}"),
            Self::InvalidTuple { message } => write!(f, "{message}"),
            Self::InvalidStruct { message } => write!(f, "{message}"),
            Self::Unsupported { feature } => write!(f, "unsupported feature: {feature}"),
            Self::InvalidBreakValue => write!(f, "only `loop` may return a value with `break`"),
            Self::InvalidArrayLength => write!(
                f,
                "array repeat count must be a nonnegative integer literal representable by u32"
            ),
            Self::IllegalBreak => write!(f, "`break` is only allowed inside a loop"),
            Self::IllegalContinue => write!(f, "`continue` is only allowed inside a loop"),
            Self::UnresolvedPath { name } => write!(f, "cannot resolve path `{name}`"),
            Self::UndefinedVariable { name } => write!(f, "undefined variable `{name}`"),
            Self::DuplicateDefinition { name } => write!(f, "duplicate definition of `{name}`"),
            Self::UnresolvedType { name } => write!(f, "cannot determine type of `{name}`"),
            Self::UnresolvedTypes { names } => {
                write!(f, "cannot determine types of {}", names.join(", "))
            }
            Self::InvalidModuleScope => write!(f, "invalid module scope"),
            Self::ExpectedValuePathButFoundType { name } => {
                write!(f, "expected a value, found type `{name}`")
            }
            Self::TypeCheckError(error) => write!(f, "{error}"),
            Self::ScopeError(error) => write!(f, "{error}"),
            Self::NameSpaceError(error) => write!(f, "{error}"),
            Self::DesugarError(error) => write!(f, "{error}"),
        }
    }
}
