#[derive(Debug)]
pub enum VMError {
    StackOverflow,
    IllegalFunctionCall,
    IndexOutOfBounds,

    UnreachableIndexOutOfBounds,
}
