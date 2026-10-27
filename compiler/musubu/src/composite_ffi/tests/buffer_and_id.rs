use super::*;

#[test]
fn resolved_id_repeated_calls_and_errors() {
    let mut engine = engine("fn f(x: i32) -> i32 { x + 1 }");
    let mut id = usize::MAX;
    unsafe {
        assert!(get_function_id(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            &mut id
        ));
        assert_eq!(id, 0);
        let value = scalar(&mut engine, integer(41));
        let args = pointers(&[&value]);
        for _ in 0..3 {
            let result = receive(&mut engine, |e, out| {
                call_function_value_by_id(e, id, args.as_ptr(), 1, out)
            })
            .unwrap();
            assert_eq!(read_scalar(&mut engine, &result).signed_value, 42);
        }
        assert!(
            receive(&mut engine, |e, out| {
                call_function_value_by_id(e, usize::MAX, args.as_ptr(), 1, out)
            })
            .is_err()
        );
        assert!(
            receive(&mut engine, |e, out| {
                call_function_value_by_id(e, id, ptr::null(), 0, out)
            })
            .is_err()
        );
        assert!(!get_function_id(
            &mut engine,
            b"missing".as_ptr().cast(),
            7,
            &mut id
        ));
        assert_eq!(id, 0);
        assert!(!get_function_id(
            &mut engine,
            [0xff_u8].as_ptr().cast(),
            1,
            &mut id
        ));
        assert!(!get_function_id(&mut engine, ptr::null(), 1, &mut id));
        assert!(!get_function_id(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            ptr::null_mut()
        ));
        assert!(get_function_id(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            &mut id
        ));
        assert!(engine.runtime_error().is_empty());
    }
}

macro_rules! buffer_tests {
    ($roundtrip:ident, $invalid:ident, $scalar:ty, $create:ident, $copy:ident, $source:literal) => {
        #[test]
        fn $roundtrip() {
            let mut engine = engine($source);
            let mut data: [$scalar; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
            let input = receive(&mut engine, |e, out| unsafe {
                $create(e, data.as_ptr(), 2, 3, out)
            })
            .unwrap();
            data.fill(99.0);
            let result = call(&mut engine, "f", &[&input]).unwrap();
            let mut output = [0.0; 6];
            assert!(unsafe { $copy(&mut engine, result.0, output.as_mut_ptr(), 2, 3) });
            assert_eq!(output, [2.0, 4.0, 6.0, 8.0, 10.0, 12.0]);
            assert!(unsafe { $copy(&mut engine, input.0, output.as_mut_ptr(), 2, 3) });
            assert_eq!(output, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
            drop(engine);
            let mut other = MusubuEngine::new();
            assert!(unsafe { $copy(&mut other, result.0, output.as_mut_ptr(), 2, 3) });
            assert_eq!(output, [2.0, 4.0, 6.0, 8.0, 10.0, 12.0]);
        }

        #[test]
        fn $invalid() {
            let mut engine = engine($source);
            let data: [$scalar; 6] = [1.0; 6];
            let value = receive(&mut engine, |e, out| unsafe {
                $create(e, data.as_ptr(), 2, 3, out)
            })
            .unwrap();
            unsafe {
                let sentinel = value.0;
                for (columns, rows) in [(0, 3), (2, 0), (usize::MAX, 2), (2, usize::MAX)] {
                    let mut out = sentinel;
                    assert!(!$create(
                        &mut engine,
                        data.as_ptr(),
                        columns,
                        rows,
                        &mut out
                    ));
                    assert_eq!(out, sentinel);
                }
                let mut out = sentinel;
                assert!(!$create(&mut engine, ptr::null(), 2, 3, &mut out));
                assert_eq!(out, sentinel);
                assert!(!$create(&mut engine, data.as_ptr(), 2, 3, ptr::null_mut()));
                let mut output = [99.0; 6];
                for (columns, rows) in [(3, 2), (1, 3), (2, 2)] {
                    assert!(!$copy(
                        &mut engine,
                        value.0,
                        output.as_mut_ptr(),
                        columns,
                        rows
                    ));
                    assert_eq!(output, [99.0; 6]);
                }
                assert!(!$copy(&mut engine, value.0, ptr::null_mut(), 2, 3));
                assert!(!$copy(&mut engine, ptr::null(), output.as_mut_ptr(), 2, 3));
                let column = vector(&mut engine, &[integer(1), integer(2), integer(3)]);
                let wrong_type = matrix(&mut engine, &[&column, &column]);
                assert!(!$copy(&mut engine, wrong_type.0, output.as_mut_ptr(), 2, 3));
                assert_eq!(output, [99.0; 6]);
                assert!($copy(&mut engine, value.0, output.as_mut_ptr(), 2, 3));
                assert!(engine.runtime_error().is_empty());
            }
        }
    };
}

buffer_tests!(
    f32_rectangular_roundtrip,
    f32_buffer_failures,
    f32,
    value_create_matrix_f32,
    value_copy_matrix_f32,
    "fn f(m: mat2x3) -> mat2x3 { m * 2.0 }"
);
buffer_tests!(
    f64_rectangular_roundtrip,
    f64_buffer_failures,
    f64,
    value_create_matrix_f64,
    value_copy_matrix_f64,
    "fn f(m: mat2x3f64) -> mat2x3f64 { m * 2.0f64 }"
);

#[test]
fn tokens_reject_other_engines_and_replaced_programs() {
    let mut engine = engine("fn f() -> i32 { 1 }");
    let mut token = MusubuFunctionToken::default();
    unsafe {
        assert!(get_function_token(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            &mut token
        ));
        let result = receive(&mut engine, |e, out| {
            call_function_value_by_token(e, &token, ptr::null(), 0, out)
        })
        .unwrap();
        assert_eq!(read_scalar(&mut engine, &result).signed_value, 1);
        let mut other = MusubuEngine::new();
        assert!(compile(&mut other, "fn f() -> i32 { 9 }"));
        assert!(
            receive(&mut other, |e, out| call_function_value_by_token(
                e,
                &token,
                ptr::null(),
                0,
                out
            ))
            .is_err()
        );
        assert!(!compile(&mut engine, "fn f() -> i32 { missing }"));
        assert!(
            receive(&mut engine, |e, out| call_function_value_by_token(
                e,
                &token,
                ptr::null(),
                0,
                out
            ))
            .is_ok()
        );
        assert!(compile(&mut engine, "fn f() -> i32 { 2 }"));
        assert!(
            receive(&mut engine, |e, out| call_function_value_by_token(
                e,
                &token,
                ptr::null(),
                0,
                out
            ))
            .is_err()
        );
        assert!(get_function_token(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            &mut token
        ));
        let result = receive(&mut engine, |e, out| {
            call_function_value_by_token(e, &token, ptr::null(), 0, out)
        })
        .unwrap();
        assert_eq!(read_scalar(&mut engine, &result).signed_value, 2);
        assert!(
            receive(&mut engine, |e, out| call_function_value_by_token(
                e,
                ptr::null(),
                ptr::null(),
                0,
                out
            ))
            .is_err()
        );
    }
}
