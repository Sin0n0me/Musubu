use super::*;
use musubu_primitive::{Float, PrimitiveType, ToPrimitiveType};

fn execute(source: &str) -> Value {
    let mut engine = MusubuEngine::new();
    assert!(compile(&mut engine, source), "{}", engine.compile_error());
    engine
        .run_function(0, vec![])
        .expect("matrix execution failed")
        .expect("missing result")
}

fn assert_matrix(source: &str, ty: &str, expected: &[&[i32]]) {
    let result = execute(source);
    assert_eq!(result.to_type(), PrimitiveType::from(ty).unwrap());
    let Value::Matrix(matrix) = result else {
        panic!("expected matrix");
    };
    let columns = matrix.columns();
    assert_eq!(columns.len(), expected.len());
    for (column, expected) in columns.iter().zip(expected) {
        assert_integer_vector(column, expected);
    }
}

fn assert_integer_vector(value: &Value, expected: &[i32]) {
    let Value::Vector(vector) = value else {
        panic!("expected vector");
    };
    let components = vector.components();
    assert_eq!(components.len(), expected.len());
    for (actual, expected) in components.iter().zip(expected) {
        assert!(
            matches!(actual, Value::Integer(Integer::Int32(n)) if n == expected),
            "expected {expected}, got {actual:?}"
        );
    }
}

fn assert_float_matrix(source: &str, ty: &str, expected: &[&[f64]], tolerance: f64) {
    let result = execute(source);
    assert_eq!(result.to_type(), PrimitiveType::from(ty).unwrap());
    let Value::Matrix(matrix) = result else {
        panic!("expected matrix");
    };
    let columns = matrix.columns();
    assert_eq!(columns.len(), expected.len());
    for (column, expected) in columns.iter().zip(expected) {
        let Value::Vector(vector) = column else {
            panic!("expected vector");
        };
        let values = vector.components();
        assert_eq!(values.len(), expected.len());
        for (value, expected) in values.iter().zip(*expected) {
            let actual = match value {
                Value::Float(Float::Float32(n)) => f64::from(*n),
                Value::Float(Float::Float64(n)) => *n,
                _ => panic!("expected float, got {value:?}"),
            };
            // Absolute error = |actual - expected|; these fixtures have small magnitudes.
            assert!(
                (actual - expected).abs() <= tolerance,
                "expected {expected}, got {actual}"
            );
        }
    }
}

macro_rules! matrix_cases {
    ($($name:ident: $source:literal, $ty:literal => $expected:expr;)*) => {
        $(#[test] fn $name() { assert_matrix($source, $ty, $expected); })*
    };
}

matrix_cases! {
    mat_all_elements_add: "fn main() -> mat2x3i32 { mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)) + mat2x3i32(vec3i32(-2,3,4),vec3i32(5,-6,7)) }", "mat2x3i32" => &[&[-1,5,7], &[9,-1,13]];
    mat_all_elements_subtract: "fn main() -> mat2x3i32 { mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)) - mat2x3i32(vec3i32(-2,3,4),vec3i32(5,-6,7)) }", "mat2x3i32" => &[&[3,-1,-1], &[-1,11,-1]];
    mat_all_elements_rectangular_product: "fn main() -> mat3x3i32 { mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)) * mat3x2i32(vec2i32(7,8),vec2i32(9,10),vec2i32(11,12)) }", "mat3x3i32" => &[&[39,54,69], &[49,68,87], &[59,82,105]];
    mat_outer_product: "fn main() -> mat2x3i32 { mat1x3i32(vec3i32(2,-1,3)) * mat2x1i32(vec1i32(4),vec1i32(-2)) }", "mat2x3i32" => &[&[8,-4,12], &[-4,2,-6]];
    mat_inner_product: "fn main() -> mat1x1i32 { mat3x1i32(vec1i32(2),vec1i32(-1),vec1i32(3)) * mat1x3i32(vec3i32(4,5,-2)) }", "mat1x1i32" => &[&[-3]];
    mat_right_identity: "fn main() -> mat2x3i32 { mat2x3i32(vec3i32(1,-2,3),vec3i32(4,5,-6)) * mat2x2i32(vec2i32(1,0),vec2i32(0,1)) }", "mat2x3i32" => &[&[1,-2,3], &[4,5,-6]];
    mat_left_identity: "fn main() -> mat2x3i32 { mat3x3i32(vec3i32(1,0,0),vec3i32(0,1,0),vec3i32(0,0,1)) * mat2x3i32(vec3i32(1,-2,3),vec3i32(4,5,-6)) }", "mat2x3i32" => &[&[1,-2,3], &[4,5,-6]];
    mat_right_zero: "fn main() -> mat2x3i32 { mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)) * mat2x2i32(vec2i32(0,0),vec2i32(0,0)) }", "mat2x3i32" => &[&[0,0,0], &[0,0,0]];
    mat_left_zero: "fn main() -> mat2x3i32 { mat3x3i32(vec3i32(0,0,0),vec3i32(0,0,0),vec3i32(0,0,0)) * mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)) }", "mat2x3i32" => &[&[0,0,0], &[0,0,0]];
    mat_negative_scale: "fn main() -> mat2x2i32 { mat2x2i32(vec2i32(1,-2),vec2i32(-3,4)) * -3 }", "mat2x2i32" => &[&[-3,6], &[9,-12]];
    mat_negative_divisor: "fn main() -> mat2x2i32 { mat2x2i32(vec2i32(-7,9),vec2i32(7,-9)) / -2 }", "mat2x2i32" => &[&[3,-4], &[-3,4]];
    mat_self_multiply: "fn main() -> mat2x2i32 { let mut m = mat2x2i32(vec2i32(1,3),vec2i32(2,4)); m *= m; m }", "mat2x2i32" => &[&[7,15], &[10,22]];
    mat_rectangular_compound_product: "fn main() -> mat2x3i32 { let mut m = mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)); m *= mat2x2i32(vec2i32(2,1),vec2i32(3,4)); m }", "mat2x3i32" => &[&[6,9,12], &[19,26,33]];
    mat_arithmetic_precedence: "fn main() -> mat2x2i32 { let a = mat2x2i32(vec2i32(1,3),vec2i32(2,4)); let b = mat2x2i32(vec2i32(0,1),vec2i32(1,0)); a + a * b }", "mat2x2i32" => &[&[3,7], &[3,7]];
}

#[test]
fn mat_multiplication_order() {
    let definitions = "let a = mat2x2i32(vec2i32(1,3),vec2i32(2,4)); let b = mat2x2i32(vec2i32(0,1),vec2i32(1,0));";
    assert_matrix(
        &alloc::format!("fn main() -> mat2x2i32 {{ {definitions} a * b }}"),
        "mat2x2i32",
        &[&[2, 4], &[1, 3]],
    );
    assert_matrix(
        &alloc::format!("fn main() -> mat2x2i32 {{ {definitions} b * a }}"),
        "mat2x2i32",
        &[&[3, 1], &[4, 2]],
    );
}

#[test]
fn mat_chained_rectangular_products() {
    let definitions = "let a = mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6)); let b = mat3x2i32(vec2i32(7,8),vec2i32(9,10),vec2i32(11,12)); let c = mat1x3i32(vec3i32(1,-1,2));";
    for expression in ["let ab = a * b; ab * c", "let bc = b * c; a * bc"] {
        assert_matrix(
            &alloc::format!("fn main() -> mat1x3i32 {{ {definitions} {expression} }}"),
            "mat1x3i32",
            &[&[108, 150, 192]],
        );
    }
}

#[test]
fn mat_vector_full_results() {
    let matrix = "let m = mat2x3i32(vec3i32(1,2,3),vec3i32(4,5,6));";
    let column = execute(&alloc::format!(
        "fn main() -> vec3i32 {{ {matrix} m * vec2i32(2,3) }}"
    ));
    assert_integer_vector(&column, &[14, 19, 24]);
    let row = execute(&alloc::format!(
        "fn main() -> vec2i32 {{ {matrix} vec3i32(2,3,4) * m }}"
    ));
    assert_integer_vector(&row, &[20, 47]);
}

const TRANSFORMS: &str = "let t = mat4x4i32(vec4i32(1,0,0,0),vec4i32(0,1,0,0),vec4i32(0,0,1,0),vec4i32(10,20,30,1)); let s = mat4x4i32(vec4i32(2,0,0,0),vec4i32(0,3,0,0),vec4i32(0,0,4,0),vec4i32(0,0,0,1));";

#[test]
fn mat_transform_composition_order() {
    // Column-vector convention: T*S scales first, then translates; S*T scales the translation too.
    assert_matrix(
        &alloc::format!("fn main() -> mat4x4i32 {{ {TRANSFORMS} t * s }}"),
        "mat4x4i32",
        &[
            &[2, 0, 0, 0],
            &[0, 3, 0, 0],
            &[0, 0, 4, 0],
            &[10, 20, 30, 1],
        ],
    );
    assert_matrix(
        &alloc::format!("fn main() -> mat4x4i32 {{ {TRANSFORMS} s * t }}"),
        "mat4x4i32",
        &[
            &[2, 0, 0, 0],
            &[0, 3, 0, 0],
            &[0, 0, 4, 0],
            &[20, 60, 120, 1],
        ],
    );
}

#[test]
fn mat_homogeneous_point_and_direction() {
    // p' = T*S*p; w=1 includes translation, while w=0 excludes it.
    for (w, expected) in [(1, [12, 26, 42, 1]), (0, [2, 6, 12, 0])] {
        let source =
            alloc::format!("fn main() -> vec4i32 {{ {TRANSFORMS} t * s * vec4i32(1,2,3,{w}) }}");
        assert_integer_vector(&execute(&source), &expected);
    }
}

#[test]
fn mat_f32_fractional_product() {
    assert_float_matrix(
        "fn main() -> mat2x2 { let m = mat2x2(vec2(0.1,0.2),vec2(0.3,0.4)); m * m }",
        "mat2x2",
        &[&[0.07, 0.10], &[0.15, 0.22]],
        1e-6,
    );
}

#[test]
fn mat_f64_fractional_product() {
    assert_float_matrix(
        "fn main() -> mat2x2f64 { let m = mat2x2f64(vec2f64(0.1f64,0.2f64),vec2f64(0.3f64,0.4f64)); m * m }",
        "mat2x2f64",
        &[&[0.07, 0.10], &[0.15, 0.22]],
        1e-12,
    );
}

#[test]
fn mat_f64_precision_is_preserved() {
    // 2^24 + 1 is exactly representable in f64, but not f32; scaling must not narrow it.
    assert_float_matrix(
        "fn main() -> mat1x2f64 { mat1x2f64(vec2f64(16777217.0f64,-16777217.0f64)) * 2.0f64 }",
        "mat1x2f64",
        &[&[33554434.0, -33554434.0]],
        0.0,
    );
}

#[test]
fn mat_floating_scalar_operations() {
    assert_float_matrix(
        "fn main() -> mat2x2 { let mut m = mat2x2(vec2(0.5,-1.5),vec2(2.5,-3.5)); m *= 2.0; m /= -4.0; m }",
        "mat2x2",
        &[&[-0.25, 0.75], &[-1.25, 1.75]],
        0.0,
    );
}
