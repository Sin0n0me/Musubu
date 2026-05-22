//TODO
#![no_std]

extern crate alloc;

pub mod cache;
pub mod errors;

// TODO: 削除(issue#4で対応予定)
// mod built_in;
mod frame;

use crate::cache::VMCache;
use crate::errors::VMError;
use crate::frame::Frame;
use alloc::{vec, vec::Vec};
use musubu_ir::*;
use musubu_primitive::*;

pub type VMResult<T> = Result<T, VMError>;

// TODO デバッグ用のスタックトレース
pub struct VM<'a> {
    cache: &'a VMCache,
    stack: Vec<VMFrame<'a>>,
}

#[derive(Debug)]
pub struct VMFrame<'a> {
    frame: Frame<'a>,
    caller_reg_dst: Option<Register>,
}

impl<'a> VM<'a> {
    pub fn new(cache: &'a VMCache) -> Self {
        Self {
            cache,
            stack: Vec::new(),
        }
    }

    pub fn run_function(&mut self, func_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        self.stack.push(VMFrame {
            frame: self.load_function(func_id, args)?,
            caller_reg_dst: None,
        });

        while let Some(frame) = self.stack.pop() {
            self.execute_frame(frame.frame)?;
        }

        Ok(None)
    }

    fn execute_frame(&mut self, frame: Frame<'a>) -> VMResult<Option<Value>> {
        // 順次実行
        let mut frame = frame;
        loop {
            let Some(inst) = frame.code.get(frame.ip) else {
                break Ok(None);
            };
            frame.ip += 1;

            // 0: break flag
            let (true, reg) = self.next(&mut frame, inst)? else {
                continue;
            };

            let Some(reg) = reg else {
                break Ok(None);
            };

            let val = frame.registers[reg.0].clone();
            let Some(vm_frame) = self.stack.last_mut() else {
                break Ok(Some(val)); // 呼び出し元が最上位の場合
            };

            // 呼び出し元に戻り値を返す場合(指定のレジスタに格納)
            if let Some(ret_dst) = vm_frame.caller_reg_dst {
                vm_frame.frame.registers[ret_dst.0] = val;
            };

            break Ok(None);
        }
    }

    fn next(
        &mut self,
        frame: &mut Frame<'a>,
        inst: &'a Instruction,
    ) -> VMResult<(bool, Option<&'a Register>)> {
        match inst {
            Instruction::LoadConst { dst, value } => {
                frame.registers[dst.0] = value.clone();
            }
            Instruction::Move { dst, src } => {
                frame.registers[dst.0] = frame.registers[src.0].clone();
            }
            Instruction::BinOp { dst, op, lhs, rhs } => {
                let l = &frame.registers[lhs.0];
                let r = &frame.registers[rhs.0];
                frame.registers[dst.0] = Self::eval_binop(op, l, r);
            }
            Instruction::Cmp { dst, op, lhs, rhs } => {
                let l = &frame.registers[lhs.0];
                let r = &frame.registers[rhs.0];
                frame.registers[dst.0] = Self::eval_cmp(op, l, r);
            }
            Instruction::JumpIfFalse { cond, target } => {
                if let Value::Bool(false) = frame.registers[cond.0] {
                    frame.ip = *target;
                }
            }
            Instruction::Jump { target } => {
                frame.ip = *target;
            }
            Instruction::Call { dst, func, args } => {
                let mut call_args = Vec::with_capacity(args.len());
                for reg in args {
                    call_args.push(frame.registers[reg.0].clone());
                }

                let func = self.load_function(*func, call_args)?;
                self.stack.push(VMFrame {
                    frame: func,
                    caller_reg_dst: dst.clone(),
                });

                return Ok((true, None));
            }
            Instruction::Return { value } => {
                return Ok((true, value.as_ref()));
            }

            // TODO: 削除
            // Callと統合する
            Instruction::BuiltInCall { dst, func, args } => {
                // TODO: 削除(issue#4で対応予定)
                /*
                   let mut call_args = Vec::with_capacity(args.len());
                   for reg in args {
                       call_args.push(frame.registers[reg.0].clone());
                   }

                   let ret = self.call_built_in(*func, call_args);
                   if let (Some(dst), Some(val)) = (dst, ret) {
                       frame.registers[dst.0] = val;
                   }
                * */
            }
        }

        Ok((false, None))
    }

    fn load_function(&self, func_id: usize, args: Vec<Value>) -> VMResult<Frame<'a>> {
        // フレーム作成
        let func = self.cache.get(&func_id)?;
        let frame = Frame::new(func.registers, &func.code, args);
        Ok(frame)
    }

    fn eval_binop(op: &BinaryOperator, l: &Value, r: &Value) -> Value {
        match (op, l, r) {
            (BinaryOperator::Addition, Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a + b)
            }
            (BinaryOperator::Subtract, Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a - b)
            }
            (BinaryOperator::Multiply, Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a * b)
            }
            (BinaryOperator::Divide, Value::Integer(a), Value::Integer(b)) => Value::Integer(a / b),

            (BinaryOperator::Multiply, Value::Matrix(a), Value::Matrix(b)) => Value::Matrix(a * b),

            _ => unimplemented!("unsupported: {op:?}, {l:?}, {r:?}"),
        }
    }

    fn eval_cmp(op: &ComparisonOperator, l: &Value, r: &Value) -> Value {
        match (op, l, r) {
            (ComparisonOperator::Equal, Value::Integer(a), Value::Integer(b)) => {
                Value::Bool(a == b)
            }
            _ => unimplemented!("unsupported"),
        }
    }

    // TODO: 削除(issue#4で対応予定)
    /*
    fn call_built_in(&self, func_id: usize, args: Vec<Value>) -> Option<Value> {
        // TODO: 専用クレートの作成(Desugerもマジックナンバー状態なので)
        // デモ用
        match func_id {
            0 => make_matrix_4x4_from_16_args(args),
            _ => None,
        }
    }
     * */
}
