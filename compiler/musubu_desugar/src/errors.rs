#[derive(Debug)]
pub enum DesugarError {
    UnsupportedAssignTarget,
    NotFunction,
    TypeMissMatch,
}
