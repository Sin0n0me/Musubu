use super::*;

impl VM<'_> {
    pub(super) fn execute_struct(frame: &mut Frame<'_>, inst: &Instruction) -> VMResult<()> {
        match inst {
            Instruction::MakeStruct {
                dst,
                fields,
                struct_type,
            } => {
                frame.registers[dst.0] = Value::Struct {
                    fields: fields
                        .iter()
                        .map(|reg| frame.registers[reg.0].clone())
                        .collect(),
                    struct_type: struct_type.clone(),
                };
            }
            Instruction::LoadField { dst, parent, index } => {
                let (Value::Struct { fields, .. } | Value::Enum { fields, .. }) =
                    &frame.registers[parent.0]
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
                    let Value::Struct { fields, .. } = target else {
                        return Err(VMError::InvalidOperand);
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
