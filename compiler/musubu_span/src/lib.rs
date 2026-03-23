#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub file_id: u32,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

pub type SpannedVec<T> = Vec<Spanned<T>>;
pub type SpannedBox<T> = Spanned<Box<T>>;

pub trait SpannedAsRef<'a, T> {
    fn as_ref_spanned(&'a self) -> Spanned<&'a T>;

    fn get_node(&self) -> &T;
}

impl<'a, T> SpannedAsRef<'a, T> for Spanned<T> {
    fn as_ref_spanned(&'a self) -> Spanned<&'a T> {
        Spanned {
            node: &self.node,
            span: self.span,
        }
    }

    fn get_node(&self) -> &T {
        &self.node
    }
}

impl<'a, T> SpannedAsRef<'a, T> for Spanned<Box<T>> {
    fn as_ref_spanned(&'a self) -> Spanned<&'a T> {
        Spanned {
            node: &*self.node,
            span: self.span,
        }
    }

    fn get_node(&self) -> &T {
        &*self.node
    }
}

impl<T> Spanned<Box<T>> {
    pub fn unbox(self) -> Spanned<T> {
        let Spanned { node, span } = self;
        Spanned { node: *node, span }
    }
}
