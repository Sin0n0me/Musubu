use musubu_primitive::*;

#[derive(Debug, Clone)]
pub struct CompiledFunction {
    pub code: Vec<Instruction>,
    pub registers: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Register(pub usize);

#[derive(Debug, Clone)]
pub enum Instruction {
    LoadConst {
        dst: Register,
        value: Value,
    },

    // 代入
    Move {
        dst: Register,
        src: Register,
    },

    // 二項演算
    BinOp {
        dst: Register,
        op: BinaryOperator,
        lhs: Register,
        rhs: Register,
    },

    // 比較
    Cmp {
        dst: Register,
        op: ComparisonOperator,
        lhs: Register,
        rhs: Register,
    },

    // 分岐
    JumpIfFalse {
        cond: Register,
        target: usize,
    },

    Jump {
        target: usize,
    },

    // 関数呼び出し
    Call {
        dst: Option<Register>,
        func: usize,
        args: Vec<Register>,
    },

    // return
    Return {
        value: Option<Register>,
    },

    // Callへ統合できるならしたいね
    BuiltInCall {
        dst: Option<Register>,
        func: usize,
        args: Vec<Register>,
    },
}

pub enum BuiltIn {
    MakeMatrix4,
}
