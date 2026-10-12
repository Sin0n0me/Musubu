use alloc::string::String;

#[derive(Debug)]
pub enum NameSpaceError {
    DuplicateFunction { name: String },
    DuplicateFunctionArgument { name: String },
    DuplicateEnumeration { name: String },
    DuplicateEnumVariant { name: String },
    DuplicateStruct { name: String },
    DuplicateStructField { name: String },
    UnresolvedFunction { name: String },
    UnresolvedStruct { name: String },
    UnresolvedStructField { name: String },
    UnresolvedEnumVariant { name: String },
    UnresolvedEnumeration { name: String },
    IllegalHierarchyAccess,
}

impl core::fmt::Display for NameSpaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateFunction { name } => write!(f, "duplicate function `{name}`"),
            Self::DuplicateFunctionArgument { name } => {
                write!(f, "duplicate function argument `{name}`")
            }
            Self::DuplicateEnumeration { name } => write!(f, "duplicate enumeration `{name}`"),
            Self::DuplicateEnumVariant { name } => write!(f, "duplicate enum variant `{name}`"),
            Self::DuplicateStruct { name } => write!(f, "duplicate struct `{name}`"),
            Self::DuplicateStructField { name } => write!(f, "duplicate struct field `{name}`"),
            Self::UnresolvedFunction { name } => write!(f, "unknown function `{name}`"),
            Self::UnresolvedStruct { name } => write!(f, "unknown struct `{name}`"),
            Self::UnresolvedStructField { name } => write!(f, "unknown struct field `{name}`"),
            Self::UnresolvedEnumVariant { name } => write!(f, "unknown enum variant `{name}`"),
            Self::UnresolvedEnumeration { name } => write!(f, "unknown enumeration `{name}`"),
            Self::IllegalHierarchyAccess => write!(f, "invalid namespace access"),
        }
    }
}
