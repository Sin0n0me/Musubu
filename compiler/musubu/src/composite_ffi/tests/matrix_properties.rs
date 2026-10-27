use super::*;

// The oracle uses integer row-major matrices; conversion to column-major floats
// happens only at the FFI boundary. Each input is divided by 4, so A*B is /16.
fn samples(rows: usize, columns: usize, seed: usize) -> Vec<i32> {
    (0..rows * columns)
        .map(|i| ((i * 7 + seed * 3) % 17) as i32 - 8)
        .collect()
}

fn expected_product(a: &[i32], b: &[i32], rows: usize, inner: usize, columns: usize) -> Vec<f64> {
    let mut result = vec![0.0; rows * columns];
    for row in 0..rows {
        for column in 0..columns {
            // C[row,column] = sum_k A[row,k] * B[k,column] / 16.
            let sum: i32 = (0..inner)
                .map(|k| a[row * inner + k] * b[k * columns + column])
                .sum();
            result[row * columns + column] = f64::from(sum) / 16.0;
        }
    }
    result
}

macro_rules! matrix_properties {
    ($product:ident, $special:ident, $ty:ty, $suffix:literal, $create:ident, $copy:ident) => {
        #[test]
        fn $product() {
            for rows in 1..=4 { for inner in 1..=4 { for columns in 1..=4 {
                let mut engine = engine(&format!("fn f(a: mat{inner}x{rows}{suffix}, b: mat{columns}x{inner}{suffix}) -> mat{columns}x{rows}{suffix} {{ a * b }}", suffix = $suffix));
                let mut token = MusubuFunctionToken::default();
                let mut id = usize::MAX;
                assert!(unsafe { get_function_token(&mut engine, b"f".as_ptr().cast(), 1, &mut token) });
                assert!(unsafe { get_function_id(&mut engine, b"f".as_ptr().cast(), 1, &mut id) });
                for seed in 0..3 {
                    let a = samples(rows, inner, seed);
                    let b = samples(inner, columns, seed + 5);
                    let pack = |values: &[i32], rows: usize, columns: usize| -> Vec<$ty> {
                        (0..columns).flat_map(|c| (0..rows).map(move |r| values[r * columns + c] as $ty / 4.0)).collect()
                    };
                    let a_buffer = pack(&a, rows, inner);
                    let b_buffer = pack(&b, inner, columns);
                    let a_handle = receive(&mut engine, |e, out| unsafe { $create(e, a_buffer.as_ptr(), inner, rows, out) }).unwrap();
                    let b_handle = receive(&mut engine, |e, out| unsafe { $create(e, b_buffer.as_ptr(), columns, inner, out) }).unwrap();
                    let args = pointers(&[&a_handle, &b_handle]);
                    let result = receive(&mut engine, |e, out| unsafe {
                        match seed {
                            0 => call_function_value(e, b"f".as_ptr().cast(), 1, args.as_ptr(), 2, out),
                            1 => call_function_value_by_id(e, id, args.as_ptr(), 2, out),
                            _ => call_function_value_by_token(e, &token, args.as_ptr(), 2, out),
                        }
                    }).unwrap();
                    assert_eq!(unsafe { value_get_kind(result.0) }, MusubuValue::MATRIX);
                    assert_eq!(unsafe { value_get_length(result.0) }, columns);
                    let guard: $ty = 101.25;
                    let mut output = vec![guard; rows * columns + 2];
                    // The writable interior has exactly rows * columns elements.
                    assert!(unsafe { $copy(&mut engine, result.0, output.as_mut_ptr().add(1), columns, rows) });
                    assert_eq!(output[0], guard);
                    assert_eq!(output[output.len() - 1], guard);
                    let expected = expected_product(&a, &b, rows, inner, columns);
                    for r in 0..rows { for c in 0..columns {
                        assert_eq!(output[1 + c * rows + r] as f64, expected[r * columns + c], "shape={rows},{inner},{columns} seed={seed} at={r},{c}");
                    }}
                    let mut preserved = vec![0.0; rows * inner];
                    assert!(unsafe { $copy(&mut engine, a_handle.0, preserved.as_mut_ptr(), inner, rows) });
                    assert_eq!(preserved, a_buffer);
                }
            }}}
        }

        #[test]
        fn $special() {
            let mut engine = engine(&format!("fn f(m: mat3x3{suffix}) -> mat3x3{suffix} {{ m }}", suffix = $suffix));
            let input: [$ty; 9] = [0.0, -0.0, <$ty>::from_bits(1), <$ty>::MIN_POSITIVE, <$ty>::MAX, -<$ty>::MAX, <$ty>::INFINITY, <$ty>::NEG_INFINITY, <$ty>::NAN];
            let matrix = receive(&mut engine, |e, out| unsafe { $create(e, input.as_ptr(), 3, 3, out) }).unwrap();
            let result = call(&mut engine, "f", &[&matrix]).unwrap();
            drop(matrix);
            drop(engine);
            let mut other = MusubuEngine::new();
            let mut output = [0.0; 9];
            assert!(unsafe { $copy(&mut other, result.0, output.as_mut_ptr(), 3, 3) });
            for (expected, actual) in input.into_iter().zip(output) {
                if expected.is_nan() { assert!(actual.is_nan()); }
                else { assert_eq!(expected.to_bits(), actual.to_bits()); }
            }
        }
    };
}

matrix_properties!(
    f32_products_match_integer_oracle,
    f32_special_values_survive_identity,
    f32,
    "",
    value_create_matrix_f32,
    value_copy_matrix_f32
);
matrix_properties!(
    f64_products_match_integer_oracle,
    f64_special_values_survive_identity,
    f64,
    "f64",
    value_create_matrix_f64,
    value_copy_matrix_f64
);

#[test]
fn repeated_argument_handles_are_independent_and_results_survive_later_calls() {
    let mut engine = engine("fn f(mut a: mat2x2, b: mat2x2) -> mat2x2 { a[0][0] = 99.0; a + b }");
    let input = [1.0_f32, 2.0, 3.0, 4.0];
    let matrix = receive(&mut engine, |e, out| unsafe {
        value_create_matrix_f32(e, input.as_ptr(), 2, 2, out)
    })
    .unwrap();
    let first = call(&mut engine, "f", &[&matrix, &matrix]).unwrap();
    let second = call(&mut engine, "f", &[&first, &matrix]).unwrap();
    assert!(compile(&mut engine, "fn replacement() {}"));
    let mut output = [0.0_f32; 4];
    for (handle, expected) in [
        (&matrix, input),
        (&first, [100.0, 4.0, 6.0, 8.0]),
        (&second, [100.0, 6.0, 9.0, 12.0]),
    ] {
        assert!(unsafe { value_copy_matrix_f32(&mut engine, handle.0, output.as_mut_ptr(), 2, 2) });
        assert_eq!(output, expected);
    }
}
