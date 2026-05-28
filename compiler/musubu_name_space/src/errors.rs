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
