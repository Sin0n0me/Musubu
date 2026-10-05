#![no_std]

extern crate alloc;

mod collections;
pub mod errors;
mod locals;
mod register_allocator;

use crate::errors::IRCompileError;
use crate::register_allocator::RegisterAllocator;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use musubu_hir::*;
use musubu_ir::*;
use musubu_primitive::{PrimitiveType, ToPrimitiveType};

pub type IRCompileResult<T> = Result<T, IRCompileError>;

pub fn compile_module(module: &HIRModule) -> IRCompileResult<Vec<(usize, CompiledFunction)>> {
    let mut functions = Vec::new();
    for (id, hir) in &module.functions {
        let code = compile_function(hir).map_err(|error| match module.function_ranges.get(id) {
            Some(&(start, end)) => IRCompileError::Located {
                start,
                end,
                error: alloc::boxed::Box::new(error),
            },
            None => error,
        })?;
        functions.push((*id, code));
    }

    Ok(functions)
}

pub fn compile_function(func: &HIRFunction) -> IRCompileResult<CompiledFunction> {
    let mut compiler = IRCompiler::new();

    for _ in 0..locals::register_count(func) {
        compiler.alloc_register();
    }
    let value = compiler.compile_block(&func.body)?;
    let value = if func.return_type.is_unit() {
        None
    } else {
        value
    };
    compiler.code.push(Instruction::Return { value });

    let code = CompiledFunction {
        code: compiler.code,
        registers: compiler.register_allocator.get_size(),
    };

    Ok(code)
}

#[derive(Debug)]
struct IRCompiler {
    code: Vec<Instruction>,
    register_allocator: RegisterAllocator,
    loop_statement: Vec<LoopStatement>,
}

#[derive(Debug)]
struct LoopStatement {
    loop_start: usize,
    result: Option<Register>,
    break_point: Vec<usize>, // コード上の位置(仮のジャンプ位置になっているので書き換える必要がある)
}

impl LoopStatement {
    fn new(loop_start: usize, result: Option<Register>) -> Self {
        Self {
            loop_start,
            result,
            break_point: Vec::new(),
        }
    }
}

impl IRCompiler {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            register_allocator: RegisterAllocator::new(),
            loop_statement: Vec::new(),
        }
    }

    fn alloc_register(&mut self) -> Register {
        self.register_allocator.alloc()
    }

    fn compile_block(&mut self, block: &HIRBlock) -> IRCompileResult<Option<Register>> {
        let dst = if block.to_type().is_unit() {
            None
        } else {
            Some(self.alloc_register())
        };

        self.register_allocator.enter_block();

        let mut src = None;
        for statement in &block.statements {
            src = self.compile_statement(statement)?;
        }

        // 戻り値があれば代入
        let ret = match (src, dst) {
            (Some(src), Some(dst)) => {
                self.code.push(Instruction::Move { dst, src });
                Some(dst)
            }
            _ => None,
        };

        // 使用済みレジスタの解放
        self.register_allocator.exit_block();

        Ok(ret)
    }

    fn compile_statement(&mut self, statement: &HIRStatement) -> IRCompileResult<Option<Register>> {
        let reg = match statement {
            HIRStatement::Let {
                symbol,
                symbol_type,
                initializer,
            } => {
                if let Some(expr) = initializer {
                    let Some(src) = self.compile_expr(expr)? else {
                        return Err(IRCompileError::ExpectRegister);
                    };
                    self.code.push(Instruction::Move {
                        dst: Register(*symbol),
                        src,
                    });
                }
                None
            }
            HIRStatement::Expr(expr) => self.compile_expr(expr)?,
            HIRStatement::Discard(expr) => {
                self.compile_expr(expr)?;
                None
            }
        };

        Ok(reg)
    }

    fn compile_expr(&mut self, expr: &HIRExpression) -> IRCompileResult<Option<Register>> {
        match expr {
            HIRExpression::Block(block) => self.compile_block(block),
            HIRExpression::If {
                cond,
                then_block,
                else_block,
            } => self.compile_if(cond, then_block, else_block.as_ref()),
            HIRExpression::Loop { body, result_type } => self.compile_loop(body, result_type),
            HIRExpression::For {
                symbol,
                iterator,
                body,
            } => {
                self.compile_for(*symbol, iterator, body)?;
                Ok(None)
            }
            HIRExpression::Continue => {
                self.compile_continue()?;
                Ok(None)
            }
            HIRExpression::Break(expr) => {
                self.compile_break(expr.as_deref())?;
                Ok(None)
            }
            HIRExpression::Return(expr) => {
                self.compile_return(expr.as_deref())?;
                Ok(None)
            }
            _ => self.compile_value(expr),
        }
    }

    fn compile_value(&mut self, expr: &HIRExpression) -> IRCompileResult<Option<Register>> {
        let register = match expr {
            HIRExpression::Literal(value) => {
                let dst = self.alloc_register();
                self.code.push(Instruction::LoadConst {
                    dst,
                    value: value.clone(),
                });
                dst
            }
            HIRExpression::Variable { id, .. } => {
                // Snapshot before later operands can mutate the same local.
                let dst = self.alloc_register();
                self.code.push(Instruction::Move {
                    dst,
                    src: Register(*id),
                });
                dst
            }
            HIRExpression::Store { target, value } => {
                let src = self
                    .compile_expr(value)?
                    .ok_or(IRCompileError::ExpectRegister)?;
                self.code.push(Instruction::Move {
                    dst: Register(*target),
                    src,
                });
                return Ok(None);
            }
            HIRExpression::BinOp { .. } | HIRExpression::CmpOp { .. } => {
                self.compile_binary(expr)?
            }
            HIRExpression::Call {
                function,
                return_type,
                arguments,
            } => {
                let args = arguments
                    .iter()
                    .map(|arg| {
                        self.compile_expr(arg)?
                            .ok_or(IRCompileError::ExpectRegister)
                    })
                    .collect::<IRCompileResult<Vec<_>>>()?;
                let dst = if return_type.is_unit() {
                    None
                } else {
                    Some(self.alloc_register())
                };
                self.code.push(Instruction::Call {
                    dst,
                    func: *function,
                    args,
                });
                return Ok(dst);
            }
            HIRExpression::Array { .. }
            | HIRExpression::ArrayRepeat { .. }
            | HIRExpression::Range { .. } => self.compile_collection(expr)?,
            _ => return Err(IRCompileError::ExpectRegister),
        };
        Ok(Some(register))
    }

    fn compile_binary(&mut self, expression: &HIRExpression) -> IRCompileResult<Register> {
        let (lhs, rhs) = match expression {
            HIRExpression::BinOp { lhs, rhs, .. } | HIRExpression::CmpOp { lhs, rhs, .. } => {
                (lhs, rhs)
            }
            _ => return Err(IRCompileError::ExpectRegister),
        };
        let lhs = self
            .compile_expr(lhs)?
            .ok_or(IRCompileError::ExpectRegister)?;
        let rhs = self
            .compile_expr(rhs)?
            .ok_or(IRCompileError::ExpectRegister)?;
        let dst = self.alloc_register();
        let instruction = match expression {
            HIRExpression::BinOp { op, .. } => Instruction::BinOp {
                dst,
                op: op.clone(),
                lhs,
                rhs,
            },
            HIRExpression::CmpOp { op, .. } => Instruction::Cmp {
                dst,
                op: op.clone(),
                lhs,
                rhs,
            },
            _ => return Err(IRCompileError::ExpectRegister),
        };
        self.code.push(instruction);
        Ok(dst)
    }

    fn compile_if(
        &mut self,
        cond: &HIRExpression,
        then_block: &HIRBlock,
        else_block: Option<&HIRBlock>,
    ) -> IRCompileResult<Option<Register>> {
        let dst = if else_block.is_some() && !then_block.to_type().is_unit() {
            Some(self.alloc_register())
        } else {
            None
        };
        let cond = self
            .compile_expr(cond)?
            .ok_or(IRCompileError::ExpectRegister)?;
        let false_jump = self.code.len();
        self.code.push(Instruction::JumpIfFalse { cond, target: 0 });
        let then_value = self.compile_block(then_block)?;
        if let (Some(dst), Some(src)) = (dst, then_value) {
            self.code.push(Instruction::Move { dst, src });
        }
        let end_jump = self.code.len();
        self.code.push(Instruction::Jump { target: 0 });
        let else_start = self.code.len();
        if let Some(body) = else_block {
            let else_value = self.compile_block(body)?;
            if let (Some(dst), Some(src)) = (dst, else_value) {
                self.code.push(Instruction::Move { dst, src });
            }
        }
        let end = self.code.len();
        if let Instruction::JumpIfFalse { target, .. } = &mut self.code[false_jump] {
            *target = else_start;
        }
        if let Instruction::Jump { target } = &mut self.code[end_jump] {
            *target = end;
        }
        Ok(dst)
    }

    fn compile_loop(
        &mut self,
        body: &HIRBlock,
        result_type: &PrimitiveType,
    ) -> IRCompileResult<Option<Register>> {
        let result = if result_type.is_unit() {
            None
        } else {
            Some(self.alloc_register())
        };
        let loop_start = self.code.len();
        self.loop_statement
            .push(LoopStatement::new(loop_start, result));

        self.compile_block(body)?;

        let Some(loop_statement) = self.loop_statement.pop() else {
            return Err(IRCompileError::InvalidLoopStatement);
        };

        let instruction = Instruction::Jump { target: loop_start };
        self.code.push(instruction);

        // break文があった場合ジャンプ位置の修正
        let loop_end = self.code.len();
        for point in loop_statement.break_point {
            let Instruction::Jump { target } = &mut self.code[point] else {
                return Err(IRCompileError::IllegalBreak);
            };

            *target = loop_end;
        }

        Ok(result)
    }

    fn compile_break(&mut self, expr: Option<&HIRExpression>) -> IRCompileResult<()> {
        if self.loop_statement.is_empty() {
            return Err(IRCompileError::IllegalBreak);
        }

        let dst = self
            .loop_statement
            .last()
            .and_then(|context| context.result);
        if let Some(expression) = expr {
            let src = self.compile_expr(expression)?;
            match (dst, src) {
                (Some(dst), Some(src)) => self.code.push(Instruction::Move { dst, src }),
                (None, None) => {}
                _ => return Err(IRCompileError::IllegalBreak),
            }
        } else if dst.is_some() {
            return Err(IRCompileError::IllegalBreak);
        }

        let break_position = self.code.len(); // 現在の命令位置
        let instruction = Instruction::Jump {
            target: self.code.len() + 1,
        };
        self.code.push(instruction);

        // 後でbreak時の飛び先を決めるためにスタックに保持
        if let Some(loop_info) = self.loop_statement.last_mut() {
            loop_info.break_point.push(break_position);
        }

        Ok(())
    }

    fn compile_continue(&mut self) -> IRCompileResult<()> {
        let loop_start = self
            .loop_statement
            .last()
            .ok_or(IRCompileError::IllegalContinue)?
            .loop_start;
        let instruction = Instruction::Jump { target: loop_start };
        self.code.push(instruction);

        Ok(())
    }

    fn compile_return(&mut self, expr: Option<&HIRExpression>) -> IRCompileResult<()> {
        let value = if let Some(expr) = expr {
            self.compile_expr(expr)?
        } else {
            None
        };

        self.code.push(Instruction::Return { value });

        Ok(())
    }
}
