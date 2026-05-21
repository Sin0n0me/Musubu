//TODO
#![no_std]

extern crate alloc;

pub mod cache;
pub mod errors;

mod built_in;
mod frame;

use crate::cache::VMCache;
use crate::errors::VMError;
use crate::frame::Frame;
use alloc::collections::btree_map::BTreeMap;
use alloc::{vec, vec::Vec};
use built_in::*;
use musubu_ir::*;
use musubu_primitive::*;

pub type VMResult<T> = Result<T, VMError>;

pub struct VM<'a> {
    cache: &'a VMCache,
    stack: Vec<(Frame<'a>, usize)>,
}

impl<'a> VM<'a> {
    pub fn new(cache: &'a VMCache) -> Self {
        Self {
            cache,
            stack: Vec::new(),
        }
    }

    pub fn debug_run() {}

    pub fn run_function(&self, func_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        let mut frame = self.load_function(func_id, args)?;

        // 順次実行
        loop {
            let Some((frame, ret_dst)) = self.stack.last_mut() else {
                return Ok(None);
            };

            self.execute_instruction(frame, *ret_dst)?;
        }
    }

    fn execute_instruction(
        &mut self,
        frame: &mut Frame<'a>,
        ret_dst: usize,
    ) -> VMResult<Option<Value>> {
        let Some(inst) = frame.next() else {
            return Ok(None);
        };

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
                let ret = self.run_function(*func, call_args);

                if let (Some(dst), Some(val)) = (dst, ret) {
                    frame.registers[dst.0] = val;
                }

                self.stack.push();
            }
            Instruction::Return { value } => {
                let Some(reg) = value else {
                    return Ok(None);
                };
                frame.registers[ret_dst] = frame.registers[reg.0].clone();

                let Some(caller) = self.stack.last_mut() else {
                    return;
                };
            }

            // TODO 削除
            Instruction::BuiltInCall { dst, func, args } => {
                let mut call_args = Vec::with_capacity(args.len());
                for reg in args {
                    call_args.push(frame.registers[reg.0].clone());
                }

                let ret = self.call_built_in(*func, call_args);
                if let (Some(dst), Some(val)) = (dst, ret) {
                    frame.registers[dst.0] = val;
                }
            }
        };

        Ok(None)
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

    fn call_built_in(&self, func_id: usize, args: Vec<Value>) -> Option<Value> {
        // TODO: 専用クレートの作成(Desugerもマジックナンバー状態なので)
        // デモ用
        match func_id {
            0 => make_matrix_4x4_from_16_args(args),
            _ => None,
        }
    }
}
