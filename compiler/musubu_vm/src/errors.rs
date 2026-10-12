#[derive(Debug)]
pub enum VMError {
    InvalidOperand,
    StackOverflow,
    IllegalFunctionCall,
    ArgumentCountMismatch,
    ArgumentTypeMismatch,
    InvalidDestinationAddressException,
    IndexOutOfBounds,

    UnreachableIndexOutOfBounds,
}
