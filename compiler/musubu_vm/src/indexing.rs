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
        Value::Vector(v) => v.components().get(index).cloned(),
        Value::Matrix(m) => m.columns().get(index).cloned(),
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
