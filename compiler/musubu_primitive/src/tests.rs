use super::*;

macro_rules! integer_cases {
    ($($name:ident: $kind:ident, $rust:ty, $ty:literal;)*) => {
        $(#[test]
        fn $name() {
            let ty = PrimitiveType::from($ty).unwrap();
            assert_eq!(Integer::new(&alloc::format!("{}", <$rust>::MIN), &ty), Some(Integer::$kind(<$rust>::MIN)));
            assert_eq!(Integer::new(&alloc::format!("{}", <$rust>::MAX), &ty), Some(Integer::$kind(<$rust>::MAX)));
            // i128 holds both adjacent out-of-range values for every supported integer width.
            let below = i128::from(<$rust>::MIN) - 1;
            let above = i128::from(<$rust>::MAX) + 1;
            assert!(Integer::new(&alloc::format!("{below}"), &ty).is_none());
            assert!(Integer::new(&alloc::format!("{above}"), &ty).is_none());
            assert_eq!(Integer::$kind(<$rust>::MAX).checked_successor(), None);
            assert_eq!(Integer::$kind(0).checked_successor(), Some(Integer::$kind(1)));
            assert!(Integer::$kind(0).is_zero());
            assert!(!Integer::$kind(1).is_zero());
        })*
    };
}

integer_cases! {
    i8_boundaries: Int8, i8, "i8";
    i16_boundaries: Int16, i16, "i16";
    i32_boundaries: Int32, i32, "i32";
    i64_boundaries: Int64, i64, "i64";
    u8_boundaries: Uint8, u8, "u8";
    u16_boundaries: Uint16, u16, "u16";
    u32_boundaries: Uint32, u32, "u32";
    u64_boundaries: Uint64, u64, "u64";
}

#[test]
fn integer_parser_rejects_non_integer_types_and_malformed_values() {
    for input in ["", "1.0", "1x", "--1", " 1", "1 "] {
        assert!(
            Integer::new(input, &PrimitiveType::default_integer()).is_none(),
            "{input}"
        );
    }
    for ty in [
        PrimitiveType::Unit,
        PrimitiveType::Boolean,
        PrimitiveType::default_float(),
    ] {
        assert!(Integer::new("1", &ty).is_none());
    }
}

#[test]
fn vector_and_matrix_names_preserve_shape_and_scalar_type() {
    for suffix in [
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
    ] {
        let scalar = PrimitiveType::from(suffix).unwrap();
        assert_eq!(
            PrimitiveType::from(&alloc::format!("vec7{suffix}")),
            Some(PrimitiveType::Vector {
                dimension: 7,
                type_kind: Box::new(scalar.clone())
            })
        );
        assert_eq!(
            PrimitiveType::from(&alloc::format!("mat2x7{suffix}")),
            Some(PrimitiveType::Matrix {
                columns: 2,
                rows: 7,
                type_kind: Box::new(scalar)
            })
        );
    }
}

#[test]
fn default_float_aliases_have_identical_types() {
    assert_eq!(PrimitiveType::from("vec3"), PrimitiveType::from("vec3f32"));
    assert_eq!(
        PrimitiveType::from("mat2x3"),
        PrimitiveType::from("mat2x3f32")
    );
}

#[test]
fn malformed_aggregate_names_are_rejected() {
    for name in [
        "vec",
        "vec0",
        "vec-1",
        "vec4294967296",
        "vec2bool",
        "vec2vec2",
        "vec2f16",
        "vec2i32x",
        "mat",
        "mat2",
        "mat2x",
        "matx2",
        "mat0x2",
        "mat2x0",
        "mat2x4294967296",
        "mat2x2bool",
        "mat2x2vec2",
        "mat2x2f16",
        "mat2x2f32extra",
    ] {
        assert!(PrimitiveType::from(name).is_none(), "{name}");
    }
}

fn ty(name: &str) -> PrimitiveType {
    PrimitiveType::from(name).unwrap()
}

#[test]
fn rectangular_product_type_uses_outer_dimensions() {
    assert_eq!(
        matrix_result_type(
            &BinaryOperator::Multiply,
            &ty("mat2x3i32"),
            &ty("mat4x2i32")
        ),
        Some(ty("mat4x3i32"))
    );
    assert_eq!(
        matrix_result_type(&BinaryOperator::Multiply, &ty("mat2x3i32"), &ty("vec2i32")),
        Some(ty("vec3i32"))
    );
    assert_eq!(
        matrix_result_type(&BinaryOperator::Multiply, &ty("vec3i32"), &ty("mat2x3i32")),
        Some(ty("vec2i32"))
    );
}

#[test]
fn incompatible_matrix_operation_types_are_rejected() {
    for (op, lhs, rhs) in [
        (BinaryOperator::Multiply, "mat2x3", "mat2x3"),
        (BinaryOperator::Multiply, "mat2x3", "mat1x2f64"),
        (BinaryOperator::Multiply, "mat2x3", "vec3"),
        (BinaryOperator::Multiply, "vec2", "mat2x3"),
        (BinaryOperator::Addition, "mat2x3", "mat3x2"),
        (BinaryOperator::Subtract, "mat2x3", "mat2x3i32"),
        (BinaryOperator::Divide, "f32", "mat2x3"),
        (BinaryOperator::Divide, "mat2x3", "mat2x3"),
        (BinaryOperator::Multiply, "mat2x3", "f64"),
    ] {
        assert!(
            matrix_result_type(&op, &ty(lhs), &ty(rhs)).is_none(),
            "{lhs} {op:?} {rhs}"
        );
    }
}

#[test]
fn matrix_scalar_operations_preserve_matrix_type() {
    for (op, lhs, rhs) in [
        (BinaryOperator::Multiply, "mat2x3", "f32"),
        (BinaryOperator::Multiply, "f32", "mat2x3"),
        (BinaryOperator::Divide, "mat2x3", "f32"),
        (BinaryOperator::Addition, "mat2x3", "mat2x3"),
        (BinaryOperator::Subtract, "mat2x3", "mat2x3"),
    ] {
        assert_eq!(
            matrix_result_type(&op, &ty(lhs), &ty(rhs)),
            Some(ty("mat2x3"))
        );
    }
}
