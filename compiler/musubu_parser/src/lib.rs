#![no_std]

extern crate alloc;

mod common;
mod errors;
mod lexer;
mod parser;

use crate::lexer::lexer::{TokenStream, tokenize};
use crate::parser::packrat::PackratAndPrattParser;
use alloc::rc::Rc;
use alloc::vec::Vec;
use errors::ParseError;
use musubu_ast::ASTNode;
use musubu_lexer::Tokens;

pub fn parse<'a>(tokens: &Tokens<'a>) -> Result<Vec<Rc<ASTNode>>, ParseError> {
    let lexer = tokenize(tokens).unwrap();
    PackratAndPrattParser::new(lexer).parse()
}
