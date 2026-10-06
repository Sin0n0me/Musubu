use alloc::vec;
use musubu_driver::{compile, compile_with_filename};
use musubu_engine::MusubuEngine;
use musubu_primitive::{Integer, Value};

fn assert_result(source: &str, expected: i32) {
    let mut engine = MusubuEngine::new();
    assert!(compile(&mut engine, source), "{}", engine.compile_error());
    let result = engine.run_function(0, vec![]).expect("execution failed");
    assert!(
        matches!(result, Some(Value::Integer(Integer::Int32(value))) if value == expected),
        "expected {expected}, got {result:?}"
    );
}

macro_rules! execution_tests {
    ($($name:ident: $source:literal => $expected:expr;)*) => {
        $(#[test]
        fn $name() {
            assert_result($source, $expected);
        })*
    };
}

execution_tests! {
    false_while_skips_body: "fn main() -> i32 { let mut x = 7; while false { x = 0; } x }" => 7;
    while_break: "fn main() -> i32 { let mut i = 0; while i < 10 { i += 1; if i == 3 { break; } } i }" => 3;
    nested_continue: "fn main() -> i32 { let mut s = 0; for i in 0..3 { for j in 0..3 { if j == 1 { continue; } s += 1; } } s }" => 6;
    sequential_loops: "fn main() -> i32 { let mut s = 0; for i in 0..3 { s += i; } for i in [4, 5] { s += i; } s }" => 12;
    return_from_for: "fn main() -> i32 { for i in [4, 5] { return i; } 0 }" => 4;
    boolean_array: "fn main() -> i32 { let mut s = 0; for b in [true, false, true] { if b { s += 1; } } s }" => 2;
    float_array: "fn main() -> i32 { let mut s = 0; for f in [1.0, 2.0, 3.0] { if f > 1.0 { s += 1; } } s }" => 2;
    singleton_inclusive_range: "fn main() -> i32 { let mut s = 0; for i in 7..=7 { s += i; } s }" => 7;
    reversed_inclusive_range: "fn main() -> i32 { let mut s = 0; for i in 5..=2 { s += 1; } s }" => 0;
    nested_loop_values: "fn main() -> i32 { loop { let x = loop { break 6; }; break x + 1; } }" => 7;
    if_else_value_in_loop: "fn main() -> i32 { let mut s = 0; for i in 0..3 { let x = if i == 1 { 10 } else { 2 }; s += x; } s }" => 14;
    unit_break_value: "fn main() -> i32 { let mut x = 0; loop { break { x = 7; }; } x }" => 7;
    while_condition: "fn main() -> i32 { let mut i = 0; let mut s = 0; while i < 5 { s += i; i += 1; } s }" => 10;
    exclusive_range: "fn main() -> i32 { let mut s = 0; for i in 0..5 { s += i; } s }" => 10;
    inclusive_range: "fn main() -> i32 { let mut s = 0; for i in 0..=5 { s += i; } s }" => 15;
    array_iteration: "fn main() -> i32 { let mut s = 0; for i in [1, 2, 3,] { s += i; } s }" => 6;
    repeated_array: "fn main() -> i32 { let mut s = 0; for i in [3; 4] { s += i; } s }" => 12;
    empty_array: "fn main() -> i32 { let mut s = 0; for i in [] { s += 1; } s }" => 0;
    zero_repeat: "fn main() -> i32 { let mut s = 0; for i in [1; 0] { s += 1; } s }" => 0;
    nested_arrays: "fn main() -> i32 { let mut s = 0; for a in [[1, 2], [3, 4]] { for i in a { s += i; } } s }" => 10;
    for_continue: "fn main() -> i32 { let mut s = 0; for i in 0..6 { if i == 2 { continue; } s += i; } s }" => 13;
    for_break: "fn main() -> i32 { let mut s = 0; for i in 0..6 { if i == 3 { break; } s += i; } s }" => 3;
    nested_break: "fn main() -> i32 { let mut s = 0; for i in 0..3 { for j in 0..5 { if j == 2 { break; } s += 1; } } s }" => 6;
    loop_value: "fn main() -> i32 { let x = loop { break 42; }; x }" => 42;
    loop_continue: "fn main() -> i32 { let mut i = 0; loop { i += 1; if i < 3 { continue; } break i; } }" => 3;
    while_continue: "fn main() -> i32 { let mut i = 0; let mut s = 0; while i < 5 { i += 1; if i == 2 { continue; } s += i; } s }" => 13;
    return_from_loop: "fn main() -> i32 { loop { return 42; } }" => 42;
    bounds_evaluated_once: "fn main() -> i32 { let mut end = 5; let mut s = 0; for i in 0..end { end = 0; s += i; } s }" => 10;
    array_evaluated_once: "fn main() -> i32 { let mut a = [1, 2]; let mut s = 0; for i in a { a = [9, 9]; s += i; } s }" => 3;
    mutable_binding: "fn main() -> i32 { let mut s = 0; for mut i in 0..3 { i = 100; s += i; } s }" => 300;
    shadowed_binding: "fn main() -> i32 { let i = 99; let mut s = 0; for i in 0..3 { s += i; } s + i }" => 102;
    wildcard_binding: "fn main() -> i32 { let mut s = 0; for _ in [1, 2, 3] { s += 1; } s }" => 3;
    negative_range: "fn main() -> i32 { let mut s = 0; for i in -3..=3 { s += i; } s }" => 0;
    signed_minimum: "fn main() -> i32 { let mut s = 0; for i in -128i8..=-127i8 { s += 1; } s }" => 2;
    inclusive_maximum: "fn main() -> i32 { let mut s = 0; for i in 254u8..=255u8 { s += 1; } s }" => 2;
    inclusive_u64_maximum: "fn main() -> i32 { let mut s = 0; for i in 18446744073709551615u64..=18446744073709551615u64 { s += 1; } s }" => 1;
    empty_range: "fn main() -> i32 { let mut s = 0; for i in 255u8..255u8 { s += 1; } s }" => 0;
    reversed_range: "fn main() -> i32 { let mut s = 0; for i in 5..2 { s += 1; } s }" => 0;
    stored_range: "fn main() -> i32 { let r = 0..3; let mut s = 0; for i in r { s += i; } s }" => 3;
    function_parameter: "fn main() -> i32 { sum(5) } fn sum(n: i32) -> i32 { let mut s = 0; for i in 0..n { s += i; } s }" => 10;
}

macro_rules! diagnostic_tests {
    ($($name:ident: $source:literal => $message:literal;)*) => {
        $(#[test]
        fn $name() {
            let mut engine = MusubuEngine::new();
            assert!(!compile_with_filename(&mut engine, "tests/invalid.msb", $source));
            let error = engine.compile_error();
            assert!(error.contains($message), "{error}");
            assert!(error.contains("--> tests/invalid.msb:1:"), "{error}");
            assert!(error.contains($source), "{error}");
            assert!(error.contains('^'), "{error}");
        })*
    };
}

diagnostic_tests! {
    break_outside_loop: "fn main() { break; }" => "only allowed inside a loop";
    continue_outside_loop: "fn main() { continue; }" => "only allowed inside a loop";
    nested_function_continue: "fn main() { loop { fn nested() { continue; } break; } }" => "only allowed inside a loop";
    negative_repeat: "fn main() { let a = [1; -1]; }" => "array repeat count";
    overflowing_repeat: "fn main() { let a = [1; 4294967296u64]; }" => "array repeat count";
    non_iterable: "fn main() { for x in 1 { } }" => "is not iterable";
    mixed_range_types: "fn main() { for x in 0i8..3i32 { } }" => "type mismatch";
    float_range: "fn main() { for x in 0.0..3.0 { } }" => "range endpoints must be integers";
    mixed_array_types: "fn main() { for x in [1, true] { } }" => "type mismatch";
    binding_outside_loop: "fn main() { for x in 0..3 { } x; }" => "cannot resolve path `x`";
    while_break_value: "fn main() { while true { break 1; } }" => "only `loop` may return a value";
    for_break_value: "fn main() { for x in 0..3 { break 1; } }" => "only `loop` may return a value";
    mixed_break_types: "fn main() { let x = loop { if true { break 1; } break true; }; }" => "type mismatch";
    nested_function_break: "fn main() { loop { fn nested() { break; } break; } }" => "only allowed inside a loop";
    nested_function_capture: "fn main() { let x = 1; fn nested() { x; } }" => "cannot resolve path `x`";
    missing_for_binding: "fn main() { for in 0..3 { } }" => "binding pattern";
    non_literal_repeat: "fn main() { let n = 3; for x in [1; n] { } }" => "array repeat count";
}

#[test]
fn ffi_null_arguments_are_rejected() {
    use core::ptr::{null, null_mut};
    let mut engine = MusubuEngine::new();
    assert!(!crate::init(null_mut()));
    crate::uninit(null_mut());
    assert!(!crate::compile(null_mut(), null(), 0));
    assert!(!crate::compile(&mut engine, null(), 0));
    assert!(
        engine
            .compile_error()
            .contains("source pointer must not be null")
    );
    // Null arguments are explicitly accepted by these APIs as invalid input.
    unsafe {
        let mut len = 99;
        assert!(crate::get_compile_error(null(), &mut len).is_null());
        assert_eq!(len, 0);
        assert!(crate::get_compile_error(&engine, null_mut()).is_null());
        assert!(!crate::compile_with_filename(
            null_mut(),
            null(),
            0,
            null(),
            0
        ));
        let valid = "fn main() {}";
        assert!(!crate::compile_with_filename(
            &mut engine,
            valid.as_ptr().cast(),
            valid.len(),
            null(),
            0,
        ));
        assert!(engine.compile_error().contains("must not be null"));
    }
}

#[test]
fn ffi_invalid_utf8_is_rejected() {
    let mut engine = MusubuEngine::new();
    let invalid = [0xffu8];
    assert!(!crate::compile(
        &mut engine,
        invalid.as_ptr().cast(),
        invalid.len()
    ));
    assert!(engine.compile_error().contains("source is not valid UTF-8"));
    let source = "fn main() {}";
    // Both input slices and the engine are valid for the duration of the call.
    unsafe {
        assert!(!crate::compile_with_filename(
            &mut engine,
            source.as_ptr().cast(),
            source.len(),
            invalid.as_ptr().cast(),
            invalid.len(),
        ));
    }
    assert!(
        engine
            .compile_error()
            .contains("filename is not valid UTF-8")
    );
}

#[test]
fn ffi_engine_lifecycle_and_empty_diagnostic() {
    let mut engine = core::ptr::null_mut();
    assert!(crate::init(&mut engine));
    assert!(!engine.is_null());
    let mut len = 99;
    // init returned a live engine; it is destroyed only after the borrowed result is checked.
    unsafe {
        assert!(crate::get_compile_error(engine, &mut len).is_null());
    }
    crate::uninit(engine);
    assert_eq!(len, 0);
}

#[test]
fn later_failure_replaces_previous_diagnostic() {
    let mut engine = MusubuEngine::new();
    assert!(!compile_with_filename(
        &mut engine,
        "first.msb",
        "fn main() { missing; }"
    ));
    assert!(engine.compile_error().contains("first.msb"));
    assert!(!compile_with_filename(
        &mut engine,
        "second.msb",
        "fn main() { break; }"
    ));
    assert!(engine.compile_error().contains("second.msb"));
    assert!(!engine.compile_error().contains("first.msb"));
    assert!(!engine.compile_error().contains("missing"));
}

#[test]
fn ffi_diagnostic_is_readable_and_cleared_after_success() {
    let mut engine = MusubuEngine::new();
    let source = "fn main() {\n    for x in 1 { }\n}";
    let filename = "tests/ffi.msb";
    // All buffers and the engine remain live throughout each FFI call.
    unsafe {
        assert!(!crate::compile_with_filename(
            &mut engine,
            source.as_ptr().cast(),
            source.len(),
            filename.as_ptr().cast(),
            filename.len(),
        ));
        let mut len = 0;
        let error = crate::get_compile_error(&engine, &mut len);
        assert!(!error.is_null());
        let bytes = core::slice::from_raw_parts(error.cast::<u8>(), len);
        let diagnostic = core::str::from_utf8(bytes).unwrap();
        assert_eq!(diagnostic, engine.compile_error());
        assert!(diagnostic.contains("--> tests/ffi.msb:2:"), "{diagnostic}");
        assert!(diagnostic.contains("for x in 1 { }"), "{diagnostic}");
        assert!(diagnostic.contains('^'), "{diagnostic}");

        let valid = "fn main() {}";
        assert!(crate::compile(
            &mut engine,
            valid.as_ptr().cast(),
            valid.len()
        ));
        assert!(crate::get_compile_error(&engine, &mut len).is_null());
        assert_eq!(len, 0);
    }
}
