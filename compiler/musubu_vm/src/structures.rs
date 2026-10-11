use super::*;

impl VM<'_> {
    pub(super) fn execute_struct(frame: &mut Frame<'_>, inst: &Instruction) -> VMResult<()> {
        match inst {
            Instruction::MakeStruct {
                dst,
                fields,
                struct_type,
            } => {
                let fields = fields
                    .iter()
                    .map(|reg| frame.registers[reg.0].clone())
                    .collect();
                frame.registers[dst.0] = if matches!(struct_type, PrimitiveType::Tuple { .. }) {
                    Value::Tuple {
                        fields,
                        tuple_type: struct_type.clone(),
                    }
                } else if matches!(struct_type, PrimitiveType::Matrix { .. }) {
                    Value::Matrix(Matrix::Columns {
                        columns: fields,
                        matrix_type: struct_type.clone(),
                    })
                } else if let PrimitiveType::Vector { type_kind, .. } = struct_type {
                    Value::Vector(Vector::Components {
                        elements: fields,
                        element_type: type_kind.as_ref().clone(),
                    })
                } else {
                    Value::Struct {
                        fields,
                        struct_type: struct_type.clone(),
                    }
                };
            }
            Instruction::LoadField { dst, parent, index } => {
                if let Value::Vector(vector) = &frame.registers[parent.0] {
                    let value = vector
                        .components()
                        .get(*index)
                        .cloned()
                        .ok_or(VMError::InvalidOperand)?;
                    frame.registers[dst.0] = value;
                    return Ok(());
                }
                let (Value::Struct { fields, .. }
                | Value::Enum { fields, .. }
                | Value::Tuple { fields, .. }) = &frame.registers[parent.0]
                else {
                    return Err(VMError::InvalidOperand);
                };
                let value = fields.get(*index).ok_or(VMError::InvalidOperand)?.clone();
                frame.registers[dst.0] = value;
            }
            Instruction::StoreField {
                target,
                path,
                value,
            } => {
                let value = frame.registers[value.0].clone();
                let mut target = &mut frame.registers[target.0];
                for index in path {
                    let fields = match target {
                        Value::Struct { fields, .. } | Value::Tuple { fields, .. } => {
                            fields.as_mut_slice()
                        }
                        Value::Vector(vector) => vector.components_mut(),
                        _ => return Err(VMError::InvalidOperand),
                    };
                    target = fields.get_mut(*index).ok_or(VMError::InvalidOperand)?;
                }
                *target = value;
            }
            _ => return Err(VMError::InvalidOperand),
        }
        Ok(())
    }
}
