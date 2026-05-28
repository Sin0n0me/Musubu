// TODO
// #![no_std]

use musubu_engine::MusubuEngine;
use musubu_ir_compiler::compile_module;
use musubu_lexer::tokenize;
use musubu_parser::parse;
use musubu_resolve::resolve_unordered;

extern crate alloc;

pub fn compile(engine: &mut MusubuEngine, code: &str) -> bool {
    let tokens = match tokenize(code) {
        Ok(tokens) => tokens,
        Err(e) => {
            println!("tokenize error: {e:?}");
            return false;
        }
    };

    let ast_items = match parse(&tokens) {
        Ok(ast) => ast,
        Err(e) => {
            println!("parse error: {e:?}");
            return false;
        }
    };

    // スコープ, 型などの解決
    let ast_items = ast_items.iter().map(|ast| ast.as_ref()).collect::<Vec<_>>();
    let result = resolve_unordered("Musubu", "musubu", &ast_items);
    let hir = match result {
        Err(e) => {
            println!("resolve error: {e:?}");
            return false;
        }
        Ok(hir) => hir,
    };

    // 命令化
    let result = compile_module(&hir);
    let functions = match result {
        Ok(functions) => functions,
        Err(e) => {
            println!("compile error: {e:?}");
            return false;
        }
    };

    // VMに関数登録
    for (id, function) in functions {
        engine.register_function(id, function);
    }

    println!("build sucess");

    true
}
