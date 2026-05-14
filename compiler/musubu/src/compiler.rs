use musubu_desugar::*;
use musubu_ir_compiler::*;
use musubu_lexer::tokenize;
use musubu_parser::parse;
use musubu_primitive::*;
use musubu_resolve::name_resolver::NameResolver;
use musubu_resolve::*;
use musubu_type_check::type_check;
use musubu_vm::VM;
use nalgebra::Matrix4;

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

    // スコープ解決
    let mut name_resolver = NameResolver::new();
    for ast in ast_items.iter() {
        if let Err(e) = name_resolver.add_ast(ast.as_ref()) {
            println!("resolve error(pre): {e:?}");
            return false;
        };
    }
    if let Err(e) = name_resolver.resolve() {
        println!("resolve error(post): {e:?}");
        return false;
    }

    // astの解析
    for ast in ast_items.iter() {
        if let Err(e) = type_check(ast.as_ref()) {
            println!("type check error: {e:?}");
            return false;
        }

        // 脱糖
        let hir = Desugar::new(&name_resolver).desugar(ast.as_ref());

        // 命令化
        let instructions = compile_module(&hir);

        // VMに関数登録
        for ins in instructions {
            // TODO: hashもしくは重複なく一意に求めることのできる値の設定
            // 今のままだと作成した関数は順番に値を割り振られている
            // コンパイルし直す度に0から割り振られるのでバグる
            //
            // ただし今はデモ用に固定させる(呼び出すのは1関数だけなので)
            VM::register_function(0, ins);
        }
    }

    println!("build sucess");

    true
}
