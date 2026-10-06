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
    enum_subject_once: "fn main() -> i32 { let mut n = 0; let r = match { n += 1; E::V(n) } { E::A => 0, E::V(x) => x }; n * 10 + r } enum E { A, V(i32) }" => 11;
    enum_named_evaluation_order: "fn main() -> i32 { let mut n = 0; let v = E::V { y: { n += 1; n }, x: { n += 1; n } }; match v { E::V { x, y } => x * 10 + y } } enum E { V { x: i32, y: i32 } }" => 21;
    enum_match_shadow: "fn main() -> i32 { let x = 3; let n = match E::V(7) { E::V(x) => x }; x * 10 + n } enum E { V(i32) }" => 37;
    enum_match_mut_named: "fn main() -> i32 { match (E::V { x: 3 }) { E::V { mut x } => { x += 4; x } } } enum E { V { x: i32 } }" => 7;
    enum_payload_wildcard: "fn main() -> i32 { match E::V(1, 7) { E::V(_, n) => n } } enum E { V(i32, i32) }" => 7;
    enum_nested_coverage_product: "fn main() -> i32 { match E::Pair(I::B, I::B) { E::Pair(I::A, _) => 1, E::Pair(I::B, I::A) => 2, E::Pair(I::B, I::B) => 7 } } enum E { Pair(I, I) } enum I { A, B }" => 7;
    enum_match_in_condition: "fn main() -> i32 { if match E::A { E::A => { let e = E::V { x: 1 }; true }, E::V { .. } => false } { 7 } else { 0 } } enum E { A, V { x: i32 } }" => 7;
    enum_match_break_value: "fn main() -> i32 { loop { match E::V(7) { E::A => break 0, E::V(n) => break n } } } enum E { A, V(i32) }" => 7;
    enum_return_from_match: "fn main() -> i32 { match make(E::A) { E::A => 0, E::V(n) => n } } fn make(e: E) -> E { match e { E::A => return E::V(7), E::V(n) => return E::V(n) } } enum E { A, V(i32) }" => 7;
    enum_empty_type_match: "fn main() -> i32 { 7 } fn impossible(e: Empty) -> E { match e {} } enum Empty {} enum E { A }" => 7;
    enum_trailing_commas: "fn main() -> i32 { match E::V(7,) { E::V(n,) => { n }, } } enum E { V(i32,), }" => 7;
    enum_unit: "fn main() -> i32 { let c = Color::Blue; match c { Color::Red => 1, Color::Blue => 2 } } enum Color { Red, Blue }" => 2;
    enum_tuple: "fn main() -> i32 { let v = Number::Value(7); match v { Number::None => 0, Number::Value(x) => x } } enum Number { None, Value(i32) }" => 7;
    enum_named: "fn main() -> i32 { let v = Number::Pair { y: 4, x: 3 }; match v { Number::Pair { x, y } => x * 10 + y } } enum Number { Pair { x: i32, y: i32 } }" => 34;
    enum_function: "fn main() -> i32 { read(make(7)) } fn make(x: i32) -> Number { Number::Value(x) } fn read(n: Number) -> i32 { match n { Number::None => 0, Number::Value(x) => x } } enum Number { None, Value(i32) }" => 7;
    enum_explicit_return: "fn main() -> i32 { match make() { E::V(x) => x } } fn make() -> E { return E::V(7); } enum E { V(i32) }" => 7;
    enum_match_wildcard: "fn main() -> i32 { match E::B { E::A => 1, _ => 7 } } enum E { A, B, C }" => 7;
    enum_match_binding: "fn main() -> i32 { match E::B { v => match v { E::A => 0, E::B => 7 } } } enum E { A, B }" => 7;
    enum_named_rest: "fn main() -> i32 { match (E::V { x: 7, y: 2 }) { E::V { x: n, .. } => n } } enum E { V { x: i32, y: i32 } }" => 7;
    enum_nested_pattern: "fn main() -> i32 { match Outer::V(Inner::B(7)) { Outer::V(Inner::A) => 0, Outer::V(Inner::B(x)) => x } } enum Outer { V(Inner) } enum Inner { A, B(i32) }" => 7;
    enum_copy_payload: "fn main() -> i32 { let e = E::V(Point { x: 3 }); let n = match e { E::V(mut p) => { p.x = 8; p.x } }; match e { E::V(p) => p.x * 10 + n } } enum E { V(Point) } struct Point { x: i32 }" => 38;
    enum_in_struct: "fn main() -> i32 { let mut p = Point { e: E::A }; p.e = E::B(7); match p.e { E::A => 0, E::B(n) => n } } struct Point { e: E } enum E { A, B(i32) }" => 7;
    enum_array: "fn main() -> i32 { let mut s = 0; for e in [E::A, E::B(7)] { s += match e { E::A => 1, E::B(n) => n }; } s } enum E { A, B(i32) }" => 8;
    enum_match_returns: "fn main() -> i32 { match E::B(7) { E::A => return 0, E::B(n) => return n, } } enum E { A, B(i32) }" => 7;
    enum_match_mixed_return: "fn main() -> i32 { match E::B(7) { E::A => return 0, E::B(n) => n } } enum E { A, B(i32) }" => 7;
    enum_match_continue: "fn main() -> i32 { let mut s = 0; for e in [E::A, E::B(7)] { let x = match e { E::A => continue, E::B(n) => n }; s += x; } s } enum E { A, B(i32) }" => 7;
    enum_reassignment: "fn main() -> i32 { let mut e: E = E::A; e = E::B(7); match e { E::A => 0, E::B(n) => n } } enum E { A, B(i32) }" => 7;
    enum_empty_shapes: "fn main() -> i32 { let a = E::T(); let b = E::S {}; match a { E::U => 0, E::T() => match b { E::U => 0, E::T() => 1, E::S {} => 7 }, E::S {} => 2 } } enum E { U, T(), S {} }" => 7;
    enum_multi_payload: "fn main() -> i32 { match E::V(3, 4) { E::V(x, y) => x * 10 + y } } enum E { V(i32, i32) }" => 34;
    struct_type_namespace: "fn main() -> i32 { let Point = 9; let p: Point = Point { x: Point }; p.x } struct Point { x: i32 }" => 9;
    struct_operand_snapshot: "fn main() -> i32 { let mut p = Point { x: 3 }; let q = Pair { x: p.x, y: { p.x = 7; p.x } }; q.x + q.y } struct Point { x: i32 } struct Pair { x: i32, y: i32 }" => 10;
    struct_infinite_function: "fn main() -> i32 { 7 } fn forever() -> Point { loop {} } struct Point { x: i32 }" => 7;
    struct_return_from_loop: "fn main() -> i32 { make().x } fn make() -> Point { loop { return Point { x: 7 }; } } struct Point { x: i32 }" => 7;
    struct_bool_field: "fn main() -> i32 { let mut p = Flag { active: false }; p.active = true; if p.active { 7 } else { 0 } } struct Flag { active: bool }" => 7;
    struct_return_branches: "fn main() -> i32 { make(true).x } fn make(b: bool) -> Point { if b { return Point { x: 7 }; } else { return Point { x: 0 }; } } struct Point { x: i32 }" => 7;
    struct_initializer_evaluation_order: "fn main() -> i32 { let mut i = 0; let p = Point { y: { i += 1; i }, x: { i += 1; i } }; p.x * 10 + p.y } struct Point { x: i32, y: i32 }" => 21;
    struct_nested_copy: "fn main() -> i32 { let p = Outer { p: Point { x: 3 } }; let mut q = p; q.p.x = 8; p.p.x * 10 + q.p.x } struct Outer { p: Point } struct Point { x: i32 }" => 38;
    struct_mutable_parameter: "fn main() -> i32 { let p = Point { x: 3 }; let q = change(p); p.x * 10 + q.x } fn change(mut p: Point) -> Point { p.x = 8; p } struct Point { x: i32 }" => 38;
    struct_replace_nested: "fn main() -> i32 { let mut o = Outer { p: Point { x: 1 } }; o.p = Point { x: 7 }; o.p.x } struct Outer { p: Point } struct Point { x: i32 }" => 7;
    struct_literal_in_for_array: "fn main() -> i32 { let mut s = 0; for p in [Point { x: 2 }, Point { x: 3 }] { s += p.x; } s } struct Point { x: i32 }" => 5;
    struct_literal_in_condition_call: "fn main() -> i32 { if positive(Point { x: 1 }) { 7 } else { 0 } } fn positive(p: Point) -> bool { p.x > 0 } struct Point { x: i32 }" => 7;
    struct_literal_in_parentheses: "fn main() -> i32 { if (Point { x: 1 }).x == 1 { 7 } else { 0 } } struct Point { x: i32 }" => 7;
    struct_loop_value: "fn main() -> i32 { let p = loop { break Point { x: 7 }; }; p.x } struct Point { x: i32 }" => 7;
    struct_repeat_copy: "fn main() -> i32 { let mut s = 0; for mut p in [Point { x: 2 }; 3] { p.x += 1; s += p.x; } s } struct Point { x: i32 }" => 9;
    struct_literal: "fn main() -> i32 { let p = Point { x: 3, y: 4 }; p.x + p.y } struct Point { x: i32, y: i32 }" => 7;
    struct_field_order: "fn main() -> i32 { let p = Point { y: 4, x: 3 }; p.x * 10 + p.y } struct Point { x: i32, y: i32 }" => 34;
    struct_mutation: "fn main() -> i32 { let mut p = Point { x: 3 }; p.x = 7; p.x += 2; p.x } struct Point { x: i32 }" => 9;
    struct_nested: "fn main() -> i32 { let mut o = Outer { p: Point { x: 3 } }; o.p.x += 4; o.p.x } struct Outer { p: Point } struct Point { x: i32 }" => 7;
    struct_copy: "fn main() -> i32 { let p = Point { x: 3 }; let mut q = p; q.x = 8; p.x * 10 + q.x } struct Point { x: i32 }" => 38;
    struct_functions: "fn main() -> i32 { let p: Point = make(7); sum(p) } fn make(x: i32) -> Point { Point { x } } fn sum(p: Point) -> i32 { p.x } struct Point { x: i32 }" => 7;
    struct_explicit_return: "fn main() -> i32 { make().x } fn make() -> Point { return Point { x: 9 }; } struct Point { x: i32 }" => 9;
    struct_empty: "fn main() -> i32 { let e = Empty {}; consume(e); 7 } fn consume(e: Empty) {} struct Empty {}" => 7;
    struct_loop_condition: "fn main() -> i32 { let mut p = Point { x: 0 }; while p.x < 3 { p.x += 1; } if p.x == 3 { 7 } else { 0 } } struct Point { x: i32 }" => 7;
    struct_array: "fn main() -> i32 { let a = [Point { x: 2 }, Point { x: 3 }]; let mut s = 0; for p in a { s += p.x; } s } struct Point { x: i32 }" => 5;
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
    enum_missing_match_arms: "fn main() { match E::A {} } enum E { A }" => "non-exhaustive";
    enum_unreachable_nested: "fn main() { match E::V(I::A) { E::V(_) => {}, E::V(I::A) => {} } } enum E { V(I) } enum I { A, B }" => "unreachable match arm";
    enum_missing_product_case: "fn main() { match E::V(I::A, I::B) { E::V(I::A, _) => {}, E::V(I::B, I::A) => {} } } enum E { V(I, I) } enum I { A, B }" => "non-exhaustive";
    enum_pattern_unknown_field: "fn main() { match (E::V { x: 1 }) { E::V { y } => {} } } enum E { V { x: i32 } }" => "has no field";
    enum_pattern_duplicate_field: "fn main() { match (E::V { x: 1 }) { E::V { x: a, x: b } => {} } } enum E { V { x: i32 } }" => "duplicate pattern field";
    enum_pattern_missing_field: "fn main() { match E::V(1, 2) { E::V(x) => {} } } enum E { V(i32, i32) }" => "pattern expects 2 fields";
    enum_pattern_wrong_shape: "fn main() { match E::V(1) { E::V => {} } } enum E { V(i32) }" => "wrong pattern shape";
    enum_constructor_wrong_shape: "fn main() { E::A(); } enum E { A }" => "not a tuple variant";
    enum_binding_immutable: "fn main() { match E::V(1) { E::V(x) => { x = 2; } } } enum E { V(i32) }" => "immutable";
    enum_wrong_return: "fn f() -> E { F::A } enum E { A } enum F { A }" => "return";
    enum_missing_return: "fn f() -> E {} enum E { A }" => "return";
    enum_not_enum_match: "fn main() { match 1 { _ => {} } }" => "match requires an enum";
    enum_nonexhaustive: "fn main() { match E::A { E::A => {} } } enum E { A, B }" => "non-exhaustive";
    enum_unreachable: "fn main() { match E::A { _ => {}, E::A => {} } } enum E { A }" => "unreachable match arm";
    enum_unknown_variant: "fn main() { E::Missing; } enum E { A }" => "has no variant";
    enum_duplicate_variant: "enum E { A, A }" => "duplicate";
    enum_wrong_payload: "fn main() { E::V(true); } enum E { V(i32) }" => "type mismatch";
    enum_wrong_arity: "fn main() { E::V(1, 2); } enum E { V(i32) }" => "expects 1 fields";
    enum_missing_payload: "fn main() { E::V; } enum E { V(i32) }" => "requires a payload";
    enum_missing_named_field: "fn main() { E::V {}; } enum E { V { x: i32 } }" => "missing field";
    enum_unknown_named_field: "fn main() { E::V { y: 1 }; } enum E { V { x: i32 } }" => "has no field";
    enum_duplicate_named_field: "fn main() { E::V { x: 1, x: 2 }; } enum E { V { x: i32 } }" => "initialized more than once";
    enum_wrong_pattern_type: "fn main() { match E::A { F::A => {} } } enum E { A } enum F { A }" => "type mismatch";
    enum_wrong_arm_type: "fn main() { let x = match E::A { E::A => 1, E::B => true }; } enum E { A, B }" => "type mismatch";
    enum_pattern_scope: "fn main() { match E::V(1) { E::V(x) => {} } x; } enum E { V(i32) }" => "cannot resolve path `x`";
    enum_duplicate_binding: "fn main() { match E::V(1, 2) { E::V(x, x) => {} } } enum E { V(i32, i32) }" => "duplicate pattern binding";
    enum_nested_nonexhaustive: "fn main() { match O::V(I::A) { O::V(I::A) => {} } } enum O { V(I) } enum I { A, B }" => "non-exhaustive";
    enum_recursive: "enum E { V(E) }" => "infinite size";
    enum_struct_recursive: "struct S { e: E } enum E { V(S) }" => "infinite size";
    enum_nominal: "fn main() { let mut x = E::A; x = F::A; } enum E { A } enum F { A }" => "type mismatch";
    struct_variable_is_not_type: "fn main() { let p = 1; let q: p = 2; }" => "cannot determine type";
    struct_break_before_return: "fn make() -> A { loop { break; return A { x: 1 }; } } struct A { x: i32 }" => "return";
    struct_missing_return: "fn make() -> A { } struct A { x: i32 }" => "return";
    struct_wrong_argument: "fn main() { take(B { x: 1 }); } fn take(a: A) {} struct A { x: i32 } struct B { x: i32 }" => "type mismatch";
    struct_wrong_implicit_return: "fn make() -> A { B { x: 1 } } struct A { x: i32 } struct B { x: i32 }" => "return";
    struct_wrong_explicit_return: "fn make() -> A { return B { x: 1 }; } struct A { x: i32 } struct B { x: i32 }" => "return";
    struct_wrong_annotation: "fn main() { let a: A = B { x: 1 }; } struct A { x: i32 } struct B { x: i32 }" => "type mismatch";
    struct_nested_immutable: "fn main() { let o = Outer { p: Point { x: 1 } }; o.p.x = 2; } struct Outer { p: Point } struct Point { x: i32 }" => "immutable";
    struct_duplicate_definition_field: "struct Point { x: i32, x: i32 }" => "duplicate";
    struct_mutual_recursion: "struct A { b: B } struct B { a: A }" => "infinite size";
    struct_unknown_field_type: "struct A { x: Missing }" => "cannot determine type";
    struct_missing_field: "fn main() { let p = Point {}; } struct Point { x: i32 }" => "missing field `x`";
    struct_unknown_field: "fn main() { let p = Point { y: 1 }; } struct Point { x: i32 }" => "has no field `y`";
    struct_duplicate_initializer: "fn main() { let p = Point { x: 1, x: 2 }; } struct Point { x: i32 }" => "initialized more than once";
    struct_field_type: "fn main() { let p = Point { x: true }; } struct Point { x: i32 }" => "type mismatch";
    struct_read_unknown: "fn main() { let p = Point { x: 1 }; p.y; } struct Point { x: i32 }" => "has no field `y`";
    struct_recursive: "struct Point { x: Point }" => "infinite size";
    struct_immutable: "fn main() { let p = Point { x: 1 }; p.x = 2; } struct Point { x: i32 }" => "immutable";
    struct_nominal_types: "fn main() { let mut a = A { x: 1 }; a = B { x: 2 }; } struct A { x: i32 } struct B { x: i32 }" => "type mismatch";
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
