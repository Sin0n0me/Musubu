use super::*;

#[test]
fn scalar_functions_require_a_value_on_every_completing_path() {
    for source in [
        "fn f() -> i32 {}",
        "fn f() -> bool {}",
        "fn f() -> f64 {}",
        "fn f(b: bool) -> i32 { if b { return 1; } }",
        "fn f() -> i32 { 42; }",
        "fn f() -> i32 { if true { 42 } }",
    ] {
        let mut engine = MusubuEngine::new();
        assert!(!compile(&mut engine, source), "{source}");
        assert!(
            engine.compile_error().contains("return type mismatch"),
            "{}",
            engine.compile_error()
        );
    }
}

#[test]
fn if_without_else_is_unit_in_value_contexts() {
    for source in [
        "fn f() { let x: i32 = if true { 42 }; }",
        "fn f() { take(if true { 42 }); } fn take(x: i32) {}",
        "fn f() -> i32 { (if true { 42 }) + 1 }",
    ] {
        let mut engine = MusubuEngine::new();
        assert!(!compile(&mut engine, source), "{source}");
        assert!(
            !engine
                .compile_error()
                .contains("expected an expression producing a value"),
            "{}",
            engine.compile_error()
        );
    }
}

#[test]
fn return_checks_preserve_early_returns_and_unit_conditionals() {
    for source in [
        "fn main() -> i32 { return 7; }",
        "fn main() -> i32 { if true { return 7; } else { return 8; } }",
        "fn main() -> i32 { loop { return 7; } }",
        "fn main() -> i32 { if false { 42 } 7 }",
        "fn main() -> i32 { let x: () = if true { 42 }; 7 }",
    ] {
        assert_result(source, 7);
    }
}
