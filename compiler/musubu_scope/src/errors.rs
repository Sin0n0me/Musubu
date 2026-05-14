#[derive(Debug)]
pub enum ScopeError {
    InvalidScope,
    IllegalHierarchyAccess,
    DuplicateVariable { name: String },
    DuplicateType { name: String },
    UnresolvePath { name: String },
    UnresolveVariable { name: String },
    UnresolveType { name: String },
}
