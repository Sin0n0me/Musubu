#[derive(Debug)]
pub enum IRCompileError {
    Located {
        start: usize,
        end: usize,
        error: alloc::boxed::Box<IRCompileError>,
    },
    IllegalBreak,
    IllegalContinue,
    InvalidLoopStatement,
    ExpectRegister,
}

impl core::fmt::Display for IRCompileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Located { error, .. } => write!(f, "{error}"),
            Self::IllegalBreak => write!(f, "`break` is only allowed inside a loop"),
            Self::IllegalContinue => write!(f, "`continue` is only allowed inside a loop"),
            Self::InvalidLoopStatement => write!(f, "invalid loop statement"),
            Self::ExpectRegister => write!(f, "expected an expression producing a value"),
        }
    }
}

impl IRCompileError {
    pub fn range(&self) -> Option<(usize, usize)> {
        match self {
            Self::Located { start, end, .. } => Some((*start, *end)),
            _ => None,
        }
    }
}
