#[derive(Debug)]
pub enum VMError {
    StackOverflow,
    IllegalFunctionCall,
    InvalidDestinationAddressException,
    IndexOutOfBounds,

    UnreachableIndexOutOfBounds,
}
