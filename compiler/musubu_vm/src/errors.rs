#[derive(Debug)]
pub enum VMError {
    InvalidOperand,
    StackOverflow,
    InstructionLimitExceeded,
    DivisionByZero,
    ArithmeticOverflow,
    IllegalFunctionCall,
    ArgumentCountMismatch,
    ArgumentTypeMismatch,
    InvalidDestinationAddressException,
    IndexOutOfBounds,

    UnreachableIndexOutOfBounds,
}
