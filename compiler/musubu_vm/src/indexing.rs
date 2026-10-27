use super::*;

impl VM<'_> {
    pub(super) fn execute_index(frame: &mut Frame<'_>, instruction: &Instruction) -> VMResult<()> {
        match instruction {
            Instruction::LoadIndex { dst, parent, index } => {
                let index = index_value(&frame.registers[index.0])?;
                frame.registers[dst.0] = load(&frame.registers[parent.0], index)?;
            }
            Instruction::StoreIndex {
                target,
                path,
                value,
            } => {
                let indices = path
                    .iter()
                    .map(|i| index_value(&frame.registers[i.0]))
                    .collect::<VMResult<Vec<_>>>()?;
                let value = frame.registers[value.0].clone();
                let mut target = &mut frame.registers[target.0];
                for index in indices {
                    let children = match target {
                        Value::Struct { fields, .. } | Value::Tuple { fields, .. } => {
                            fields.as_mut_slice()
                        }
                        Value::Vector(v) => v.components_mut(),
                        Value::Matrix(m) => m.columns_mut(),
                        _ => return Err(VMError::InvalidOperand),
                    };
                    target = children.get_mut(index).ok_or(VMError::IndexOutOfBounds)?;
                }
                *target = value;
            }
            _ => return Err(VMError::InvalidOperand),
        }
        Ok(())
    }
}

fn load(parent: &Value, index: usize) -> VMResult<Value> {
    let value = match parent {
        Value::Struct { fields, .. } | Value::Tuple { fields, .. } => fields.get(index).cloned(),
        Value::Vector(v) => v.component(index),
        Value::Matrix(m) => m.column(index),
        _ => return Err(VMError::InvalidOperand),
    };
    value.ok_or(VMError::IndexOutOfBounds)
}

fn index_value(value: &Value) -> VMResult<usize> {
    macro_rules! indices {
        ($($kind:ident),*) => { match value {
            $(Value::Integer(Integer::$kind(n)) => usize::try_from(*n).map_err(|_| VMError::IndexOutOfBounds)),*,
            _ => Err(VMError::InvalidOperand),
        } };
    }
    indices!(Int8, Int16, Int32, Int64, Uint8, Uint16, Uint32, Uint64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_integer_width_can_index() {
        for integer in [
            Integer::Int8(1),
            Integer::Int16(1),
            Integer::Int32(1),
            Integer::Int64(1),
            Integer::Uint8(1),
            Integer::Uint16(1),
            Integer::Uint32(1),
            Integer::Uint64(1),
        ] {
            assert_eq!(index_value(&Value::Integer(integer)).unwrap(), 1);
        }
    }

    #[test]
    fn negative_indices_are_bounds_errors() {
        for integer in [
            Integer::Int8(-1),
            Integer::Int16(-1),
            Integer::Int32(-1),
            Integer::Int64(-1),
        ] {
            assert!(matches!(
                index_value(&Value::Integer(integer)),
                Err(VMError::IndexOutOfBounds)
            ));
        }
    }

    #[test]
    fn boolean_and_float_indices_are_operand_errors() {
        for value in [Value::Bool(false), Value::Float(Float::Float32(1.0))] {
            assert!(matches!(index_value(&value), Err(VMError::InvalidOperand)));
        }
    }

    fn vector() -> Value {
        Value::Vector(Vector::Components {
            elements: vec![
                Value::Integer(Integer::Int32(2)),
                Value::Integer(Integer::Int32(9)),
            ],
            element_type: PrimitiveType::default_integer(),
        })
    }

    #[test]
    fn vector_load_checks_both_valid_endpoints_and_upper_bound() {
        let value = vector();
        assert!(matches!(
            load(&value, 0),
            Ok(Value::Integer(Integer::Int32(2)))
        ));
        assert!(matches!(
            load(&value, 1),
            Ok(Value::Integer(Integer::Int32(9)))
        ));
        assert!(matches!(load(&value, 2), Err(VMError::IndexOutOfBounds)));
        assert!(matches!(
            load(&value, usize::MAX),
            Err(VMError::IndexOutOfBounds)
        ));
    }

    #[test]
    fn scalar_load_returns_operand_error() {
        assert!(matches!(
            load(&Value::Bool(true), 0),
            Err(VMError::InvalidOperand)
        ));
    }

    #[test]
    fn store_updates_only_selected_component() {
        let mut frame = Frame::new(3, &[], vec![]);
        frame.registers[0] = vector();
        frame.registers[1] = Value::Integer(Integer::Int32(1));
        frame.registers[2] = Value::Integer(Integer::Int32(7));
        VM::execute_index(
            &mut frame,
            &Instruction::StoreIndex {
                target: Register(0),
                path: vec![Register(1)],
                value: Register(2),
            },
        )
        .unwrap();
        assert!(matches!(
            load(&frame.registers[0], 0),
            Ok(Value::Integer(Integer::Int32(2)))
        ));
        assert!(matches!(
            load(&frame.registers[0], 1),
            Ok(Value::Integer(Integer::Int32(7)))
        ));
    }

    #[test]
    fn failed_store_does_not_change_components() {
        let mut frame = Frame::new(3, &[], vec![]);
        frame.registers[0] = vector();
        frame.registers[1] = Value::Integer(Integer::Int32(2));
        frame.registers[2] = Value::Integer(Integer::Int32(7));
        assert!(matches!(
            VM::execute_index(
                &mut frame,
                &Instruction::StoreIndex {
                    target: Register(0),
                    path: vec![Register(1)],
                    value: Register(2)
                }
            ),
            Err(VMError::IndexOutOfBounds)
        ));
        assert!(matches!(
            load(&frame.registers[0], 0),
            Ok(Value::Integer(Integer::Int32(2)))
        ));
        assert!(matches!(
            load(&frame.registers[0], 1),
            Ok(Value::Integer(Integer::Int32(9)))
        ));
    }
}
