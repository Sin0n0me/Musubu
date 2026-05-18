use musubu_desugar::*;
use musubu_ir_compiler::*;
use musubu_lexer::tokenize;
use musubu_parser::parse;
use musubu_resolve::Resolver;
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
    let mut resolver = Resolver::new("M");
    let ast_items = ast_items.iter().map(|ast| ast.as_ref()).collect::<Vec<_>>();

    if let Err(e) = resolver.import("m", ast_items.as_slice()) {
        println!("resolve import error: {e:?}");
        return false;
    }

    let result = resolver.resolve("m", ast_items.as_slice());
    if let Err(e) = result {
        println!("resolve error: {e:?}");
        return false;
    };

    // astの解析
    for ast in ast_items {
        // 脱糖
        let hir = Desugar::new().desugar(ast);

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
