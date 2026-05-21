// TODO
//#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use musubu_hir::*;
use musubu_ir::*;

#[derive(Debug)]
struct IRCompiler {
    code: Vec<Instruction>,
    next_reg: usize,
}

impl IRCompiler {
    const INITIAL_REG: usize = 0;

    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            next_reg: Self::INITIAL_REG,
        }
    }

    fn alloc_register(&mut self) -> Register {
        const STEP: usize = 1;

        let r = Register(self.next_reg);
        self.next_reg += STEP;
        r
    }
}

pub fn compile_module(module: &HIRModule) -> Vec<CompiledFunction> {
    let mut functions = Vec::new();
    for func in &module.functions {
        functions.push(compile_function(func));
    }
    functions
}

pub fn compile_function(func: &HIRFunction) -> CompiledFunction {
    let mut compiler = IRCompiler::new();

    compiler.next_reg = func.params.len();
    compiler.compile_block(&func.body);

    CompiledFunction {
        code: compiler.code,
        registers: compiler.next_reg,
    }
}

impl IRCompiler {
    fn compile_block(&mut self, block: &HIRBlock) -> Option<Register> {
        let mut res = None;
        for statement in &block.statements {
            res = self.compile_statement(statement);
        }
        res
    }

    fn compile_statement(&mut self, statement: &HIRStatement) -> Option<Register> {
        match statement {
            HIRStatement::Let {
                symbol,
                symbol_type,
                initializer,
            } => {
                if let Some(expr) = initializer {
                    let r = self.compile_expr(expr);
                    self.code.push(Instruction::Move {
                        dst: Register(symbol.0 as usize),
                        src: r,
                    });
                }
                None
            }
            HIRStatement::Expr(expr) => Some(self.compile_expr(expr)),
        }
    }

    fn compile_expr(&mut self, expr: &HIRExpression) -> Register {
        match expr {
            HIRExpression::Literal(literal) => {
                let dst = self.alloc_register();

                self.code.push(Instruction::LoadConst {
                    dst,
                    value: literal.clone(),
                });

                dst
            }

            HIRExpression::Variable { id, symbol_type } => Register(id as usize),

            HIRExpression::Store { target, value } => {
                let val = self.compile_expr(value);
                let dst = Register(target.0 as usize);

                self.code.push(Instruction::Move { dst, src: val });

                dst
            }

            HIRExpression::BinOp { op, lhs, rhs } => {
                let lhs = self.compile_expr(lhs);
                let rhs = self.compile_expr(rhs);
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
                let lhs = self.compile_expr(lhs);
                let rhs = self.compile_expr(rhs);
                let dst = self.alloc_register();

                self.code.push(Instruction::Cmp {
                    dst,
                    op: op.clone(),
                    lhs,
                    rhs,
                });

                dst
            }

            HIRExpression::Call { function, args } => {
                let regs: Vec<_> = args.iter().map(|a| self.compile_expr(a)).collect();

                let dst = self.alloc_register();

                match function.id {
                    FunctionType::BuiltIn(id) => {
                        self.code.push(Instruction::BuiltInCall {
                            dst: Some(dst),
                            func: id,
                            args: regs,
                        });
                    }
                    FunctionType::UserDefined(id) => {
                        self.code.push(Instruction::Call {
                            dst: Some(dst),
                            func: id,
                            args: regs,
                        });
                    }
                }

                dst
            }
            HIRExpression::Block(block) => self.compile_block(block).unwrap_or(Register(0)),
            HIRExpression::If {
                cond,
                then_block,
                else_block,
            } => self.compile_if(cond, then_block, else_block.as_ref()),

            HIRExpression::Loop { body } => self.compile_loop(body),
            HIRExpression::Continue => {
                panic!("continue handling requires loop context")
            }
            HIRExpression::Break(_) => {
                panic!("break handling requires loop context")
            }

            HIRExpression::Return(expr) => {
                let val = expr.as_ref().map(|e| self.compile_expr(e));

                self.code.push(Instruction::Return { value: val });

                Register(0)
            }
        }
    }

    fn compile_if(
        &mut self,
        cond: &HIRExpression,
        then_block: &HIRBlock,
        else_block: Option<&HIRBlock>,
    ) -> Register {
        let cond_reg = self.compile_expr(cond);

        let jump_if_false_pos = self.code.len();

        self.code.push(Instruction::JumpIfFalse {
            cond: cond_reg,
            target: 0,
        });

        let then_reg = self.compile_block(then_block);

        let jump_end_pos = self.code.len();

        self.code.push(Instruction::Jump { target: 0 });

        let else_start = self.code.len();
        if let Some(else_block) = else_block {
            self.compile_block(else_block);
        }

        let end = self.code.len();

        if let Instruction::JumpIfFalse { target, .. } = &mut self.code[jump_if_false_pos] {
            *target = else_start;
        }

        if let Instruction::Jump { target } = &mut self.code[jump_end_pos] {
            *target = end;
        }

        then_reg.unwrap_or(Register(0))
    }

    fn compile_loop(&mut self, body: &HIRBlock) -> Register {
        let loop_start = self.code.len();

        self.compile_block(body);

        self.code.push(Instruction::Jump { target: loop_start });

        Register(0)
    }
}
