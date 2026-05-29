// TODO
//#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use musubu_primitive::*;

#[derive(Debug, Clone)]
pub struct HIRModule {
    pub functions: BTreeMap<usize, HIRFunction>,
    pub globals: Vec<HIRGlobal>,
}

impl HIRModule {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            globals: Vec::new(),
        }
    }

    pub fn add_function(&mut self, id: usize, function: HIRFunction) {
        self.functions.insert(id, function);
    }

    pub fn add_global(&mut self, global: HIRGlobal) {
        self.globals.push(global);
    }

    pub fn get_function(&self, function_id: &usize) -> Option<&HIRFunction> {
        self.functions.get(function_id)
    }
}

#[derive(Debug, Clone)]
pub struct HIRFunction {
    pub params: Vec<(usize, PrimitiveType)>,
    pub return_type: PrimitiveType,
    pub body: HIRBlock,
}

#[derive(Debug, Clone)]
pub struct HIRGlobal {
    pub symbol: usize,
    pub symbol_type: PrimitiveType,
    pub initializer: Option<HIRExpression>,
}

#[derive(Debug, Clone)]
pub enum HIRStatement {
    Let {
        symbol: usize,
        symbol_type: PrimitiveType,
        initializer: Option<HIRExpression>,
    },
    Expr(HIRExpression),
}

impl ToPrimitiveType for HIRStatement {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Let { .. } => PrimitiveType::Unit,
            Self::Expr(e) => e.to_type(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HIRBlock {
    pub statements: Vec<HIRStatement>,
}

impl ToPrimitiveType for HIRBlock {
    fn to_type(&self) -> PrimitiveType {
        self.statements
            .last()
            .cloned()
            .map_or(PrimitiveType::Unit, |s| s.to_type())
    }
}

#[derive(Debug, Clone)]
pub enum HIRExpression {
    // 即値
    Literal(Value),

    // 変数参照
    Variable {
        id: usize,
        symbol_type: PrimitiveType,
    },

    // 代入
    Store {
        target: usize,
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
        function: usize,
        args: Vec<HIRExpression>,
    },

    // 条件分岐
    If {
        cond: Box<HIRExpression>,
        then_block: HIRBlock,
        else_block: Option<HIRBlock>,
    },

    Block(HIRBlock),

    // 繰り返し
    Loop {
        body: HIRBlock,
    },

    Continue,

    Break(Option<Box<HIRExpression>>),

    Return(Option<Box<HIRExpression>>),
}

impl HIRExpression {
    pub fn to_statement(self) -> HIRStatement {
        HIRStatement::Expr(self)
    }

    pub fn to_block(self) -> HIRBlock {
        HIRBlock {
            statements: vec![self.to_statement()],
        }
    }
}

impl ToPrimitiveType for HIRExpression {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Store { target: _, value } => value.to_type(),
            Self::Variable { id: _, symbol_type } => symbol_type.clone(),
            Self::CmpOp { op: _, lhs, rhs: _ } => lhs.to_type(),
            Self::BinOp { op: _, lhs, rhs: _ } => lhs.to_type(),
            Self::Return(expr) => expr.as_ref().map_or(PrimitiveType::Unit, |e| e.to_type()),
            Self::Literal(v) => v.to_type(),
            Self::Continue => PrimitiveType::Unit,
            Self::Loop { body } => body.to_type(),
            Self::Break(expr) => expr.as_ref().map_or(PrimitiveType::Unit, |e| e.to_type()),
            Self::Block(b) => b.to_type(),
            Self::If {
                cond: _,
                then_block,
                else_block: _,
            } => then_block.to_type(),
            Self::Call {
                function: _,
                args: _,
            } => PrimitiveType::Unit,
        }
    }
}
