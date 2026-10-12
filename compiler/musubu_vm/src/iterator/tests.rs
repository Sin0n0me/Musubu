use super::*;
use alloc::vec;
use musubu_primitive::PrimitiveType;

fn range(start: Integer, end: Integer, inclusive: bool) -> IteratorState {
    IteratorState::new(Value::Range {
        start,
        end,
        inclusive,
    })
    .unwrap()
}

macro_rules! endpoint_tests {
    ($($name:ident: $kind:ident, $max:expr;)*) => {
        $(#[test]
        fn $name() {
            let max = Integer::$kind($max);
            let mut iter = range(Integer::$kind($max - 1), max.clone(), true);
            assert!(matches!(iter.next(), Some(Value::Integer(Integer::$kind(n))) if n == $max - 1));
            assert!(matches!(iter.next(), Some(Value::Integer(Integer::$kind(n))) if n == $max));
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
            let mut exclusive = range(max.clone(), max, false);
            assert!(exclusive.next().is_none());
        })*
    };
}

endpoint_tests! {
    i8_maximum: Int8, i8::MAX;
    i16_maximum: Int16, i16::MAX;
    i32_maximum: Int32, i32::MAX;
    i64_maximum: Int64, i64::MAX;
    u8_maximum: Uint8, u8::MAX;
    u16_maximum: Uint16, u16::MAX;
    u32_maximum: Uint32, u32::MAX;
    u64_maximum: Uint64, u64::MAX;
}

#[test]
fn signed_range_crosses_zero_and_stays_exhausted() {
    let mut iter = range(Integer::Int8(-1), Integer::Int8(1), true);
    for expected in [-1, 0, 1] {
        assert!(matches!(iter.next(), Some(Value::Integer(Integer::Int8(n))) if n == expected));
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn reversed_ranges_are_empty_for_both_endpoint_modes() {
    for inclusive in [false, true] {
        let mut iter = range(Integer::Int32(3), Integer::Int32(-3), inclusive);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
}

#[test]
fn incompatible_integer_endpoint_types_are_rejected() {
    for end in [Integer::Int16(2), Integer::Uint8(2)] {
        assert!(matches!(
            IteratorState::new(Value::Range {
                start: Integer::Int8(1),
                end,
                inclusive: true
            }),
            Err(VMError::InvalidOperand)
        ));
    }
}

#[test]
fn non_iterable_values_are_rejected() {
    for value in [Value::Bool(true), Value::Integer(Integer::Int32(3))] {
        assert!(matches!(
            IteratorState::new(value),
            Err(VMError::InvalidOperand)
        ));
    }
}

#[test]
fn array_iteration_preserves_order_and_exhaustion() {
    let mut iter = IteratorState::new(Value::Array {
        elements: vec![
            Value::Integer(Integer::Int32(4)),
            Value::Integer(Integer::Int32(-2)),
        ],
        element_type: PrimitiveType::default_integer(),
    })
    .unwrap();
    for expected in [4, -2] {
        assert!(matches!(iter.next(), Some(Value::Integer(Integer::Int32(n))) if n == expected));
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn empty_array_is_immediately_exhausted() {
    let mut iter = IteratorState::new(Value::Array {
        elements: vec![],
        element_type: PrimitiveType::Unit,
    })
    .unwrap();
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}
