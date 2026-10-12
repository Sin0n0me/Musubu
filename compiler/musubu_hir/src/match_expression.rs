use super::*;

#[derive(Debug, Clone)]
pub struct HIRMatchArm {
    pub pattern: HIRMatchPattern,
    pub body: HIRExpression,
}

#[derive(Debug, Clone)]
pub enum HIRMatchPattern {
    Tuple(Vec<(usize, HIRMatchPattern)>),
    Wildcard,
    Binding(usize),
    Variant {
        index: usize,
        fields: Vec<(usize, HIRMatchPattern)>,
    },
}
