#[derive(Debug)]
pub enum IRCompileError {
    IllegalBreak,
    IllegalContinue,
    InvalidLoopStatement,
    ExpectRegister,
}
