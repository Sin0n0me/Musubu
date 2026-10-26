use super::*;
use musubu_driver::compile;

fn engine(source: &str) -> MusubuEngine {
    let mut engine = MusubuEngine::new();
    assert!(compile(&mut engine, source), "{}", engine.compile_error());
    engine
}

fn call(
    engine: &mut MusubuEngine,
    name: &str,
    args: &[MusubuValue],
) -> Result<MusubuValue, String> {
    let mut output = MusubuValue::default();
    // All borrowed inputs and the output are alive, aligned and independent of the engine.
    if unsafe {
        call_function(
            engine,
            name.as_ptr().cast(),
            name.len(),
            args.as_ptr(),
            args.len(),
            &mut output,
        )
    } {
        Ok(output)
    } else {
        Err(String::from(engine.runtime_error()))
    }
}

fn signed(kind: u32, value: i64) -> MusubuValue {
    MusubuValue {
        kind,
        signed_value: value,
        ..MusubuValue::default()
    }
}

fn unsigned(kind: u32, value: u64) -> MusubuValue {
    MusubuValue {
        kind,
        unsigned_value: value,
        ..MusubuValue::default()
    }
}

fn float(kind: u32, value: f64) -> MusubuValue {
    MusubuValue {
        kind,
        float_value: value,
        ..MusubuValue::default()
    }
}

#[test]
fn signed_integer_widths_roundtrip_boundaries() {
    for (name, kind, min, max) in [
        (
            "i8",
            MusubuValue::I8,
            i64::from(i8::MIN),
            i64::from(i8::MAX),
        ),
        (
            "i16",
            MusubuValue::I16,
            i64::from(i16::MIN),
            i64::from(i16::MAX),
        ),
        (
            "i32",
            MusubuValue::I32,
            i64::from(i32::MIN),
            i64::from(i32::MAX),
        ),
        ("i64", MusubuValue::I64, i64::MIN, i64::MAX),
    ] {
        let mut engine = engine(&format!("fn identity(x: {name}) -> {name} {{ x }}"));
        for value in [min, 0, max] {
            let result = call(&mut engine, "identity", &[signed(kind, value)]).unwrap();
            assert_eq!(result.kind, kind);
            assert_eq!(result.signed_value, value);
        }
    }
}

#[test]
fn unsigned_integer_widths_roundtrip_boundaries() {
    for (name, kind, max) in [
        ("u8", MusubuValue::U8, u64::from(u8::MAX)),
        ("u16", MusubuValue::U16, u64::from(u16::MAX)),
        ("u32", MusubuValue::U32, u64::from(u32::MAX)),
        ("u64", MusubuValue::U64, u64::MAX),
    ] {
        let mut engine = engine(&format!("fn identity(x: {name}) -> {name} {{ x }}"));
        for value in [0, max] {
            let result = call(&mut engine, "identity", &[unsigned(kind, value)]).unwrap();
            assert_eq!(result.kind, kind);
            assert_eq!(result.unsigned_value, value);
        }
    }
}

#[test]
fn float_widths_roundtrip_and_arithmetic() {
    for (name, kind) in [("f32", MusubuValue::F32), ("f64", MusubuValue::F64)] {
        let mut engine = engine(&format!(
            "fn identity(x: {name}) -> {name} {{ x }} fn mul(x: {name}, y: {name}) -> {name} {{ x * y }}"
        ));
        for value in [-0.0, 1.25, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let result = call(&mut engine, "identity", &[float(kind, value)]).unwrap();
            assert_eq!(result.kind, kind);
            if value.is_nan() {
                assert!(result.float_value.is_nan());
            } else {
                assert_eq!(result.float_value.to_bits(), value.to_bits());
            }
        }
        assert_eq!(
            call(&mut engine, "mul", &[float(kind, 1.5), float(kind, 2.5)])
                .unwrap()
                .float_value,
            3.75
        );
    }
}

#[test]
fn bool_and_unit_results() {
    let mut engine =
        engine("fn same(a: bool) -> bool { a } fn empty() {} fn unit(x: ()) -> () { x }");
    for value in [0, 1] {
        let result = call(&mut engine, "same", &[unsigned(MusubuValue::BOOL, value)]).unwrap();
        assert_eq!(result.kind, MusubuValue::BOOL);
        assert_eq!(result.unsigned_value, value);
    }
    assert_eq!(
        call(&mut engine, "empty", &[]).unwrap().kind,
        MusubuValue::UNIT
    );
    assert_eq!(
        call(&mut engine, "unit", &[MusubuValue::default()])
            .unwrap()
            .kind,
        MusubuValue::UNIT
    );
}

#[test]
fn name_lookup_recursion_and_multiple_functions() {
    let mut engine = engine(
        "fn unused() -> i32 { 99 } fn factorial(n: i32) -> i32 { if n == 0 { 1 } else { n * factorial(n - 1) } } fn 加算(a: i32, b: i32) -> i32 { a + b }",
    );
    assert_eq!(
        call(&mut engine, "factorial", &[signed(MusubuValue::I32, 6)])
            .unwrap()
            .signed_value,
        720
    );
    assert_eq!(
        call(
            &mut engine,
            "加算",
            &[signed(MusubuValue::I32, 3), signed(MusubuValue::I32, 4)]
        )
        .unwrap()
        .signed_value,
        7
    );
    assert!(
        call(&mut engine, "missing", &[])
            .unwrap_err()
            .contains("undefined function")
    );
}

#[test]
fn wrong_argument_counts_types_tags_and_ranges_fail() {
    let mut engine = engine("fn f(x: i8) -> i8 { x }");
    assert!(
        call(&mut engine, "f", &[])
            .unwrap_err()
            .contains("expects 1 arguments")
    );
    assert!(
        call(&mut engine, "f", &[signed(MusubuValue::I8, 1); 10])
            .unwrap_err()
            .contains("expects 1 arguments")
    );
    for value in [
        signed(MusubuValue::I8, 128),
        signed(MusubuValue::I8, -129),
        signed(MusubuValue::I32, 1),
        signed(999, 1),
    ] {
        assert!(call(&mut engine, "f", &[value]).is_err());
    }
    let mut engine = self::engine(
        "fn b(x: bool) -> bool { x } fn f(x: f32) -> f32 { x } fn u(x: u8) -> u8 { x }",
    );
    assert!(
        call(&mut engine, "b", &[unsigned(MusubuValue::BOOL, 2)])
            .unwrap_err()
            .contains("bool must")
    );
    assert!(
        call(&mut engine, "f", &[float(MusubuValue::F32, f64::MAX)])
            .unwrap_err()
            .contains("out of range")
    );
    assert!(
        call(&mut engine, "u", &[unsigned(MusubuValue::U8, 256)])
            .unwrap_err()
            .contains("out of range")
    );
}

#[test]
fn unsupported_signatures_fail_before_execution() {
    let mut engine = engine("fn output() -> (i32,) { (1 / 0,) } fn input(v: vec2i32) {} ");
    for name in ["output", "input"] {
        assert!(
            call(&mut engine, name, &[])
                .unwrap_err()
                .contains("signatures")
        );
    }
}

#[test]
fn runtime_errors_are_readable_and_cleared_on_success() {
    let mut engine = engine("fn divide(a: i32, b: i32) -> i32 { a / b }");
    assert!(
        call(
            &mut engine,
            "divide",
            &[signed(MusubuValue::I32, 1), signed(MusubuValue::I32, 0)]
        )
        .unwrap_err()
        .contains("InvalidOperand")
    );
    let mut length = 0;
    // The diagnostic is borrowed only while the engine is unmodified.
    unsafe {
        let error = get_runtime_error(&engine, &mut length);
        assert!(!error.is_null());
        assert_eq!(
            slice::from_raw_parts(error.cast::<u8>(), length),
            engine.runtime_error().as_bytes()
        );
    }
    assert_eq!(
        call(
            &mut engine,
            "divide",
            &[signed(MusubuValue::I32, 8), signed(MusubuValue::I32, 2)]
        )
        .unwrap()
        .signed_value,
        4
    );
    unsafe {
        assert!(get_runtime_error(&engine, &mut length).is_null());
    }
    assert_eq!(length, 0);
}

#[test]
fn recompile_updates_signature_and_failed_compile_keeps_executable() {
    let mut engine = engine("fn f() -> i32 { 1 }");
    assert_eq!(call(&mut engine, "f", &[]).unwrap().signed_value, 1);
    assert!(compile(&mut engine, "fn f(x: i32) -> i32 { x + 2 }"));
    assert!(call(&mut engine, "f", &[]).is_err());
    assert!(!compile(&mut engine, "fn f(x: i32) -> i32 { missing }"));
    let error = String::from(engine.compile_error());
    assert_eq!(
        call(&mut engine, "f", &[signed(MusubuValue::I32, 3)])
            .unwrap()
            .signed_value,
        5
    );
    assert_eq!(engine.compile_error(), error);
}

#[test]
fn successful_recompile_replaces_whole_program_and_failed_resolution_is_atomic() {
    let mut engine = engine("fn f() -> i32 { 1 } fn g() -> i32 { f() + 1 }");
    assert_eq!(call(&mut engine, "g", &[]).unwrap().signed_value, 2);
    assert!(!compile(&mut engine, "fn new_name() -> i32 { missing }"));
    assert_eq!(call(&mut engine, "g", &[]).unwrap().signed_value, 2);
    assert!(engine.get_function_id("musubu::new_name").is_none());
    assert!(compile(&mut engine, "fn f() -> bool { true }"));
    assert!(call(&mut engine, "g", &[]).is_err());
    assert_eq!(call(&mut engine, "f", &[]).unwrap().unsigned_value, 1);
    assert!(compile(&mut engine, "fn new_name() -> i32 { 8 }"));
    assert_eq!(engine.get_function_id("musubu::new_name"), Some(0));
    assert!(call(&mut engine, "f", &[]).is_err());
    assert_eq!(call(&mut engine, "new_name", &[]).unwrap().signed_value, 8);
    assert!(compile(&mut engine, ""));
    assert!(call(&mut engine, "new_name", &[]).is_err());
}

#[test]
fn null_and_invalid_inputs_leave_output_unchanged() {
    let mut engine = engine("fn f() -> i32 { 7 }");
    let mut output = signed(MusubuValue::I32, 99);
    let name = b"f";
    // Every non-null pointer references a live object; invalid lengths are rejected before reading.
    unsafe {
        assert!(!call_function(
            ptr::null_mut(),
            name.as_ptr().cast(),
            1,
            ptr::null(),
            0,
            &mut output
        ));
        assert!(!call_function(
            &mut engine,
            ptr::null(),
            0,
            ptr::null(),
            0,
            &mut output
        ));
        assert!(!call_function(
            &mut engine,
            name.as_ptr().cast(),
            1,
            ptr::null(),
            1,
            &mut output
        ));
        assert!(!call_function(
            &mut engine,
            name.as_ptr().cast(),
            1,
            ptr::null(),
            0,
            ptr::null_mut()
        ));
        assert!(!call_function(
            &mut engine,
            name.as_ptr().cast(),
            usize::MAX,
            ptr::null(),
            0,
            &mut output
        ));
        let invalid_utf8 = [255u8];
        assert!(!call_function(
            &mut engine,
            invalid_utf8.as_ptr().cast(),
            1,
            ptr::null(),
            0,
            &mut output
        ));
        assert!(engine.runtime_error().contains("UTF-8"));
        assert_eq!(output.signed_value, 99);
        assert!(call_function(
            &mut engine,
            name.as_ptr().cast(),
            1,
            ptr::null(),
            0,
            &mut output
        ));
        assert_eq!(output.signed_value, 7);
        let mut length = 99;
        assert!(get_runtime_error(ptr::null(), &mut length).is_null());
        assert_eq!(length, 0);
        assert!(get_runtime_error(&engine, ptr::null_mut()).is_null());
    }
}

#[test]
fn ffi_engine_compile_call_and_destroy_lifecycle() {
    let mut engine = ptr::null_mut();
    // The output is writable and owns no existing engine.
    assert!(unsafe { crate::init(&mut engine) });
    let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let args = [signed(MusubuValue::I32, 3), signed(MusubuValue::I32, 4)];
    let mut output = MusubuValue::default();
    // init owns the engine until uninit; all input buffers and output remain live.
    unsafe {
        assert!(crate::compile(engine, source.as_ptr().cast(), source.len()));
        assert!(call_function(
            engine,
            c"add".as_ptr(),
            3,
            args.as_ptr(),
            args.len(),
            &mut output
        ));
        crate::uninit(engine);
    }
    assert_eq!(output.kind, MusubuValue::I32);
    assert_eq!(output.signed_value, 7);
}

#[test]
fn floating_precision_matches_the_selected_width() {
    let mut engine = engine("fn single(x: f32) -> f32 { x } fn double(x: f64) -> f64 { x }");
    // 2^24 + 1 is representable in f64; f32 rounds it to 2^24.
    let value = 16777217.0;
    assert_eq!(
        call(&mut engine, "single", &[float(MusubuValue::F32, value)])
            .unwrap()
            .float_value,
        16777216.0
    );
    assert_eq!(
        call(&mut engine, "double", &[float(MusubuValue::F64, value)])
            .unwrap()
            .float_value,
        value
    );
}

#[test]
fn result_is_owned_by_caller_and_errors_are_engine_local() {
    let mut first = engine("fn value() -> i32 { 7 }");
    let mut second = engine("fn value() -> i32 { 9 }");
    let result = call(&mut first, "value", &[]).unwrap();
    assert!(call(&mut first, "未定義", &[]).is_err());
    let saved = String::from(first.runtime_error());
    assert_eq!(call(&mut second, "value", &[]).unwrap().signed_value, 9);
    assert_eq!(first.runtime_error(), saved);
    drop(first);
    assert_eq!(result.kind, MusubuValue::I32);
    assert_eq!(result.signed_value, 7);
}

#[test]
fn vm_bounds_and_overflow_failures_leave_output_unchanged() {
    let mut engine = engine(
        "fn bounds(i: i32) -> i32 { let v = vec1i32(3); v[i] } fn overflow(i: i32) -> i32 { i + 1 }",
    );
    for (name, value, error) in [
        ("bounds", 1, "IndexOutOfBounds"),
        ("overflow", i32::MAX, "InvalidOperand"),
    ] {
        let args = [signed(MusubuValue::I32, i64::from(value))];
        let mut output = signed(MusubuValue::I32, 99);
        // Inputs and output are distinct live values; the engine is borrowed exclusively.
        unsafe {
            assert!(!call_function(
                &mut engine,
                name.as_ptr().cast(),
                name.len(),
                args.as_ptr(),
                1,
                &mut output
            ));
        }
        assert_eq!(output.signed_value, 99);
        assert!(engine.runtime_error().contains(error));
    }
}

#[test]
fn execution_limits_stop_loops_and_recursion_and_allow_recovery() {
    let mut engine = engine(
        "fn spin() { loop {} } fn recurse(n: i32) -> i32 { if n == 0 { 1 } else { recurse(n - 1) } } fn ok() -> i32 { 7 }",
    );
    unsafe {
        assert!(crate::set_execution_limits(&mut engine, 1000, 8));
        assert!(!crate::set_execution_limits(&mut engine, 0, 8));
        assert!(!crate::set_execution_limits(&mut engine, 1000, 0));
        assert!(!crate::set_execution_limits(ptr::null_mut(), 1000, 8));
    }
    assert!(
        call(&mut engine, "spin", &[])
            .unwrap_err()
            .contains("InstructionLimitExceeded")
    );
    assert!(
        call(&mut engine, "recurse", &[signed(MusubuValue::I32, -1)])
            .unwrap_err()
            .contains("StackOverflow")
    );
    assert_eq!(
        call(&mut engine, "recurse", &[signed(MusubuValue::I32, 6)])
            .unwrap()
            .signed_value,
        1
    );
    assert_eq!(call(&mut engine, "ok", &[]).unwrap().signed_value, 7);
    assert!(compile(&mut engine, "fn spin() { loop {} }"));
    assert!(
        call(&mut engine, "spin", &[])
            .unwrap_err()
            .contains("InstructionLimitExceeded")
    );
}
