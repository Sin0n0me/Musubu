//TODO
//#![no_std]

extern crate alloc;

pub mod errors;

// TODO: 削除(issue#4で対応予定)
// mod built_in;
mod frame;

use crate::errors::VMError;
use crate::frame::Frame;
use alloc::{vec, vec::Vec};
use musubu_cache::Cache;
use musubu_ir::*;
use musubu_primitive::*;

pub type VMResult<T> = Result<T, VMError>;

// TODO デバッグ用のスタックトレース
pub struct VM<'a> {
    cache: &'a Cache,
}

impl<'a> VM<'a> {
    pub fn new(cache: &'a Cache) -> Self {
        Self { cache }
    }

    pub fn run_function(&mut self, func_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        let frame = self.load_function(func_id, args)?;
        self.execute_frame(frame)
    }

    fn execute_frame(&mut self, frame: Frame<'a>) -> VMResult<Option<Value>> {
        // 順次実行
        let mut frame_stack = vec![frame];
        loop {
            let ret = self.next(&mut frame_stack)?;
            if frame_stack.is_empty() {
                return Ok(ret);
            }
        }
    }

    fn next(&self, frame_stack: &mut Vec<Frame<'a>>) -> VMResult<Option<Value>> {
        let Some(frame) = frame_stack.last_mut() else {
            return Ok(None);
        };
        let Some(inst) = frame.code.get(frame.ip) else {
            frame_stack.pop();
            return Ok(None);
        };
        frame.ip += 1;

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
                frame.next_reg = dst.map(|reg| reg.0);

                let mut call_args = Vec::with_capacity(args.len());
                for reg in args {
                    call_args.push(frame.registers[reg.0].clone());
                }

                let func = self.load_function(*func, call_args)?;
                frame_stack.push(func);
            }
            Instruction::Return { value } => {
                let Some(frame) = frame_stack.pop() else {
                    return Ok(None);
                };
                let Some(value) = value else {
                    return Ok(None);
                };

                let value = frame.registers[value.0].clone();

                // 呼び出し元の取得
                let Some(caller) = frame_stack.last_mut() else {
                    return Ok(Some(value));
                };

                // 呼び出し元に戻り値を返す(指定のレジスタに格納)
                let Some(ret_reg) = caller.next_reg else {
                    // IRコンパイル時点で保障されているはずなので
                    // 本来ならここの到達はあり得ない
                    return Err(VMError::InvalidDestinationAddressException);
                };
                caller.registers[ret_reg] = value;
            }

            // TODO: 削除(issue#4で対応予定)
            // Callと統合する
            Instruction::BuiltInCall { .. } => {
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

        Ok(None)
    }

    fn load_function(&self, func_id: usize, args: Vec<Value>) -> VMResult<Frame<'a>> {
        // フレーム作成
        let func = self
            .cache
            .get_function(&func_id)
            .ok_or(VMError::IllegalFunctionCall)?;
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
