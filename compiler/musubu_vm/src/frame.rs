use alloc::{vec, vec::Vec};
use musubu_ir::{Instruction, Register};
use musubu_primitive::{Integer, Value};

//const HALT: usize = usize::MAX;
const INITIAL_IP: usize = 0;

#[derive(Debug)]
pub(crate) struct Frame<'a> {
    pub registers: Vec<Value>,
    pub ip: usize,
    pub code: &'a [Instruction],
    pub next_reg: usize,
}

impl<'a> Frame<'a> {
    pub fn new(registers: usize, code: &'a [Instruction], args: Vec<Value>) -> Self {
        let mut frame = Self {
            registers: Vec::with_capacity(registers),
            ip: INITIAL_IP,
            code,
            next_reg: args.len() + 1,
        };
        frame.init(args);
        frame
    }

    pub fn init(&mut self, args: Vec<Value>) {
        let args_len = args.len();

        // 引数セット
        for (i, arg) in args.into_iter().enumerate() {
            self.registers[i] = arg;
        }

        self.registers[args_len..].fill(Value::Integer(Integer::Int32(0)));
        self.ip = INITIAL_IP;
    }
}
