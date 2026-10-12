use super::*;

impl IRCompiler {
    pub(super) fn compile_collection(
        &mut self,
        expression: &HIRExpression,
    ) -> IRCompileResult<Register> {
        let dst = self.alloc_register();
        let instruction = match expression {
            HIRExpression::Array {
                elements,
                element_type,
            } => {
                let elements = elements
                    .iter()
                    .map(|value| {
                        self.compile_expr(value)?
                            .ok_or(IRCompileError::ExpectRegister)
                    })
                    .collect::<IRCompileResult<Vec<_>>>()?;
                Instruction::MakeArray {
                    dst,
                    elements,
                    element_type: element_type.clone(),
                }
            }
            HIRExpression::ArrayRepeat { value, count } => {
                let value = self
                    .compile_expr(value)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                Instruction::RepeatArray {
                    dst,
                    value,
                    count: *count,
                }
            }
            HIRExpression::Range {
                start,
                end,
                inclusive,
            } => {
                let start = self
                    .compile_expr(start)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                let end = self
                    .compile_expr(end)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                Instruction::MakeRange {
                    dst,
                    start,
                    end,
                    inclusive: *inclusive,
                }
            }
            _ => return Err(IRCompileError::ExpectRegister),
        };
        self.code.push(instruction);
        Ok(dst)
    }

    pub(super) fn compile_for(
        &mut self,
        symbol: usize,
        iterable: &HIRExpression,
        body: &HIRBlock,
    ) -> IRCompileResult<()> {
        let iterable = self
            .compile_expr(iterable)?
            .ok_or(IRCompileError::ExpectRegister)?;
        let iterator = self.alloc_register();
        self.code.push(Instruction::IterInit { iterator, iterable });
        let loop_start = self.code.len();
        self.code.push(Instruction::IterNext {
            iterator,
            dst: Register(symbol),
            exhausted: 0,
        });
        self.loop_statement
            .push(LoopStatement::new(loop_start, None));
        self.compile_block(body)?;
        self.code.push(Instruction::Jump { target: loop_start });
        let end = self.code.len();
        if let Instruction::IterNext { exhausted, .. } = &mut self.code[loop_start] {
            *exhausted = end;
        }
        let context = self
            .loop_statement
            .pop()
            .ok_or(IRCompileError::InvalidLoopStatement)?;
        for point in context.break_point {
            let Instruction::Jump { target } = &mut self.code[point] else {
                return Err(IRCompileError::IllegalBreak);
            };
            *target = end;
        }
        self.code.push(Instruction::IterDrop { iterator });
        Ok(())
    }
}
