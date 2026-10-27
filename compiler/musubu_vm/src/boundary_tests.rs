use super::*;

fn function(
    code: Vec<Instruction>,
    registers: usize,
    return_type: PrimitiveType,
) -> CompiledFunction {
    CompiledFunction {
        code,
        registers,
        return_type,
        parameters: Vec::new(),
        source_ranges: Vec::new(),
    }
}

fn constant(value: i32) -> CompiledFunction {
    function(
        vec![
            Instruction::LoadConst {
                dst: Register(0),
                value: Value::Integer(Integer::Int32(value)),
            },
            Instruction::Return {
                value: Some(Register(0)),
            },
        ],
        1,
        PrimitiveType::default_integer(),
    )
}

#[test]
fn instruction_budget_counts_instructions_not_implicit_returns() {
    let mut cache = Cache::new();
    cache
        .register_function(
            0,
            function(
                vec![Instruction::Jump { target: 1 }],
                0,
                PrimitiveType::Unit,
            ),
        )
        .unwrap();
    cache
        .register_function(1, function(vec![], 0, PrimitiveType::Unit))
        .unwrap();
    let limits = ExecutionLimits {
        instructions: 1,
        call_depth: 1,
    };
    let mut vm = VM::with_limits(&cache, limits);
    assert!(matches!(vm.run_function(0, vec![]), Ok(None)));
    assert!(matches!(vm.run_function(1, vec![]), Ok(None)));
}

#[test]
fn exact_budget_and_trace_reset_between_calls() {
    let mut cache = Cache::new();
    cache.register_function(0, constant(7)).unwrap();
    cache
        .register_function(
            1,
            function(
                vec![Instruction::Return { value: None }],
                0,
                PrimitiveType::Unit,
            ),
        )
        .unwrap();
    let mut short = VM::with_limits(
        &cache,
        ExecutionLimits {
            instructions: 1,
            call_depth: 1,
        },
    );
    for _ in 0..3 {
        assert!(matches!(
            short.run_function(0, vec![]),
            Err(VMError::InstructionLimitExceeded)
        ));
        assert_eq!(short.stack_trace().len(), 1);
        assert_eq!(short.stack_trace()[0].function, 0);
        assert_eq!(short.stack_trace()[0].instruction, 1);
        assert!(matches!(short.run_function(1, vec![]), Ok(None)));
        assert!(short.stack_trace().is_empty());
    }
    let mut exact = VM::with_limits(
        &cache,
        ExecutionLimits {
            instructions: 2,
            call_depth: 1,
        },
    );
    for _ in 0..3 {
        assert!(matches!(
            exact.run_function(0, vec![]),
            Ok(Some(Value::Integer(Integer::Int32(7))))
        ));
    }
}

#[test]
fn depth_limit_includes_entry_and_reports_rejected_call_site() {
    let mut cache = Cache::new();
    for id in 0..3 {
        cache
            .register_function(
                id,
                function(
                    vec![
                        Instruction::Call {
                            dst: Some(Register(0)),
                            func: id + 1,
                            args: vec![],
                        },
                        Instruction::Return {
                            value: Some(Register(0)),
                        },
                    ],
                    1,
                    PrimitiveType::default_integer(),
                ),
            )
            .unwrap();
    }
    cache.register_function(3, constant(9)).unwrap();
    let mut vm = VM::with_limits(
        &cache,
        ExecutionLimits {
            instructions: 20,
            call_depth: 3,
        },
    );
    assert!(matches!(
        vm.run_function(0, vec![]),
        Err(VMError::StackOverflow)
    ));
    assert_eq!(
        vm.stack_trace()
            .iter()
            .map(|frame| (frame.function, frame.instruction))
            .collect::<Vec<_>>(),
        vec![(2, 0), (1, 0), (0, 0)]
    );
    let mut vm = VM::with_limits(
        &cache,
        ExecutionLimits {
            instructions: 8,
            call_depth: 4,
        },
    );
    assert!(matches!(
        vm.run_function(0, vec![]),
        Ok(Some(Value::Integer(Integer::Int32(9))))
    ));
}

#[test]
fn non_boolean_branch_operands_are_rejected() {
    let mut cache = Cache::new();
    cache
        .register_function(
            0,
            function(
                vec![
                    Instruction::LoadConst {
                        dst: Register(0),
                        value: Value::Integer(Integer::Int32(0)),
                    },
                    Instruction::JumpIfFalse {
                        cond: Register(0),
                        target: 2,
                    },
                    Instruction::Return { value: None },
                ],
                1,
                PrimitiveType::Unit,
            ),
        )
        .unwrap();
    let mut vm = VM::new(&cache);
    assert!(matches!(
        vm.run_function(0, vec![]),
        Err(VMError::InvalidOperand)
    ));
    assert_eq!(vm.stack_trace()[0].instruction, 1);
}

#[test]
fn invalid_replacement_does_not_destroy_registered_code() {
    let mut cache = Cache::new();
    cache.register_function(0, constant(17)).unwrap();
    let invalid = function(
        vec![Instruction::Return {
            value: Some(Register(1)),
        }],
        1,
        PrimitiveType::Unit,
    );
    assert_eq!(
        cache.register_function(0, invalid),
        Err(InvalidIR::Register {
            instruction: 0,
            register: 1
        })
    );
    assert!(matches!(
        VM::new(&cache).run_function(0, vec![]),
        Ok(Some(Value::Integer(Integer::Int32(17))))
    ));
}

macro_rules! integer_boundaries {
    ($name:ident, $ty:ty, $variant:ident) => {
        #[test]
        fn $name() {
            let edges: Vec<$ty> = [i128::from(<$ty>::MIN), i128::from(<$ty>::MIN) + 1, -1, 0, 1, i128::from(<$ty>::MAX)]
                .into_iter().filter_map(|value| <$ty>::try_from(value).ok()).collect();
            for &a in &edges { for &b in &edges {
                for op in [BinaryOperator::Addition, BinaryOperator::Subtract, BinaryOperator::Multiply, BinaryOperator::Divide] {
                    // The independent oracle widens operands before evaluating a op b.
                    let expected = match op {
                        BinaryOperator::Addition => Some(i128::from(a) + i128::from(b)),
                        BinaryOperator::Subtract => Some(i128::from(a) - i128::from(b)),
                        BinaryOperator::Multiply => i128::from(a).checked_mul(i128::from(b)),
                        BinaryOperator::Divide if b != 0 => Some(i128::from(a) / i128::from(b)),
                        _ => None,
                    };
                    let actual = vectors::checked_integer(&op, &Integer::$variant(a), &Integer::$variant(b));
                    match expected.and_then(|value| <$ty>::try_from(value).ok()) {
                        Some(value) => assert!(matches!(actual, Ok(Integer::$variant(found)) if found == value), "{a:?} {op:?} {b:?}: {actual:?}"),
                        None if matches!(op, BinaryOperator::Divide) && b == 0 => assert!(matches!(actual, Err(VMError::DivisionByZero))),
                        None => assert!(matches!(actual, Err(VMError::ArithmeticOverflow)), "{a:?} {op:?} {b:?}: {actual:?}"),
                    }
                }
            }}
        }
    };
}

integer_boundaries!(i8_boundaries, i8, Int8);
integer_boundaries!(i16_boundaries, i16, Int16);
integer_boundaries!(i32_boundaries, i32, Int32);
integer_boundaries!(i64_boundaries, i64, Int64);
integer_boundaries!(u8_boundaries, u8, Uint8);
integer_boundaries!(u16_boundaries, u16, Uint16);
integer_boundaries!(u32_boundaries, u32, Uint32);
integer_boundaries!(u64_boundaries, u64, Uint64);
