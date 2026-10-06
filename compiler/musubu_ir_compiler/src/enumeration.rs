use super::*;
use musubu_hir::{HIRMatchArm, HIRMatchPattern};

impl IRCompiler {
    pub(super) fn compile_enum_expression(
        &mut self,
        expr: &HIRExpression,
    ) -> IRCompileResult<Option<Register>> {
        match expr {
            HIRExpression::Enum {
                variant,
                fields,
                enum_type,
            } => {
                let mut regs = alloc::vec![Register(0); fields.len()];
                // Payload expressions run in source order; slots use declaration order.
                for (index, expr) in fields {
                    regs[*index] = self
                        .compile_expr(expr)?
                        .ok_or(IRCompileError::ExpectRegister)?;
                }
                let dst = self.alloc_register();
                self.code.push(Instruction::MakeEnum {
                    dst,
                    variant: *variant,
                    fields: regs,
                    enum_type: enum_type.clone(),
                });
                Ok(Some(dst))
            }
            HIRExpression::Match {
                value,
                arms,
                result_type,
            } => self.compile_match(value, arms, result_type),
            _ => Err(IRCompileError::ExpectRegister),
        }
    }

    fn compile_match(
        &mut self,
        value: &HIRExpression,
        arms: &[HIRMatchArm],
        ty: &PrimitiveType,
    ) -> IRCompileResult<Option<Register>> {
        let value = self
            .compile_expr(value)?
            .ok_or(IRCompileError::ExpectRegister)?;
        let dst = if ty.is_unit() {
            None
        } else {
            Some(self.alloc_register())
        };
        let mut ends = Vec::new();
        for arm in arms {
            let mut failures = Vec::new();
            self.compile_match_pattern(value, &arm.pattern, &mut failures)?;
            let result = self.compile_expr(&arm.body)?;
            if let (Some(dst), Some(src)) = (dst, result) {
                self.code.push(Instruction::Move { dst, src });
            }
            ends.push(self.code.len());
            self.code.push(Instruction::Jump { target: 0 });
            let next = self.code.len();
            for failure in failures {
                if let Instruction::JumpIfFalse { target, .. } = &mut self.code[failure] {
                    *target = next;
                }
            }
        }
        // A malformed externally supplied enum must never yield an uninitialized result.
        self.code.push(Instruction::Unreachable);
        let end = self.code.len();
        for position in ends {
            if let Instruction::Jump { target } = &mut self.code[position] {
                *target = end;
            }
        }
        Ok(dst)
    }

    fn compile_match_pattern(
        &mut self,
        value: Register,
        pattern: &HIRMatchPattern,
        failures: &mut Vec<usize>,
    ) -> IRCompileResult<()> {
        match pattern {
            HIRMatchPattern::Wildcard => {}
            HIRMatchPattern::Binding(id) => self.code.push(Instruction::Move {
                dst: Register(*id),
                src: value,
            }),
            HIRMatchPattern::Variant { index, fields } => {
                let cond = self.alloc_register();
                self.code.push(Instruction::IsVariant {
                    dst: cond,
                    value,
                    variant: *index,
                });
                failures.push(self.code.len());
                self.code.push(Instruction::JumpIfFalse { cond, target: 0 });
                for (index, pattern) in fields {
                    let dst = self.alloc_register();
                    self.code.push(Instruction::LoadField {
                        dst,
                        parent: value,
                        index: *index,
                    });
                    self.compile_match_pattern(dst, pattern, failures)?;
                }
            }
        }
        Ok(())
    }
}
