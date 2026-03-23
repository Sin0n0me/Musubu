mod common;
mod errors;
mod lexer;
mod parser;

use std::rc::Rc;

use crate::lexer::lexer::{TokenStream, tokenize};
use crate::parser::packrat::PackratAndPrattParser;
use errors::ParseError;
use musubu_ast::ASTNode;
use musubu_lexer::Tokens;

pub fn parse<'a>(tokens: &Tokens<'a>) -> Result<Rc<ASTNode>, ParseError> {
    let lexer = tokenize(tokens).unwrap();
    PackratAndPrattParser::new(lexer).parse()
}
