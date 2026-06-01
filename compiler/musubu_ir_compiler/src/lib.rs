// TODO
//#![no_std]

extern crate alloc;

pub mod errors;
mod register_allocator;

use crate::errors::IRCompileError;
use crate::register_allocator::RegisterAllocator;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use musubu_hir::*;
use musubu_ir::*;
use musubu_primitive::PrimitiveType;

pub type IRCompileResult<T> = Result<T, IRCompileError>;

pub fn compile_module(module: &HIRModule) -> IRCompileResult<Vec<(usize, CompiledFunction)>> {
    let mut functions = Vec::new();
    for (id, hir) in &module.functions {
        let code = compile_function(hir)?;
        functions.push((*id, code));
    }

    Ok(functions)
}

pub fn compile_function(func: &HIRFunction) -> IRCompileResult<CompiledFunction> {
    let mut compiler = IRCompiler::new();

    compiler.compile_arguments(&func.params)?;
    compiler.compile_block(&func.body)?;

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
    break_point: Vec<usize>, // コード上の位置(仮のジャンプ位置になっているので書き換える必要がある)
}

impl LoopStatement {
    fn new(loop_start: usize) -> Self {
        Self {
            loop_start,
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

    fn compile_arguments(&mut self, arguments: &[HIRFunctionParam]) -> IRCompileResult<()> {
        for _ in arguments {
            self.alloc_register();
        }

        Ok(())
    }

    fn compile_block(&mut self, block: &HIRBlock) -> IRCompileResult<Option<Register>> {
        // TODO
        // 戻り値がない場合無駄に割り当てるので無駄を省きたい
        let dst = self.alloc_register();

        self.register_allocator.enter_block();

        let mut res = None;
        for statement in &block.statements {
            res = self.compile_statement(statement)?;
        }

        // 戻り値があれば代入
        let ret = if let Some(src) = res {
            self.code.push(Instruction::Move { dst, src });
            Some(dst)
        } else {
            None
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
                    let reg = self.compile_expr(expr)?;
                    self.code.push(Instruction::Move {
                        dst: Register(*symbol),
                        src: reg,
                    });
                }
                None
            }
            HIRStatement::Expr(expr) => Some(self.compile_expr(expr)?),
        };

        Ok(reg)
    }

    fn compile_expr(&mut self, expr: &HIRExpression) -> IRCompileResult<Register> {
        let reg = match expr {
            HIRExpression::Literal(literal) => {
                let dst = self.alloc_register();
                self.code.push(Instruction::LoadConst {
                    dst,
                    value: literal.clone(),
                });
                dst
            }

            HIRExpression::Variable { id, symbol_type } => Register(*id),

            HIRExpression::Store { target, value } => {
                let val = self.compile_expr(value)?;
                let dst = Register(*target);
                self.code.push(Instruction::Move { dst, src: val });
                dst
            }

            HIRExpression::BinOp { op, lhs, rhs } => {
                let lhs = self.compile_expr(lhs)?;
                let rhs = self.compile_expr(rhs)?;
                let dst = self.alloc_register();
                self.code.push(Instruction::BinOp {
                    dst,
                    op: op.clone(),
                    lhs,
                    rhs,
                });
                dst
            }

            HIRExpression::CmpOp { op, lhs, rhs } => {
                let lhs = self.compile_expr(lhs)?;
                let rhs = self.compile_expr(rhs)?;
                let dst = self.alloc_register();
                self.code.push(Instruction::Cmp {
                    dst,
                    op: op.clone(),
                    lhs,
                    rhs,
                });
                dst
            }
            HIRExpression::Call {
                function,
                return_type,
                arguments,
            } => {
                let regs = arguments
                    .iter()
                    .map(|a| self.compile_expr(a))
                    .collect::<Result<Vec<_>, IRCompileError>>()?;
                let dst = self.alloc_register();
                self.code.push(Instruction::Call {
                    dst: Some(dst),
                    func: *function,
                    args: regs,
                });
                dst
            }
            HIRExpression::Block(block) => self.compile_block(block)?.unwrap_or(Register(0)),
            HIRExpression::If {
                cond,
                then_block,
                else_block,
            } => self.compile_if(cond, then_block, else_block.as_ref())?,

            HIRExpression::Loop { body } => self.compile_loop(body)?,
            HIRExpression::Continue => self.compile_continue()?,
            HIRExpression::Break(expr) => {
                self.compile_break(expr.as_ref().map(|expr| expr.as_ref()))?
            }

            HIRExpression::Return(expr) => {
                self.compile_return(expr.as_ref().map(|expr| expr.as_ref()))?
            }
        };

        Ok(reg)
    }

    fn compile_if(
        &mut self,
        cond: &HIRExpression,
        then_block: &HIRBlock,
        else_block: Option<&HIRBlock>,
    ) -> IRCompileResult<Register> {
        let cond_reg = self.compile_expr(cond)?;

        // else 部分
        let jump_if_false_pos = self.code.len();
        self.code.push(Instruction::JumpIfFalse {
            cond: cond_reg,
            target: 0,
        });

        // then 部分
        let then_reg = self.compile_block(then_block)?;
        let jump_end_pos = self.code.len();
        self.code.push(Instruction::Jump { target: 0 });

        let else_start = self.code.len();
        if let Some(else_block) = else_block {
            self.compile_block(else_block)?;
        }

        let end = self.code.len();

        if let Instruction::JumpIfFalse { target, .. } = &mut self.code[jump_if_false_pos] {
            *target = else_start;
        }

        if let Instruction::Jump { target } = &mut self.code[jump_end_pos] {
            *target = end;
        }

        Ok(then_reg.unwrap_or(Register(0)))
    }

    fn compile_loop(&mut self, body: &HIRBlock) -> IRCompileResult<Register> {
        let loop_start = self.code.len();
        self.loop_statement.push(LoopStatement::new(loop_start));

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

        Ok(Register(0))
    }

    fn compile_break(&mut self, expr: Option<&HIRExpression>) -> IRCompileResult<Register> {
        if self.loop_statement.is_empty() {
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

        Ok(Register(0))
    }

    fn compile_continue(&mut self) -> IRCompileResult<Register> {
        let loop_start = self
            .loop_statement
            .last()
            .ok_or(IRCompileError::IllegalContinue)?
            .loop_start;
        let instruction = Instruction::Jump { target: loop_start };
        self.code.push(instruction);

        Ok(Register(0))
    }

    fn compile_return(&mut self, expr: Option<&HIRExpression>) -> IRCompileResult<Register> {
        let val = if let Some(expr) = expr {
            Some(self.compile_expr(expr)?)
        } else {
            None
        };

        self.code.push(Instruction::Return { value: val });

        Ok(Register(0))
    }
}
