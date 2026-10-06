use super::*;

impl VM<'_> {
    pub(super) fn execute_enum(frame: &mut Frame<'_>, inst: &Instruction) -> VMResult<()> {
        match inst {
            Instruction::MakeEnum {
                dst,
                variant,
                fields,
                enum_type,
            } => {
                frame.registers[dst.0] = Value::Enum {
                    variant: *variant,
                    enum_type: enum_type.clone(),
                    fields: fields
                        .iter()
                        .map(|reg| frame.registers[reg.0].clone())
                        .collect(),
                };
            }
            Instruction::IsVariant {
                dst,
                value,
                variant,
            } => {
                let Value::Enum {
                    variant: actual, ..
                } = &frame.registers[value.0]
                else {
                    return Err(VMError::InvalidOperand);
                };
                frame.registers[dst.0] = Value::Bool(actual == variant);
            }
            _ => return Err(VMError::InvalidOperand),
        }
        Ok(())
    }
}
