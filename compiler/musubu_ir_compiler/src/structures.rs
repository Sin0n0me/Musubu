use super::*;

impl IRCompiler {
    pub(super) fn compile_struct_expression(
        &mut self,
        expr: &HIRExpression,
    ) -> IRCompileResult<Option<Register>> {
        match expr {
            HIRExpression::Struct {
                fields,
                struct_type,
            } => {
                let mut registers = alloc::vec![Register(0); fields.len()];
                // Evaluate in source order, then store in declaration order.
                for (index, expression) in fields {
                    registers[*index] = self
                        .compile_expr(expression)?
                        .ok_or(IRCompileError::ExpectRegister)?;
                }
                let dst = self.alloc_register();
                self.code.push(Instruction::MakeStruct {
                    dst,
                    fields: registers,
                    struct_type: struct_type.clone(),
                });
                Ok(Some(dst))
            }
            HIRExpression::Field { parent, index, .. } => {
                let parent = self
                    .compile_expr(parent)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                let dst = self.alloc_register();
                self.code.push(Instruction::LoadField {
                    dst,
                    parent,
                    index: *index,
                });
                Ok(Some(dst))
            }
            HIRExpression::StoreField {
                target,
                path,
                value,
            } => {
                let value = self
                    .compile_expr(value)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                self.code.push(Instruction::StoreField {
                    target: Register(*target),
                    path: path.clone(),
                    value,
                });
                Ok(None)
            }
            _ => Err(IRCompileError::ExpectRegister),
        }
    }
}
