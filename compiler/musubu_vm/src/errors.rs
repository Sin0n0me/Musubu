#[derive(Debug)]
pub enum VMError {
    InvalidOperand,
    StackOverflow,
    IllegalFunctionCall,
    InvalidDestinationAddressException,
    IndexOutOfBounds,

    UnreachableIndexOutOfBounds,
}
