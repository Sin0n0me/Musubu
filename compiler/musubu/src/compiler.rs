// TODO driverへ移動

use musubu_ir_compiler::{compile_function, compile_module};
use musubu_lexer::tokenize;
use musubu_parser::parse;
use musubu_resolve::{resolve_sequential, resolve_unordered};
use musubu_vm::VM;

pub fn compile(code: &str) -> bool {
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
    let functions = compile_module(&hir);

    // VMに関数登録
    for function in functions {
        println!("-----");
        println!("{function:#?}");
    }

    println!("build sucess");

    true
}
