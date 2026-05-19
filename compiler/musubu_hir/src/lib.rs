// TODO
//#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use musubu_primitive::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId {
    pub id: FunctionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionType {
    UserDefined(usize),
    BuiltIn(usize),
}

#[derive(Debug, Clone)]
pub struct HIRModule {
    pub functions: Vec<HIRFunction>,
    pub globals: Vec<HIRGlobal>,
}

#[derive(Debug, Clone)]
pub struct HIRFunction {
    pub id: FunctionId,
    pub params: Vec<(SymbolId, TypeId)>,
    pub return_type: TypeId,
    pub body: HIRExpression,
}

#[derive(Debug, Clone)]
pub struct HIRGlobal {
    pub symbol: SymbolId,
    pub type_id: TypeId,
    pub initializer: Option<HIRExpression>,
}

#[derive(Debug, Clone)]
pub enum HIRStatement {
    Let {
        symbol: SymbolId,
        ty: TypeId,
        init: Option<HIRExpression>,
    },
    Expr(HIRExpression),
}

#[derive(Debug, Clone)]
pub enum HIRExpression {
    // 即値
    Literal(Value),

    // 変数参照
    Variable(SymbolId),

    // 代入
    Store {
        target: SymbolId,
        value: Box<HIRExpression>,
    },

    // 二項演算
    BinOp {
        op: BinaryOperator,
        lhs: Box<HIRExpression>,
        rhs: Box<HIRExpression>,
    },

    // 比較
    CmpOp {
        op: ComparisonOperator,
        lhs: Box<HIRExpression>,
        rhs: Box<HIRExpression>,
    },

    // 関数呼び出し
    Call {
        function: Box<HIRExpression>,
        args: Vec<HIRExpression>,
    },

    Block {
        statements: Vec<HIRStatement>,
        result: Option<Box<HIRExpression>>, // 式ブロック対応
    },

    If {
        cond: Box<HIRExpression>,
        then_block: Box<HIRExpression>,
        else_block: Option<Box<HIRExpression>>,
    },

    Loop {
        body: Box<HIRExpression>,
    },

    Continue,
    Break(Option<Box<HIRExpression>>),

    // return
    Return(Option<Box<HIRExpression>>),
}
