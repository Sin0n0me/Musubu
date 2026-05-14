#[derive(Debug)]
pub enum NameSpaceError {
    DuplicateFunction { name: String },
    DuplicateFunctionArgument { name: String },
    DuplicateEnumeration { name: String },
    DuplicateEnumeVariant { name: String },
    DuplicateStruct { name: String },
    DuplicateStructField { name: String },
    UnresolveFunction { name: String },
    UnresolveStruct { name: String },
    UnresolveStructField { name: String },
    UnresolveEnumVariant { name: String },
    UnresolveEnumeration { name: String },
    IllegalHierarchyAccess,
}
