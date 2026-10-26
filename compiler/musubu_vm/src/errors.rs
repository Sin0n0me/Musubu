#[derive(Debug)]
pub enum VMError {
    InvalidOperand,
    StackOverflow,
    InstructionLimitExceeded,
    IllegalFunctionCall,
    ArgumentCountMismatch,
    ArgumentTypeMismatch,
    InvalidDestinationAddressException,
    IndexOutOfBounds,

    UnreachableIndexOutOfBounds,
}
