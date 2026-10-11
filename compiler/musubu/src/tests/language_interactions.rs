use super::*;

macro_rules! execution_cases {
    ($($name:ident: $source:literal => $expected:expr;)*) => {
        $(#[test] fn $name() { assert_result($source, $expected); })*
    };
}

execution_cases! {
    recursive_factorial: "fn main() -> i32 { factorial(6) } fn factorial(n: i32) -> i32 { if n == 0 { 1 } else { let previous = factorial(n - 1); n * previous } }" => 720;
    mutual_recursion: "fn main() -> i32 { if even(12) && odd(11) { 1 } else { 0 } } fn even(n: i32) -> bool { if n == 0 { true } else { odd(n - 1) } } fn odd(n: i32) -> bool { if n == 0 { false } else { even(n - 1) } }" => 1;
    nested_calls_keep_arguments: "fn main() -> i32 { combine(inc(1), inc(inc(2)), inc(3)) } fn inc(x: i32) -> i32 { x + 1 } fn combine(a: i32, b: i32, c: i32) -> i32 { a * 100 + b * 10 + c }" => 244;
    call_argument_order: "fn main() -> i32 { let mut n = 0; encode({ n += 1; n }, { n += 1; n }, { n += 1; n }) } fn encode(a: i32, b: i32, c: i32) -> i32 { a * 100 + b * 10 + c }" => 123;
    function_mutation_is_local_to_copy: "fn main() -> i32 { let s = S { x: 3 }; let n = change(s); s.x * 10 + n } fn change(mut s: S) -> i32 { s.x = 8; s.x } struct S { x: i32 }" => 38;
    recursive_tuple_return: "fn main() -> i32 { let (a, b) = pair(4); a * 10 + b } fn pair(n: i32) -> (i32, i32) { if n == 0 { (2, 3) } else { pair(n - 1) } }" => 23;
    repeated_array_initializer_runs_once: "fn main() -> i32 { let mut n = 0; let a = [{ n += 1; n }; 3]; let mut sum = 0; for x in a { sum += x; } n * 10 + sum }" => 13;
    empty_repeated_array_still_evaluates_initializer: "fn main() -> i32 { let mut n = 0; let a = [{ n += 1; n }; 0]; for x in a { n += 100; } n }" => 1;
    stored_array_can_be_iterated_twice: "fn main() -> i32 { let a = [2, 3]; let mut sum = 0; for x in a { sum += x; } for x in a { sum += x; } sum }" => 10;
    stored_range_can_be_iterated_twice: "fn main() -> i32 { let r = 1..=3; let mut sum = 0; for x in r { sum += x; } for x in r { sum += x; } sum }" => 12;
    loop_binding_copy_does_not_change_array: "fn main() -> i32 { let a = [S { x: 2 }, S { x: 3 }]; for mut s in a { s.x = 99; } let mut sum = 0; for s in a { sum += s.x; } sum } struct S { x: i32 }" => 5;
    nested_tuple_copy_is_independent: "fn main() -> i32 { let a = ((2, 3), 4); let mut b = a; b.0.1 = 9; a.0.1 * 10 + b.0.1 }" => 39;
    tuple_destructuring_preserves_source: "fn main() -> i32 { let t = (S { x: 2 },); let (mut s,) = t; s.x = 9; t.0.x * 10 + s.x } struct S { x: i32 }" => 29;
    match_scrutinee_evaluated_once: "fn main() -> i32 { let mut n = 0; let result = match ({ n += 1; E::B(7) }) { E::A => 0, E::B(x) => x }; n * 10 + result } enum E { A, B(i32) }" => 17;
    match_unselected_arm_has_no_side_effects: "fn main() -> i32 { let mut n = 0; match E::B { E::A => { n += 100; }, E::B => { n += 3; } }; n } enum E { A, B }" => 3;
    enum_payload_mutation_preserves_source: "fn main() -> i32 { let e = E::V(S { x: 2 }); let a = match e { E::V(mut s) => { s.x = 9; s.x } }; let b = match e { E::V(s) => s.x }; a * 10 + b } enum E { V(S) } struct S { x: i32 }" => 92;
    enum_array_nested_tuple_payload: "fn main() -> i32 { let mut sum = 0; for e in [E::V((2, 3)), E::None, E::V((4, 5))] { sum += match e { E::None => 0, E::V((a,b)) => a + b }; } sum } enum E { None, V((i32, i32)) }" => 14;
    match_break_and_continue_in_for: "fn main() -> i32 { let mut sum = 0; for e in [E::Skip, E::Add(3), E::Stop, E::Add(99)] { match e { E::Skip => { continue; }, E::Stop => { break; }, E::Add(n) => { sum += n; } }; } sum } enum E { Skip, Stop, Add(i32) }" => 3;
    nested_function_calls_inside_loop: "fn main() -> i32 { let mut sum = 0; for i in 0..4 { sum += twice(i); } sum } fn twice(n: i32) -> i32 { let mut sum = 0; for i in 0..2 { sum += n; } sum }" => 12;
    return_from_nested_loop_restores_caller: "fn main() -> i32 { find() + 5 } fn find() -> i32 { for i in 1..4 { for j in 1..4 { if i + j == 5 { return i * 10 + j; } } } 0 }" => 28;
    vector_in_struct_function_copy: "fn main() -> i32 { let s = S { v: vec2i32(2,3) }; let t = scale(s); s.v.x * 100 + t.v.x * 10 + t.v.y } fn scale(mut s: S) -> S { s.v *= 2; s } struct S { v: vec2i32 }" => 246;
    matrix_tuple_enum_function_roundtrip: "fn main() -> i32 { match make() { E::V((m,n)) => m[1][0] + n } } fn make() -> E { E::V((mat2x1i32(vec1i32(2),vec1i32(5)), 3)) } enum E { V((mat2x1i32, i32)) }" => 8;
    unicode_binding_and_function: "fn main() -> i32 { let 数値 = 4; 加算(数値) } fn 加算(入力: i32) -> i32 { 入力 + 3 }" => 7;
}

#[test]
fn repeated_engine_execution_has_fresh_locals_and_iterators() {
    let mut engine = MusubuEngine::new();
    assert!(
        compile(
            &mut engine,
            "fn main(n: i32) -> i32 { let mut sum = 0; for i in 0..n { sum += i; } sum }"
        ),
        "{}",
        engine.compile_error()
    );
    for (input, expected) in [(4, 6), (0, 0), (2, 1), (4, 6)] {
        let result = engine
            .run_function(0, vec![Value::Integer(Integer::Int32(input))])
            .unwrap();
        assert!(matches!(result, Some(Value::Integer(Integer::Int32(n))) if n == expected));
    }
}

#[test]
fn unknown_function_id_returns_specific_error() {
    let engine = MusubuEngine::new();
    assert!(matches!(
        engine.run_function(999, vec![]),
        Err(musubu_vm::errors::VMError::IllegalFunctionCall)
    ));
}
