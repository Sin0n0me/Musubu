use crate::iterator::IteratorState;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use musubu_ir::{Instruction, Register};
use musubu_primitive::{Integer, Value};

//const HALT: usize = usize::MAX;
const INITIAL_IP: usize = 0;

#[derive(Debug)]
pub(crate) struct Frame<'a> {
    pub iterators: BTreeMap<usize, IteratorState>,
    pub registers: Vec<Value>, // TODO 初期化コストが重たいのでVM側に移動して1つのVecでやる(オフセットで位置を定められる)
    pub ip: usize,
    pub code: &'a [Instruction],
    pub next_reg: Option<usize>,
}

impl<'a> Frame<'a> {
    pub fn new(registers: usize, code: &'a [Instruction], mut args: Vec<Value>) -> Self {
        // Reuse the owned argument storage; initialize only the remaining registers once.
        args.resize(registers, Value::Integer(Integer::Int32(0)));
        Self {
            iterators: BTreeMap::new(),
            registers: args,
            ip: INITIAL_IP,
            code,
            next_reg: None,
        }
    }
}
