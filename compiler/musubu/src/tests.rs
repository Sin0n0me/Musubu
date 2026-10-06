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
