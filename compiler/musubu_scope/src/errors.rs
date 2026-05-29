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
