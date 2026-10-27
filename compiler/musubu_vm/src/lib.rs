#![no_std]

extern crate alloc;

pub mod errors;

mod enumeration;
mod frame;
mod indexing;
mod iterator;
mod matrices;
mod structures;
mod vectors;

#[cfg(test)]
mod boundary_tests;

use crate::errors::VMError;
use crate::frame::Frame;
use alloc::{vec, vec::Vec};
use musubu_cache::Cache;
use musubu_ir::*;
use musubu_primitive::*;

pub type VMResult<T> = Result<T, VMError>;

#[derive(Debug, Clone, Copy)]
pub struct TraceFrame {
    pub function: usize,
    pub instruction: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutionLimits {
    pub instructions: u64,
    pub call_depth: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            instructions: 1_000_000,
            call_depth: 256,
        }
    }
}

// TODO デバッグ用のスタックトレース
pub struct VM<'a> {
    cache: &'a Cache,
    limits: ExecutionLimits,
    trace: Vec<TraceFrame>,
}

impl<'a> VM<'a> {
    pub fn new(cache: &'a Cache) -> Self {
        Self::with_limits(cache, ExecutionLimits::default())
    }

    pub fn with_limits(cache: &'a Cache, limits: ExecutionLimits) -> Self {
        Self {
            cache,
            limits,
            trace: Vec::new(),
        }
    }

    pub fn stack_trace(&self) -> &[TraceFrame] {
        &self.trace
    }

    fn capture_trace(&mut self, frames: &[Frame<'_>], pending: bool) {
        self.trace = frames
            .iter()
            .rev()
            .enumerate()
            .map(|(index, frame)| TraceFrame {
                function: frame.function_id,
                instruction: if pending && index == 0 {
                    frame.ip
                } else {
                    frame.ip.saturating_sub(1)
                },
            })
            .collect();
    }

    pub fn run_function(&mut self, func_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        self.trace.clear();
        if self.limits.call_depth == 0 {
            return Err(VMError::StackOverflow);
        }
        let frame = self.load_function(func_id, args)?;
        self.execute_frame(frame)
    }

    fn execute_frame(&mut self, frame: Frame<'a>) -> VMResult<Option<Value>> {
        // 順次実行
        let mut frame_stack = vec![frame];
        let mut remaining = self.limits.instructions;
        loop {
            // Falling off a function executes no instruction and consumes no budget.
            while frame_stack
                .last()
                .is_some_and(|frame| frame.ip == frame.code.len())
            {
                frame_stack.pop();
            }
            if frame_stack.is_empty() {
                return Ok(None);
            }
            if remaining == 0 {
                self.capture_trace(&frame_stack, true);
                return Err(VMError::InstructionLimitExceeded);
            }
            remaining -= 1;
            let ret = match self.next(&mut frame_stack) {
                Ok(value) => value,
                Err(error) => {
                    self.capture_trace(&frame_stack, false);
                    return Err(error);
                }
            };
            if frame_stack.is_empty() {
                return Ok(ret);
            }
        }
    }

    fn next(&self, frame_stack: &mut Vec<Frame<'a>>) -> VMResult<Option<Value>> {
        let depth = frame_stack.len();
        let Some(frame) = frame_stack.last_mut() else {
            return Ok(None);
        };
        let Some(inst) = frame.code.get(frame.ip) else {
            frame_stack.pop();
            return Ok(None);
        };
        frame.ip += 1;

        match inst {
            Instruction::LoadIndex { .. } | Instruction::StoreIndex { .. } => {
                Self::execute_index(frame, inst)?
            }
            Instruction::Unreachable => return Err(VMError::InvalidOperand),
            Instruction::MakeEnum { .. } | Instruction::IsVariant { .. } => {
                Self::execute_enum(frame, inst)?
            }
            Instruction::MakeStruct { .. }
            | Instruction::LoadField { .. }
            | Instruction::StoreField { .. } => Self::execute_struct(frame, inst)?,
            Instruction::MakeArray { .. }
            | Instruction::RepeatArray { .. }
            | Instruction::MakeRange { .. }
            | Instruction::IterInit { .. }
            | Instruction::IterDrop { .. }
            | Instruction::IterNext { .. } => Self::execute_collection(frame, inst)?,
            Instruction::LoadConst { dst, value } => {
                frame.registers[dst.0] = value.clone();
            }
            Instruction::Move { dst, src } => {
                frame.registers[dst.0] = frame.registers[src.0].clone();
            }
            Instruction::BinOp { dst, op, lhs, rhs } => {
                let l = &frame.registers[lhs.0];
                let r = &frame.registers[rhs.0];
                frame.registers[dst.0] = Self::eval_binop(op, l, r)?;
            }
            Instruction::Cmp { dst, op, lhs, rhs } => {
                let l = &frame.registers[lhs.0];
                let r = &frame.registers[rhs.0];
                frame.registers[dst.0] = Self::eval_cmp(op, l, r)?;
            }
            Instruction::JumpIfFalse { cond, target } => match frame.registers[cond.0] {
                Value::Bool(false) => frame.ip = *target,
                Value::Bool(true) => (),
                _ => return Err(VMError::InvalidOperand),
            },
            Instruction::Jump { target } => {
                frame.ip = *target;
            }
            Instruction::Call { dst, func, args } => {
                if depth >= self.limits.call_depth {
                    return Err(VMError::StackOverflow);
                }
                frame.next_reg = dst.map(|reg| reg.0);

                let mut call_args = Vec::with_capacity(args.len());
                for reg in args {
                    call_args.push(frame.registers[reg.0].clone());
                }

                let func = self.load_function(*func, call_args)?;
                frame_stack.push(func);
            }
            Instruction::Return { value } => {
                let Some(mut frame) = frame_stack.pop() else {
                    return Ok(None);
                };
                let Some(value) = value else {
                    return Ok(None);
                };

                // The popped frame is never executed again, so transfer its owned return value.
                let value = core::mem::replace(&mut frame.registers[value.0], Value::Unit);

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
        }

        Ok(None)
    }

    fn execute_collection(frame: &mut Frame<'_>, inst: &Instruction) -> VMResult<()> {
        match inst {
            Instruction::MakeArray {
                dst,
                elements,
                element_type,
            } => {
                frame.registers[dst.0] = Value::Array {
                    elements: elements
                        .iter()
                        .map(|reg| frame.registers[reg.0].clone())
                        .collect(),
                    element_type: element_type.clone(),
                };
            }
            Instruction::RepeatArray { dst, value, count } => {
                let value = frame.registers[value.0].clone();
                frame.registers[dst.0] = Value::Array {
                    element_type: value.to_type(),
                    elements: vec![value; *count as usize],
                };
            }
            Instruction::MakeRange {
                dst,
                start,
                end,
                inclusive,
            } => {
                let (Value::Integer(start), Value::Integer(end)) =
                    (&frame.registers[start.0], &frame.registers[end.0])
                else {
                    return Err(VMError::InvalidOperand);
                };
                frame.registers[dst.0] = Value::Range {
                    start: start.clone(),
                    end: end.clone(),
                    inclusive: *inclusive,
                };
            }
            Instruction::IterInit { iterator, iterable } => {
                let state = iterator::IteratorState::new(frame.registers[iterable.0].clone())?;
                frame.iterators.insert(iterator.0, state);
            }
            Instruction::IterDrop { iterator } => {
                frame.iterators.remove(&iterator.0);
            }
            Instruction::IterNext {
                iterator,
                dst,
                exhausted,
            } => {
                let state = frame
                    .iterators
                    .get_mut(&iterator.0)
                    .ok_or(VMError::InvalidOperand)?;
                if let Some(value) = state.next() {
                    frame.registers[dst.0] = value;
                } else {
                    frame.ip = *exhausted;
                }
            }
            _ => return Err(VMError::InvalidOperand),
        }
        Ok(())
    }

    fn load_function(&self, func_id: usize, args: Vec<Value>) -> VMResult<Frame<'a>> {
        // フレーム作成
        let func = self
            .cache
            .get_function(&func_id)
            .ok_or(VMError::IllegalFunctionCall)?;
        if args.len() != func.parameters.len() {
            return Err(VMError::ArgumentCountMismatch);
        }
        if args
            .iter()
            .zip(&func.parameters)
            .any(|(value, expected)| value.to_type() != *expected)
        {
            return Err(VMError::ArgumentTypeMismatch);
        }
        let mut frame = Frame::new(func.registers, &func.code, args);
        frame.function_id = func_id;
        Ok(frame)
    }

    fn eval_binop(op: &BinaryOperator, l: &Value, r: &Value) -> VMResult<Value> {
        if let (Value::Integer(a), Value::Integer(b)) = (l, r) {
            return vectors::checked_integer(op, a, b).map(Value::Integer);
        }
        if matches!(l, Value::Matrix(_)) || matches!(r, Value::Matrix(_)) {
            return Self::eval_matrix_operator(op, l, r);
        }
        if matches!(l, Value::Vector(_)) || matches!(r, Value::Vector(_)) {
            return Self::eval_vector_operator(op, l, r);
        }
        if let (Value::Float(a), Value::Float(b)) = (l, r) {
            if a.to_type() != b.to_type() {
                return Err(VMError::InvalidOperand);
            }
            return Ok(Value::Float(match op {
                BinaryOperator::Addition => a + b,
                BinaryOperator::Subtract => a - b,
                BinaryOperator::Multiply => a * b,
                BinaryOperator::Divide => a / b,
                _ => return Err(VMError::InvalidOperand),
            }));
        }
        Err(VMError::InvalidOperand)
    }

    fn eval_cmp(op: &ComparisonOperator, l: &Value, r: &Value) -> VMResult<Value> {
        use core::cmp::Ordering;
        let ordering = match (l, r) {
            (Value::Integer(a), Value::Integer(b)) if a.to_type() == b.to_type() => {
                a.partial_cmp(b)
            }
            (Value::Float(a), Value::Float(b)) if a.to_type() == b.to_type() => a.partial_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            _ => return Err(VMError::InvalidOperand),
        };
        let result = match op {
            ComparisonOperator::Equal => ordering == Some(Ordering::Equal),
            ComparisonOperator::NotEqual => ordering != Some(Ordering::Equal),
            ComparisonOperator::LessThan => ordering == Some(Ordering::Less),
            ComparisonOperator::LessThanEqual => {
                matches!(ordering, Some(Ordering::Less | Ordering::Equal))
            }
            ComparisonOperator::GreaterThan => ordering == Some(Ordering::Greater),
            ComparisonOperator::GreaterThanEqual => {
                matches!(ordering, Some(Ordering::Greater | Ordering::Equal))
            }
        };
        Ok(Value::Bool(result))
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
