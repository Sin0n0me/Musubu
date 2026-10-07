use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchArm {
    pub pattern: Spanned<MatchPattern>,
    pub body: SpannedBox<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchPattern {
    Tuple(Vec<Spanned<MatchPattern>>),
    Wildcard,
    Binding {
        name: String,
        mutable: bool,
    },
    Variant {
        path: Spanned<Path>,
        kind: EnumVariantKind,
        fields: Vec<(Spanned<String>, Spanned<MatchPattern>)>,
        rest: bool,
    },
}
