#![no_std]

extern crate alloc;

mod common;
pub mod errors;
mod lexer;
mod parser;

#[cfg(test)]
use crate::lexer::lexer::tokenize;
use crate::lexer::lexer::{TokenStream, tokenize_located};
use crate::parser::packrat::PackratAndPrattParser;
use alloc::rc::Rc;
use alloc::vec::Vec;
use errors::ParseError;
use musubu_ast::ASTNode;
use musubu_lexer::Tokens;

pub fn parse<'a>(tokens: &Tokens<'a>) -> Result<Vec<Rc<ASTNode>>, ParseError> {
    let lexer = tokenize_located(tokens).map_err(ParseError::TokenStreamError)?;
    PackratAndPrattParser::new(lexer).parse()
}
