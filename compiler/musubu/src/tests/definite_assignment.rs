use super::*;

macro_rules! uninitialized_cases {
    ($($name:ident: $source:literal;)*) => { $(
        #[test]
        fn $name() {
            let mut engine = MusubuEngine::new();
            assert!(!compile_with_filename(&mut engine, "tests/initialization.musubu", $source));
            let error = engine.compile_error();
            assert!(error.contains("variable may be uninitialized"), "{error}");
            assert!(error.contains("tests/initialization.musubu:"), "{error}");
            assert!(error.contains('^'), "{error}");
        }
    )* };
}

uninitialized_cases! {
    uninitialized_integer: "fn main() -> i32 { let x: i32; x }";
    uninitialized_boolean: "fn main() -> bool { let x: bool; x }";
    uninitialized_float: "fn main() -> f64 { let x: f64; x }";
    uninitialized_one_branch: "fn main(b: bool) -> i32 { let mut x: i32; if b { x = 1; } x }";
    uninitialized_rhs_before_assignment: "fn main() -> i32 { let mut x: i32; x = x + 1; x }";
    uninitialized_compound_assignment: "fn main() { let mut x: i32; x += 1; }";
    uninitialized_short_circuit: "fn main(b: bool) -> i32 { let mut x: i32; b && { x = 1; true }; x }";
    uninitialized_for_may_be_empty: "fn main(n: i32) -> i32 { let mut x: i32; for i in 0..n { x = i; } x }";
    uninitialized_while_may_not_run: "fn main(b: bool) -> i32 { let mut x: i32; while b { x = 1; break; } x }";
    uninitialized_break_path: "fn main(b: bool) -> i32 { let mut x: i32; loop { if b { break; } x = 1; break; } x }";
    uninitialized_first_iteration: "fn main() { let mut x: i32; loop { x + 1; x = 1; } }";
    uninitialized_fresh_loop_binding: "fn main() { for i in 0..2 { let mut x: i32; if i == 0 { x = 1; } x + 1; } }";
    uninitialized_match_arm: "enum E { A, B } fn main(e: E) -> i32 { let mut x: i32; match e { E::A => { x = 1; }, E::B => {} }; x }";
    uninitialized_field_store: "struct S { x: i32 } fn main() { let mut s: S; s.x = 1; }";
    uninitialized_index_store: "fn main() { let mut v: vec2i32; v[0] = 1; }";
    uninitialized_shadow: "fn main() -> i32 { let x = 7; { let x: i32; x } }";
    uninitialized_call_argument: "fn take(x: i32) {} fn main() { let x: i32; take(x); }";
}

#[test]
fn definite_initialization_preserves_valid_control_flow() {
    for source in [
        "fn main() -> i32 { let mut x: i32; x = 7; x }",
        "fn main() -> i32 { let mut x: i32; if true { x = 7; } else { x = 8; } x }",
        "fn main() -> i32 { let mut x: i32; if false { return 8; } else { x = 7; } x }",
        "fn main() -> i32 { let mut x: i32; loop { x = 7; break; } x }",
        "fn main() -> i32 { let mut x: i32; loop { if false { continue; } x = 7; break; } x }",
        "fn main() -> i32 { let mut x: i32; loop { loop { break; } x = 7; break; } x }",
        "fn main() -> i32 { let mut x: i32; loop { for i in 0..0 { break; } x = 7; break; } x }",
        "fn main() -> i32 { let mut x: i32; match E::A { E::A => { x = 7; }, E::B => { x = 8; } }; x } enum E { A, B }",
        "fn main() -> i32 { match E::V((7, 8)) { E::V((x, y)) => x } } enum E { V((i32, i32)) }",
        "fn main() -> i32 { let x = 7; { let x = x + 1; } x }",
        "fn main() -> i32 { let x: i32; return 7; x }",
        "fn main() -> i32 { let mut x: i32; let y = { x = 7; x }; y }",
    ] {
        assert_result(source, 7);
    }
}

#[test]
fn uninitialized_diagnostic_points_at_read() {
    let mut engine = MusubuEngine::new();
    let source = "fn main() -> i32 {\n    let x: i32;\n    x\n}";
    assert!(!compile_with_filename(&mut engine, "sample.musubu", source));
    assert!(
        engine.compile_error().contains("sample.musubu:3:5"),
        "{}",
        engine.compile_error()
    );
}

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
