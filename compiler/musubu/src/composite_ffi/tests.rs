use super::*;
use alloc::vec;
use musubu_driver::compile;

mod buffer_and_id;
mod failure_contracts;
mod matrix_properties;
mod performance;

#[derive(Debug)]
struct Owned(*mut MusubuValueHandle);
impl Drop for Owned {
    fn drop(&mut self) {
        // Each helper transfers exactly one newly allocated handle to this owner.
        unsafe {
            value_free(self.0);
        }
    }
}

fn engine(source: &str) -> MusubuEngine {
    let mut engine = MusubuEngine::new();
    assert!(compile(&mut engine, source), "{}", engine.compile_error());
    engine
}

fn receive(
    engine: &mut MusubuEngine,
    operation: impl FnOnce(*mut MusubuEngine, *mut *mut MusubuValueHandle) -> bool,
) -> Result<Owned, String> {
    let mut output = ptr::null_mut();
    if operation(engine, &mut output) {
        assert!(!output.is_null());
        Ok(Owned(output))
    } else {
        assert!(output.is_null());
        Err(String::from(engine.runtime_error()))
    }
}

fn pointers(values: &[&Owned]) -> Vec<*const MusubuValueHandle> {
    values.iter().map(|v| v.0.cast_const()).collect()
}

fn scalar(engine: &mut MusubuEngine, value: MusubuValue) -> Owned {
    // Fixtures pass live, aligned objects throughout each FFI call.
    receive(engine, |e, out| unsafe {
        value_create_scalar(e, &value, out)
    })
    .unwrap()
}

fn integer(n: i64) -> MusubuValue {
    MusubuValue {
        kind: MusubuValue::I32,
        signed_value: n,
        ..MusubuValue::default()
    }
}

fn vector(engine: &mut MusubuEngine, values: &[MusubuValue]) -> Owned {
    receive(engine, |e, out| unsafe {
        value_create_vector(e, values.as_ptr(), values.len(), out)
    })
    .unwrap()
}

fn matrix(engine: &mut MusubuEngine, columns: &[&Owned]) -> Owned {
    let columns = pointers(columns);
    receive(engine, |e, out| unsafe {
        value_create_matrix(e, columns.as_ptr(), columns.len(), out)
    })
    .unwrap()
}

fn enumeration(engine: &mut MusubuEngine, ty: &str, variant: &str, fields: &[&Owned]) -> Owned {
    let fields = pointers(fields);
    receive(engine, |e, out| unsafe {
        value_create_enum(
            e,
            ty.as_ptr().cast(),
            ty.len(),
            variant.as_ptr().cast(),
            variant.len(),
            fields.as_ptr(),
            fields.len(),
            out,
        )
    })
    .unwrap()
}

fn call(engine: &mut MusubuEngine, name: &str, args: &[&Owned]) -> Result<Owned, String> {
    let args = pointers(args);
    receive(engine, |e, out| unsafe {
        call_function_value(
            e,
            name.as_ptr().cast(),
            name.len(),
            args.as_ptr(),
            args.len(),
            out,
        )
    })
}

fn element(engine: &mut MusubuEngine, value: &Owned, index: usize) -> Owned {
    receive(engine, |e, out| unsafe {
        value_get_element(e, value.0, index, out)
    })
    .unwrap()
}

fn read_scalar(engine: &mut MusubuEngine, value: &Owned) -> MusubuValue {
    let mut output = MusubuValue::default();
    assert!(
        unsafe { value_get_scalar(engine, value.0, &mut output) },
        "{}",
        engine.runtime_error()
    );
    output
}

fn read_vector(engine: &mut MusubuEngine, value: &Owned) -> Vec<MusubuValue> {
    assert_eq!(unsafe { value_get_kind(value.0) }, MusubuValue::VECTOR);
    (0..unsafe { value_get_length(value.0) })
        .map(|i| {
            let component = element(engine, value, i);
            read_scalar(engine, &component)
        })
        .collect()
}

fn assert_vector(engine: &mut MusubuEngine, value: &Owned, expected: &[i64]) {
    let actual = read_vector(engine, value);
    assert!(actual.iter().all(|v| v.kind == MusubuValue::I32));
    assert_eq!(
        actual.iter().map(|v| v.signed_value).collect::<Vec<_>>(),
        expected
    );
}

fn enum_names(value: &Owned) -> (String, String) {
    // Borrowed bytes are copied while the handle remains live.
    unsafe {
        let mut length = 0;
        let pointer = value_get_enum_type(value.0, &mut length);
        assert!(!pointer.is_null());
        let name = String::from(
            str::from_utf8(slice::from_raw_parts(pointer.cast::<u8>(), length)).unwrap(),
        );
        let pointer = value_get_enum_variant(value.0, &mut length);
        assert!(!pointer.is_null());
        let variant = String::from(
            str::from_utf8(slice::from_raw_parts(pointer.cast::<u8>(), length)).unwrap(),
        );
        (name, variant)
    }
}

#[test]
fn vectors_are_passed_computed_and_returned_without_mutating_inputs() {
    let mut engine = engine("fn scale(mut v: vec3i32, n: i32) -> vec3i32 { v.x += 1; v * n }");
    let v = vector(&mut engine, &[integer(1), integer(2), integer(3)]);
    let n = scalar(&mut engine, integer(4));
    let result = call(&mut engine, "scale", &[&v, &n]).unwrap();
    assert_vector(&mut engine, &result, &[8, 8, 12]);
    assert_vector(&mut engine, &v, &[1, 2, 3]);
}

#[test]
fn every_numeric_vector_type_roundtrips() {
    for (ty, kind) in [
        ("i8", MusubuValue::I8),
        ("i16", MusubuValue::I16),
        ("i32", MusubuValue::I32),
        ("i64", MusubuValue::I64),
        ("u8", MusubuValue::U8),
        ("u16", MusubuValue::U16),
        ("u32", MusubuValue::U32),
        ("u64", MusubuValue::U64),
        ("f32", MusubuValue::F32),
        ("f64", MusubuValue::F64),
    ] {
        let mut engine = engine(&format!("fn f(v: vec2{ty}) -> vec2{ty} {{ v }}"));
        let input = MusubuValue {
            kind,
            signed_value: -7,
            unsigned_value: 9,
            float_value: 1.25,
        };
        let value = vector(&mut engine, &[input, input]);
        let output = call(&mut engine, "f", &[&value]).unwrap();
        for component in read_vector(&mut engine, &output) {
            assert_eq!(component.kind, kind);
            if ty.starts_with('i') {
                assert_eq!(component.signed_value, -7);
            } else if ty.starts_with('u') {
                assert_eq!(component.unsigned_value, 9);
            } else {
                assert_eq!(component.float_value, 1.25);
            }
        }
    }
}

#[test]
fn rectangular_matrix_product_checks_all_columns_and_rows() {
    let mut engine = engine("fn multiply(a: mat2x3i32, b: mat3x2i32) -> mat3x3i32 { a * b }");
    let a0 = vector(&mut engine, &[integer(1), integer(2), integer(3)]);
    let a1 = vector(&mut engine, &[integer(4), integer(5), integer(6)]);
    let a = matrix(&mut engine, &[&a0, &a1]);
    let b0 = vector(&mut engine, &[integer(7), integer(8)]);
    let b1 = vector(&mut engine, &[integer(9), integer(10)]);
    let b2 = vector(&mut engine, &[integer(11), integer(12)]);
    let b = matrix(&mut engine, &[&b0, &b1, &b2]);
    let result = call(&mut engine, "multiply", &[&a, &b]).unwrap();
    assert_eq!(unsafe { value_get_kind(result.0) }, MusubuValue::MATRIX);
    assert_eq!(unsafe { value_get_length(result.0) }, 3);
    for (i, expected) in [[39, 54, 69], [49, 68, 87], [59, 82, 105]]
        .iter()
        .enumerate()
    {
        let column = element(&mut engine, &result, i);
        assert_vector(&mut engine, &column, expected);
    }
}

#[test]
fn matrix_vector_product_returns_vector() {
    let mut engine = engine("fn f(m: mat2x2i32, v: vec2i32) -> vec2i32 { m * v }");
    let c0 = vector(&mut engine, &[integer(1), integer(2)]);
    let c1 = vector(&mut engine, &[integer(3), integer(4)]);
    let m = matrix(&mut engine, &[&c0, &c1]);
    let result = call(&mut engine, "f", &[&m, &c0]).unwrap();
    assert_vector(&mut engine, &result, &[7, 10]);
}

#[test]
fn matrix_float64_preserves_precision() {
    let mut engine = engine("fn f(m: mat1x1f64) -> mat1x1f64 { m * 2.0f64 }");
    let component = MusubuValue {
        kind: MusubuValue::F64,
        float_value: 16777217.0,
        ..MusubuValue::default()
    };
    let column = vector(&mut engine, &[component]);
    let matrix = matrix(&mut engine, &[&column]);
    let output = call(&mut engine, "f", &[&matrix]).unwrap();
    let column = element(&mut engine, &output, 0);
    let components = read_vector(&mut engine, &column);
    assert_eq!(components[0].kind, MusubuValue::F64);
    assert_eq!(components[0].float_value, 33554434.0);
}

#[test]
fn unit_enum_input_and_returned_variant() {
    let mut engine = engine(
        "enum State { Off, On } fn flip(s: State) -> State { match s { State::Off => State::On, State::On => State::Off } }",
    );
    let value = enumeration(&mut engine, "State", "Off", &[]);
    let output = call(&mut engine, "flip", &[&value]).unwrap();
    assert_eq!(unsafe { value_get_kind(output.0) }, MusubuValue::ENUM);
    assert_eq!(unsafe { value_get_length(output.0) }, 0);
    assert_eq!(
        enum_names(&output),
        (String::from("State"), String::from("On"))
    );
    let again = call(&mut engine, "flip", &[&output]).unwrap();
    assert_eq!(enum_names(&again).1, "Off");
}

#[test]
fn tuple_enum_fields_roundtrip() {
    let mut engine = engine(
        "enum E { V(i32, bool) } fn f(e: E) -> E { match e { E::V(n,b) => E::V(n + 1,b) } }",
    );
    let n = scalar(&mut engine, integer(4));
    let b = scalar(
        &mut engine,
        MusubuValue {
            kind: MusubuValue::BOOL,
            unsigned_value: 1,
            ..MusubuValue::default()
        },
    );
    let value = enumeration(&mut engine, "E", "V", &[&n, &b]);
    let output = call(&mut engine, "f", &[&value]).unwrap();
    assert_eq!(unsafe { value_get_length(output.0) }, 2);
    let n = element(&mut engine, &output, 0);
    let b = element(&mut engine, &output, 1);
    assert_eq!(read_scalar(&mut engine, &n).signed_value, 5);
    assert_eq!(read_scalar(&mut engine, &b).unsigned_value, 1);
}

#[test]
fn named_enum_fields_use_declaration_order() {
    let mut engine = engine(
        "enum E { V { z: i32, a: bool } } fn f(e: E) -> E { match e { E::V { z, a } => E::V { a, z: z + 2 } } }",
    );
    let z = scalar(&mut engine, integer(7));
    let a = scalar(
        &mut engine,
        MusubuValue {
            kind: MusubuValue::BOOL,
            unsigned_value: 1,
            ..MusubuValue::default()
        },
    );
    let value = enumeration(&mut engine, "E", "V", &[&z, &a]);
    let output = call(&mut engine, "f", &[&value]).unwrap();
    let z = element(&mut engine, &output, 0);
    let a = element(&mut engine, &output, 1);
    assert_eq!(read_scalar(&mut engine, &z).signed_value, 9);
    assert_eq!(read_scalar(&mut engine, &a).kind, MusubuValue::BOOL);
}

#[test]
fn nested_enum_vector_and_matrix_payloads() {
    let mut engine = engine(
        "enum Inner { V(vec2i32) } enum Outer { Data(Inner, mat1x2i32) } fn f(e: Outer) -> Outer { match e { Outer::Data(Inner::V(v), m) => Outer::Data(Inner::V(v * 2), m * 3) } }",
    );
    let v = vector(&mut engine, &[integer(2), integer(3)]);
    let m = matrix(&mut engine, &[&v]);
    let inner = enumeration(&mut engine, "Inner", "V", &[&v]);
    let outer = enumeration(&mut engine, "Outer", "Data", &[&inner, &m]);
    let output = call(&mut engine, "f", &[&outer]).unwrap();
    let inner = element(&mut engine, &output, 0);
    let v = element(&mut engine, &inner, 0);
    assert_vector(&mut engine, &v, &[4, 6]);
    let m = element(&mut engine, &output, 1);
    let column = element(&mut engine, &m, 0);
    assert_vector(&mut engine, &column, &[6, 9]);
}

#[test]
fn enum_names_preserve_utf8() {
    let mut engine = engine("enum 状態 { 有効 } fn make() -> 状態 { 状態::有効 }");
    let value = call(&mut engine, "make", &[]).unwrap();
    assert_eq!(
        enum_names(&value),
        (String::from("状態"), String::from("有効"))
    );
}

#[test]
fn children_outlive_parents_and_engines() {
    let mut engine = engine("enum E { V(vec2i32) } fn make() -> E { E::V(vec2i32(2,3)) }");
    let output = call(&mut engine, "make", &[]).unwrap();
    let child = element(&mut engine, &output, 0);
    drop(output);
    drop(engine);
    let mut other = self::engine("fn f(v: vec2i32) -> vec2i32 { v * 2 }");
    assert_vector(&mut other, &child, &[2, 3]);
    let output = call(&mut other, "f", &[&child]).unwrap();
    assert_vector(&mut other, &output, &[4, 6]);
}

#[test]
fn constructors_copy_children() {
    let mut engine = engine("enum E { V(mat1x2i32) } fn f(e: E) -> E { e }");
    let v = vector(&mut engine, &[integer(2), integer(3)]);
    let m = matrix(&mut engine, &[&v]);
    drop(v);
    let e = enumeration(&mut engine, "E", "V", &[&m]);
    drop(m);
    let output = call(&mut engine, "f", &[&e]).unwrap();
    let m = element(&mut engine, &output, 0);
    let c = element(&mut engine, &m, 0);
    assert_vector(&mut engine, &c, &[2, 3]);
}

#[test]
fn shape_width_and_nominal_enum_errors_are_reported() {
    let mut engine =
        engine("enum E { V(i32) } fn e(v: E) -> E { v } fn v(x: vec2i32) {} fn m(x: mat1x2i32) {}");
    let n = scalar(&mut engine, integer(1));
    for (ty, variant, fields, error) in [
        ("F", "V", vec![&n], "expected enum"),
        ("E", "Missing", vec![], "has no variant"),
        ("E", "V", vec![], "expects 1 fields"),
        ("E", "V", vec![&n, &n], "expects 1 fields"),
    ] {
        let value = enumeration(&mut engine, ty, variant, &fields);
        assert!(
            call(&mut engine, "e", &[&value])
                .unwrap_err()
                .contains(error)
        );
    }
    let wrong = scalar(
        &mut engine,
        MusubuValue {
            kind: MusubuValue::BOOL,
            unsigned_value: 1,
            ..MusubuValue::default()
        },
    );
    let value = enumeration(&mut engine, "E", "V", &[&wrong]);
    assert!(
        call(&mut engine, "e", &[&value])
            .unwrap_err()
            .contains("field 0")
    );
    let short = vector(&mut engine, &[integer(1)]);
    assert!(call(&mut engine, "v", &[&short]).is_err());
    let m = matrix(&mut engine, &[&short]);
    assert!(call(&mut engine, "m", &[&m]).is_err());
    let wrong_width = vector(
        &mut engine,
        &[MusubuValue {
            kind: MusubuValue::I16,
            signed_value: 1,
            ..MusubuValue::default()
        }; 2],
    );
    assert!(call(&mut engine, "v", &[&wrong_width]).is_err());
}

#[test]
fn invalid_constructors_do_not_allocate_outputs() {
    let mut engine = engine("");
    for values in [
        vec![],
        vec![
            integer(1),
            MusubuValue {
                kind: MusubuValue::F32,
                ..MusubuValue::default()
            },
        ],
        vec![MusubuValue {
            kind: MusubuValue::BOOL,
            ..MusubuValue::default()
        }],
        vec![MusubuValue {
            kind: 999,
            ..MusubuValue::default()
        }],
    ] {
        assert!(
            receive(&mut engine, |e, out| unsafe {
                value_create_vector(e, values.as_ptr(), values.len(), out)
            })
            .is_err()
        );
    }
    let a = vector(&mut engine, &[integer(1)]);
    let b = vector(&mut engine, &[integer(1), integer(2)]);
    let scalar = scalar(&mut engine, integer(1));
    for columns in [
        vec![],
        vec![a.0.cast_const(), b.0.cast_const()],
        vec![scalar.0.cast_const()],
        vec![ptr::null()],
    ] {
        assert!(
            receive(&mut engine, |e, out| unsafe {
                value_create_matrix(e, columns.as_ptr(), columns.len(), out)
            })
            .is_err()
        );
    }
}

#[test]
fn bounds_and_scalar_accessor_failures_preserve_outputs() {
    let mut engine = engine("");
    let v = vector(&mut engine, &[integer(7)]);
    for index in [1, usize::MAX] {
        assert!(
            receive(&mut engine, |e, out| unsafe {
                value_get_element(e, v.0, index, out)
            })
            .unwrap_err()
            .contains("out of bounds")
        );
    }
    let mut output = integer(99);
    assert!(!unsafe { value_get_scalar(&mut engine, v.0, &mut output) });
    assert_eq!(output.signed_value, 99);
    let component = element(&mut engine, &v, 0);
    assert!(
        receive(&mut engine, |e, out| unsafe {
            value_get_element(e, component.0, 0, out)
        })
        .is_err()
    );
}

#[test]
fn vm_error_and_success_clear_diagnostics() {
    let mut engine = engine("fn f(v: vec1i32, divisor: i32) -> vec1i32 { v / divisor }");
    let v = vector(&mut engine, &[integer(8)]);
    let zero = scalar(&mut engine, integer(0));
    let two = scalar(&mut engine, integer(2));
    assert!(
        call(&mut engine, "f", &[&v, &zero])
            .unwrap_err()
            .contains("DivisionByZero")
    );
    assert_eq!(unsafe { value_get_kind(v.0) }, MusubuValue::VECTOR);
    assert!(!engine.runtime_error().is_empty());
    let output = call(&mut engine, "f", &[&v, &two]).unwrap();
    assert!(engine.runtime_error().is_empty());
    assert_vector(&mut engine, &output, &[4]);
}

#[test]
fn unsupported_enum_payload_signature_is_rejected() {
    let mut engine = engine("enum E { V((i32,)) } fn f() -> E { E::V((1,)) }");
    assert!(
        call(&mut engine, "f", &[])
            .unwrap_err()
            .contains("unsupported FFI signature")
    );
}

#[test]
fn scalar_and_unit_functions_work_through_handle_api() {
    let mut engine = engine("fn add(a: i32) -> i32 { a + 2 } fn empty() {}");
    let n = scalar(&mut engine, integer(5));
    let output = call(&mut engine, "add", &[&n]).unwrap();
    assert_eq!(read_scalar(&mut engine, &output).signed_value, 7);
    let output = call(&mut engine, "empty", &[]).unwrap();
    assert_eq!(unsafe { value_get_kind(output.0) }, MusubuValue::UNIT);
}

#[test]
fn recompile_revalidates_owned_enum_descriptors() {
    let mut engine = engine("enum E { V(i32) } fn f(e: E) -> E { e }");
    let n = scalar(&mut engine, integer(5));
    let e = enumeration(&mut engine, "E", "V", &[&n]);
    let output = call(&mut engine, "f", &[&e]).unwrap();
    assert!(compile(
        &mut engine,
        "enum E { V(bool) } fn f(e: E) -> E { e }"
    ));
    assert!(
        call(&mut engine, "f", &[&output])
            .unwrap_err()
            .contains("field 0")
    );
    assert_eq!(enum_names(&output), (String::from("E"), String::from("V")));
}

#[test]
fn null_lengths_utf8_and_metadata_accessors() {
    let mut engine = engine("fn empty() {}");
    // Only nulls or live fixture pointers are used; oversized counts are rejected before reads.
    unsafe {
        assert_eq!(value_get_kind(ptr::null()), MusubuValue::INVALID);
        assert_eq!(value_get_length(ptr::null()), 0);
        value_free(ptr::null_mut());
        let mut len = 99;
        assert!(value_get_enum_type(ptr::null(), &mut len).is_null());
        assert_eq!(len, 0);
        assert!(value_get_enum_variant(ptr::null(), ptr::null_mut()).is_null());
        assert!(!value_create_scalar(
            &mut engine,
            ptr::null(),
            ptr::null_mut()
        ));
        assert!(
            receive(&mut engine, |e, out| value_create_scalar(
                e,
                ptr::null(),
                out
            ))
            .is_err()
        );
        assert!(
            receive(&mut engine, |e, out| value_create_vector(
                e,
                ptr::null(),
                1,
                out
            ))
            .is_err()
        );
        let value = integer(1);
        assert!(
            receive(&mut engine, |e, out| value_create_vector(
                e,
                &value,
                usize::MAX,
                out
            ))
            .is_err()
        );
        let invalid_utf8 = [255u8];
        assert!(
            receive(&mut engine, |e, out| value_create_enum(
                e,
                invalid_utf8.as_ptr().cast(),
                1,
                c"A".as_ptr(),
                1,
                ptr::null(),
                0,
                out
            ))
            .unwrap_err()
            .contains("UTF-8")
        );
        let mut output = ptr::null_mut();
        assert!(!call_function_value(
            ptr::null_mut(),
            c"empty".as_ptr(),
            5,
            ptr::null(),
            0,
            &mut output
        ));
        let output = receive(&mut engine, |e, out| {
            call_function_value(e, c"empty".as_ptr(), 5, ptr::null(), 0, out)
        })
        .unwrap();
        len = 99;
        assert!(value_get_enum_type(output.0, &mut len).is_null());
        assert_eq!(len, 0);
    }
}
