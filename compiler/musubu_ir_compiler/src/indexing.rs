use super::*;

impl IRCompiler {
    pub(super) fn compile_index_expression(
        &mut self,
        expr: &HIRExpression,
    ) -> IRCompileResult<Option<Register>> {
        match expr {
            HIRExpression::Index { parent, index, .. } => {
                let parent = self
                    .compile_expr(parent)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                let index = self
                    .compile_expr(index)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                let dst = self.alloc_register();
                self.code
                    .push(Instruction::LoadIndex { dst, parent, index });
                Ok(Some(dst))
            }
            HIRExpression::StoreIndex {
                target,
                path,
                value,
                operator,
            } => {
                // Evaluate every subscript once, and snapshot the old element before the RHS.
                let mut indices = Vec::new();
                for index in path {
                    indices.push(
                        self.compile_expr(index)?
                            .ok_or(IRCompileError::ExpectRegister)?,
                    );
                }
                let mut old = Register(*target);
                for index in &indices {
                    let dst = self.alloc_register();
                    self.code.push(Instruction::LoadIndex {
                        dst,
                        parent: old,
                        index: *index,
                    });
                    old = dst;
                }
                let mut value = self
                    .compile_expr(value)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                if let Some(op) = operator {
                    let dst = self.alloc_register();
                    self.code.push(Instruction::BinOp {
                        dst,
                        op: op.clone(),
                        lhs: old,
                        rhs: value,
                    });
                    value = dst;
                }
                self.code.push(Instruction::StoreIndex {
                    target: Register(*target),
                    path: indices,
                    value,
                });
                Ok(None)
            }
            _ => Err(IRCompileError::ExpectRegister),
        }
    }
}
