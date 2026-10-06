use alloc::string::String;

#[derive(Debug)]
pub enum ScopeError {
    InvalidScope,
    IllegalHierarchyAccess,
    DuplicateVariable {
        name: String,
    },
    DuplicateType {
        name: String,
    },
    TypeConflict {
        name: String,
        expected: String,
        found: String,
    },
    UnresolvedPath {
        name: String,
    },
    UnresolvedVariable {
        name: String,
    },
    NotVariable {
        name: String,
        found: String,
    },
}

impl core::fmt::Display for ScopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidScope => write!(f, "invalid scope"),
            Self::IllegalHierarchyAccess => write!(f, "invalid scope access"),
            Self::DuplicateVariable { name } => write!(f, "duplicate variable `{name}`"),
            Self::DuplicateType { name } => write!(f, "duplicate type `{name}`"),
            Self::TypeConflict {
                name,
                expected,
                found,
            } => write!(
                f,
                "conflicting type for `{name}`: expected {expected}, found {found}"
            ),
            Self::UnresolvedPath { name } => write!(f, "cannot resolve path `{name}`"),
            Self::UnresolvedVariable { name } => write!(f, "undefined variable `{name}`"),
            Self::NotVariable { name, found } => {
                write!(f, "expected variable `{name}`, found {found}")
            }
        }
    }
}
