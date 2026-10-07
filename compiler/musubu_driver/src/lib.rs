#![no_std]

extern crate alloc;

mod diagnostic;

use alloc::string::ToString;
use alloc::vec::Vec;
use musubu_engine::MusubuEngine;
use musubu_ir_compiler::compile_module;
use musubu_lexer::{errors::TokenizeError, tokenize};
use musubu_parser::parse;
use musubu_resolve::resolve_unordered;

pub fn compile(engine: &mut MusubuEngine, code: &str) -> bool {
    compile_with_filename(engine, "<input>", code)
}

/// `filename` is the caller's display path, preferably relative to its project root.
pub fn compile_with_filename(engine: &mut MusubuEngine, filename: &str, code: &str) -> bool {
    engine.clear_compile_error();
    let tokens = match tokenize(code) {
        Ok(tokens) => tokens,
        Err(error) => {
            let (start, end) = match &error {
                TokenizeError::InvalidCharacters { c, position }
                | TokenizeError::UnusableWhitespace { c, position } => {
                    (*position, position + c.len_utf8())
                }
                TokenizeError::NotSymbol => (0, 0),
            };
            engine.set_compile_error(diagnostic::render(
                filename,
                code,
                start,
                end,
                &error.to_string(),
            ));
            return false;
        }
    };
    let ast_items = match parse(&tokens) {
        Ok(ast) => ast,
        Err(error) => {
            let (start, end) = error.range().unwrap_or((code.len(), code.len()));
            engine.set_compile_error(diagnostic::render(
                filename,
                code,
                start,
                end,
                &error.to_string(),
            ));
            return false;
        }
    };
    let ast_items = ast_items.iter().map(|ast| ast.as_ref()).collect::<Vec<_>>();
    let allocator = engine.get_cache().get_function_allocator();
    let hir = match resolve_unordered("Musubu", "musubu", &ast_items, allocator) {
        Ok(hir) => hir,
        Err(error) => {
            let (start, end) = error
                .span()
                .map(|span| (span.start as usize, span.end as usize))
                .unwrap_or((0, 0));
            engine.set_compile_error(diagnostic::render(
                filename,
                code,
                start,
                end,
                &error.to_string(),
            ));
            return false;
        }
    };
    let functions = match compile_module(&hir) {
        Ok(functions) => functions,
        Err(error) => {
            let diagnostic = match error.range() {
                Some((start, end)) => {
                    diagnostic::render(filename, code, start, end, &error.to_string())
                }
                None => alloc::format!("error: {error}\n   --> {filename}\n"),
            };
            engine.set_compile_error(diagnostic);
            return false;
        }
    };
    for (id, function) in functions {
        engine.register_function(id, function);
    }
    true
}
