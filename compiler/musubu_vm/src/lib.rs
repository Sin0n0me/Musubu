mod built_in;
mod cache;

use built_in::*;
use cache::get_cache;
use musubu_hir::*;
use musubu_ir::*;
use musubu_primitive::*;

pub struct VM {}

#[derive(Debug)]
pub struct Frame {
    pub registers: Vec<Value>,
    pub ip: usize,
    pub code: Vec<Instruction>,
}

impl VM {
    pub fn new() -> Self {
        Self {}
    }

    pub fn register_function(hash: usize, function: CompiledFunction) {
        let cache = get_cache();
        let mut map = cache.write().unwrap();
        map.insert(hash, function);
    }

    pub fn run_function(&self, func_id: usize, args: Vec<Value>) -> Option<Value> {
        const INITIAL_IP: usize = 0;

        let cache = get_cache();
        let binding = cache.read().ok()?;
        let func = binding.get(&func_id)?;

        // フレーム作成
        let mut frame = Frame {
            registers: vec![Value::Unit; func.registers],
            ip: INITIAL_IP,
            code: func.code.clone(),
        };

        // 引数セット
        for (i, arg) in args.into_iter().enumerate() {
            frame.registers[i] = arg;
        }

        //panic!("{frame:#?}");

        // 順次実行
        loop {
            const HALT: usize = usize::MAX;
            if frame.ip == HALT {
                return None;
            }

            // TODO: to Err
            let inst = frame.code.get(frame.ip)?;
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
                    let mut call_args = Vec::with_capacity(args.len());
                    for reg in args {
                        call_args.push(frame.registers[reg.0].clone());
                    }

                    // TODO: stackフレーム管理へ
                    // Rust自体のスタックオーバーフロー(再帰呼び出しによる)が起こる可能性がある
                    let ret = self.run_function(*func, call_args);

                    if let (Some(dst), Some(val)) = (dst, ret) {
                        frame.registers[dst.0] = val;
                    }
                }
                Instruction::Return { value } => {
                    return value.map(|r| frame.registers[r.0].clone());
                }
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
            }
        }
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
