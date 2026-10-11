use super::*;

macro_rules! rejected_cases {
    ($($name:ident: $source:literal => $message:literal;)*) => {
        $(#[test]
        fn $name() {
            let mut engine = MusubuEngine::new();
            assert!(!compile_with_filename(&mut engine, "tests/invalid.msb", $source));
            let error = engine.compile_error();
            assert!(error.starts_with("error: "), "{error}");
            assert!(error.contains($message), "{error}");
            assert!(error.contains("--> tests/invalid.msb:"), "{error}");
            assert!(error.contains('^'), "{error}");
        })*
    };
}

rejected_cases! {
    tuple_in_struct_wrong_field_type: "fn main() { S { t: (true,) }; } struct S { t: (i32,) }" => "type mismatch";
    matrix_in_enum_wrong_payload_shape: "fn main() { E::M(mat1x2i32(vec2i32(1,2))); } enum E { M(mat2x1i32) }" => "type mismatch";
    vector_in_tuple_wrong_return: "fn main() -> (vec2i32,) { (vec3i32(1,2,3),) }" => "return type mismatch";
    nested_tuple_duplicate_binding: "fn main() { let ((a,), a) = ((1,), 2); }" => "duplicate tuple binding";
    tuple_field_nested_immutable_vector: "fn main() { let t = (vec2i32(1,2),); t.0.x = 3; }" => "immutable";
    nested_enum_pattern_wrong_type: "fn main() { match E::V(I::A) { E::V(J::A) => {} } } enum E { V(I) } enum I { A } enum J { A }" => "type mismatch";
    enum_tuple_match_missing_combination: "fn main() { match (E::A, E::A, E::A) { (E::A, _, _) => {}, (E::B, E::A, _) => {}, (E::B, E::B, E::A) => {} } } enum E { A, B }" => "non-exhaustive";
    function_argument_nominal_enum_mismatch: "fn main() { take(B::V(1)); } fn take(a: A) {} enum A { V(i32) } enum B { V(i32) }" => "type mismatch";
    range_boolean_endpoints: "fn main() { for x in false..true {} }" => "range endpoints must be integers";
    array_nested_element_mismatch: "fn main() { let a = [(1, true), (2, 3)]; }" => "type mismatch";
    loop_binding_inaccessible_after_nested_loop: "fn main() { for x in 0..1 { for y in 0..1 {} y; } }" => "cannot resolve path `y`";
    matrix_compound_scalar_width_mismatch: "fn main() { let mut m = mat1x1i32(vec1i32(1)); m /= 2i16; }" => "incompatible matrix operation";
}

#[test]
fn malformed_syntax_reports_a_located_error_without_panicking() {
    for source in [
        "fn",
        "fn main(",
        "fn main() {",
        "fn main() { let x = ; }",
        "struct S { x:",
        "enum E { V(",
        "fn main() { match E::A { E::A => } } enum E { A }",
        "fn main() { let x = (1,; }",
        "fn main() { let x = [1,; }",
        "fn main() { let x = vec2(1.0,); }",
        "fn main() { while { } }",
    ] {
        let mut engine = MusubuEngine::new();
        assert!(
            !compile_with_filename(&mut engine, "broken.msb", source),
            "{source}"
        );
        assert!(
            engine.compile_error().contains("--> broken.msb:"),
            "{}",
            engine.compile_error()
        );
        assert!(
            engine.compile_error().contains('^'),
            "{}",
            engine.compile_error()
        );
    }
}

#[test]
fn utf8_diagnostic_ffi_length_is_bytes_and_reads_are_repeatable() {
    let mut engine = MusubuEngine::new();
    assert!(!compile_with_filename(
        &mut engine,
        "src/日本語.msb",
        "fn main() { 未定義; }"
    ));
    for _ in 0..2 {
        let mut length = 0;
        // The engine owns the diagnostic and remains alive and unmodified during the borrow.
        unsafe {
            let pointer = crate::get_compile_error(&engine, &mut length);
            assert!(!pointer.is_null());
            assert_eq!(length, engine.compile_error().len());
            assert!(length > engine.compile_error().chars().count());
            assert_eq!(
                core::slice::from_raw_parts(pointer.cast::<u8>(), length),
                engine.compile_error().as_bytes()
            );
        }
    }
}

#[test]
fn independent_engines_do_not_share_diagnostics() {
    let mut first = MusubuEngine::new();
    let mut second = MusubuEngine::new();
    assert!(!compile(&mut first, "fn main() { missing; }"));
    let saved = alloc::string::String::from(first.compile_error());
    assert!(compile(&mut second, "fn main() -> i32 { 7 }"));
    assert!(second.compile_error().is_empty());
    assert_eq!(first.compile_error(), saved);
}
