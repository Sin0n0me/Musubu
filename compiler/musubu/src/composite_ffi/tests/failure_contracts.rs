use super::*;

#[test]
fn scalar_getter_rejects_aggregates_without_changing_output() {
    let mut engine = MusubuEngine::new();
    let vector = vector(&mut engine, &[integer(1), integer(2)]);
    let matrix = matrix(&mut engine, &[&vector]);
    let enumeration = enumeration(&mut engine, "E", "V", &[&matrix]);
    for handle in [&vector, &matrix, &enumeration] {
        let mut output = integer(123);
        assert!(!unsafe { value_get_scalar(&mut engine, handle.0, &mut output) });
        assert_eq!(output.kind, MusubuValue::I32);
        assert_eq!(output.signed_value, 123);
        assert_eq!(output.unsigned_value, 0);
        assert_eq!(output.float_value, 0.0);
        assert_eq!(engine.runtime_error(), "value is not a scalar");
    }
}

#[test]
fn token_validation_is_read_only_and_tracks_program_lifetime() {
    let mut engine = engine("fn f() { loop {} }");
    let other = MusubuEngine::new();
    let mut token = MusubuFunctionToken::default();
    assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
    engine.set_runtime_error(String::from("preserved runtime diagnostic"));
    engine.set_compile_error(String::from("preserved compile diagnostic"));
    let missing = MusubuFunctionToken {
        function: usize::MAX,
        ..token
    };
    unsafe {
        assert!(is_function_token_valid(&engine, &token));
        assert!(!is_function_token_valid(&other, &token));
        assert!(!is_function_token_valid(&engine, &missing));
        assert!(!is_function_token_valid(
            &engine,
            &MusubuFunctionToken::default()
        ));
        assert!(!is_function_token_valid(ptr::null(), &token));
        assert!(!is_function_token_valid(&engine, ptr::null()));
        assert!(!is_function_token_valid(ptr::null(), ptr::null()));
    }
    assert_eq!(engine.runtime_error(), "preserved runtime diagnostic");
    assert_eq!(engine.compile_error(), "preserved compile diagnostic");
    assert!(!compile(&mut engine, "fn f("));
    let diagnostic = String::from(engine.compile_error());
    assert!(unsafe { is_function_token_valid(&engine, &token) });
    assert_eq!(engine.compile_error(), diagnostic);
    assert!(compile(&mut engine, "fn f() {}"));
    assert!(!unsafe { is_function_token_valid(&engine, &token) });
    assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
    assert!(unsafe { is_function_token_valid(&engine, &token) });
}

#[test]
fn rejected_data_flow_keeps_previous_program_and_token() {
    let mut engine = engine("fn f() -> i32 { 41 }");
    let mut token = MusubuFunctionToken::default();
    assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
    for source in [
        "fn f() -> i32 {}",
        "fn f() -> i32 { if true { 42 } }",
        "fn f() -> i32 { let x: i32; x }",
    ] {
        assert!(!unsafe { crate::compile(&mut engine, source.as_ptr().cast(), source.len()) });
        let mut length = 0;
        let diagnostic = unsafe { crate::get_compile_error(&engine, &mut length) };
        assert!(!diagnostic.is_null());
        let diagnostic = unsafe { slice::from_raw_parts(diagnostic.cast::<u8>(), length) };
        let diagnostic = str::from_utf8(diagnostic).unwrap();
        assert!(diagnostic.contains("error:"));
        assert!(diagnostic.contains("<input>:1:"));
        assert!(diagnostic.contains('^'));
        let result = receive(&mut engine, |e, out| unsafe {
            call_function_value_by_token(e, &token, ptr::null(), 0, out)
        })
        .unwrap();
        assert_eq!(read_scalar(&mut engine, &result).signed_value, 41);
    }
}

#[test]
fn failed_calls_preserve_existing_output_and_arguments_for_all_dispatch_modes() {
    let mut engine = engine("fn f(x: i32) -> i32 { 12 / x }");
    let sentinel = scalar(&mut engine, integer(123));
    let zero = scalar(&mut engine, integer(0));
    let wrong = vector(&mut engine, &[integer(1), integer(2)]);
    let mut id = usize::MAX;
    let mut token = MusubuFunctionToken::default();
    unsafe {
        assert!(get_function_id(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            &mut id
        ));
        assert!(get_function_token(
            &mut engine,
            b"f".as_ptr().cast(),
            1,
            &mut token
        ));
    }
    let cases = [
        vec![],
        pointers(&[&zero]),
        pointers(&[&wrong]),
        pointers(&[&zero, &zero]),
        vec![ptr::null()],
    ];
    for mode in 0..3 {
        for args in &cases {
            let mut output = sentinel.0;
            let success = unsafe {
                match mode {
                    0 => call_function_value(
                        &mut engine,
                        b"f".as_ptr().cast(),
                        1,
                        args.as_ptr(),
                        args.len(),
                        &mut output,
                    ),
                    1 => call_function_value_by_id(
                        &mut engine,
                        id,
                        args.as_ptr(),
                        args.len(),
                        &mut output,
                    ),
                    _ => call_function_value_by_token(
                        &mut engine,
                        &token,
                        args.as_ptr(),
                        args.len(),
                        &mut output,
                    ),
                }
            };
            assert!(!success, "mode={mode} count={}", args.len());
            assert_eq!(output, sentinel.0);
            assert!(!engine.runtime_error().is_empty());
            assert_eq!(read_scalar(&mut engine, &sentinel).signed_value, 123);
            assert_eq!(read_scalar(&mut engine, &zero).signed_value, 0);
            assert_vector(&mut engine, &wrong, &[1, 2]);
        }
    }
    let divisor = scalar(&mut engine, integer(3));
    let result = call(&mut engine, "f", &[&divisor]).unwrap();
    assert!(engine.runtime_error().is_empty());
    assert_eq!(read_scalar(&mut engine, &result).signed_value, 4);
}

#[test]
fn failed_token_lookups_preserve_both_output_fields() {
    let mut engine = engine("fn f() {}");
    let mut token = MusubuFunctionToken {
        program: 123,
        function: 456,
    };
    for name in [&b"missing"[..], &b""[..], &[0xff][..]] {
        assert!(!unsafe {
            get_function_token(&mut engine, name.as_ptr().cast(), name.len(), &mut token)
        });
        assert_eq!((token.program, token.function), (123, 456));
        assert!(!engine.runtime_error().is_empty());
    }
    assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
    assert!(engine.runtime_error().is_empty());
    receive(&mut engine, |e, out| unsafe {
        call_function_value_by_token(e, &token, ptr::null(), 0, out)
    })
    .unwrap();
}

#[test]
fn tokens_survive_each_failed_compile_stage_but_never_successful_replacement() {
    let mut engine = engine("fn f() -> i32 { 41 } fn removed() {}");
    let mut token = MusubuFunctionToken::default();
    assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
    for source in ["fn f() { @ }", "fn f( {", "fn f() -> i32 { missing }"] {
        assert!(!compile(&mut engine, source));
        let diagnostic = String::from(engine.compile_error());
        assert!(!diagnostic.is_empty());
        let value = receive(&mut engine, |e, out| unsafe {
            call_function_value_by_token(e, &token, ptr::null(), 0, out)
        })
        .unwrap();
        assert_eq!(read_scalar(&mut engine, &value).signed_value, 41);
        assert_eq!(engine.compile_error(), diagnostic);
    }
    for expected in [42, 43, 44] {
        assert!(compile(
            &mut engine,
            &format!("fn f() -> i32 {{ {expected} }}")
        ));
        assert!(engine.compile_error().is_empty());
        assert!(
            receive(&mut engine, |e, out| unsafe {
                call_function_value_by_token(e, &token, ptr::null(), 0, out)
            })
            .is_err()
        );
        assert!(call(&mut engine, "removed", &[]).is_err());
        assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
        let value = receive(&mut engine, |e, out| unsafe {
            call_function_value_by_token(e, &token, ptr::null(), 0, out)
        })
        .unwrap();
        assert_eq!(read_scalar(&mut engine, &value).signed_value, expected);
    }
}

#[test]
fn failed_matrix_copies_do_not_partially_write_destination() {
    let mut engine = MusubuEngine::new();
    let a = receive(&mut engine, |e, out| unsafe {
        value_create_matrix_f32(e, [1.0_f32; 6].as_ptr(), 2, 3, out)
    })
    .unwrap();
    let b = receive(&mut engine, |e, out| unsafe {
        value_create_matrix_f64(e, [2.0_f64; 6].as_ptr(), 2, 3, out)
    })
    .unwrap();
    for (columns, rows) in [(2, 3), (3, 2), (0, 3), (5, 3)] {
        // Buffers cover even rejected dimensions; failure must preserve every element.
        let mut out32 = [123.0_f32; 17];
        let mut out64 = [456.0_f64; 17];
        assert!(!unsafe {
            value_copy_matrix_f32(&mut engine, b.0, out32.as_mut_ptr().add(1), columns, rows)
        });
        assert!(!unsafe {
            value_copy_matrix_f64(&mut engine, a.0, out64.as_mut_ptr().add(1), columns, rows)
        });
        if (columns, rows) != (2, 3) {
            assert!(!unsafe {
                value_copy_matrix_f32(&mut engine, a.0, out32.as_mut_ptr().add(1), columns, rows)
            });
            assert!(!unsafe {
                value_copy_matrix_f64(&mut engine, b.0, out64.as_mut_ptr().add(1), columns, rows)
            });
        }
        assert_eq!(out32, [123.0; 17]);
        assert_eq!(out64, [456.0; 17]);
    }
}
