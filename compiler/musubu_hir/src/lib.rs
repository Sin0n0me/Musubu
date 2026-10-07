#![no_std]

extern crate alloc;
mod match_expression;
pub use match_expression::{HIRMatchArm, HIRMatchPattern};

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::{vec, vec::Vec};
use musubu_primitive::*;

#[derive(Debug, Clone)]
pub struct HIRModule {
    pub functions: BTreeMap<usize, HIRFunction>,
    pub globals: Vec<HIRGlobal>,
    /// Half-open UTF-8 byte ranges of function bodies, keyed by function ID.
    pub function_ranges: BTreeMap<usize, (usize, usize)>,
}

impl HIRModule {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            globals: Vec::new(),
            function_ranges: BTreeMap::new(),
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
    pub params: Vec<HIRFunctionParam>,
    pub return_type: PrimitiveType,
    pub body: HIRBlock,
}

#[derive(Debug, Clone)]
pub struct HIRFunctionParam {
    pub argument: usize,
    pub argument_type: PrimitiveType,
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
    Discard(HIRExpression),
}

impl ToPrimitiveType for HIRStatement {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Let { .. } => PrimitiveType::Unit,
            Self::Expr(e) => e.to_type(),
            Self::Discard(_) => PrimitiveType::Unit,
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
    Enum {
        variant: usize,
        fields: Vec<(usize, HIRExpression)>,
        enum_type: PrimitiveType,
    },
    Match {
        value: Box<HIRExpression>,
        arms: Vec<HIRMatchArm>,
        result_type: PrimitiveType,
    },
    Struct {
        fields: Vec<(usize, HIRExpression)>,
        struct_type: PrimitiveType,
    },
    Field {
        parent: Box<HIRExpression>,
        index: usize,
        field_type: PrimitiveType,
    },
    StoreField {
        target: usize,
        path: Vec<usize>,
        value: Box<HIRExpression>,
    },
    Array {
        elements: Vec<HIRExpression>,
        element_type: PrimitiveType,
    },
    ArrayRepeat {
        value: Box<HIRExpression>,
        count: u32,
    },
    Range {
        start: Box<HIRExpression>,
        end: Box<HIRExpression>,
        inclusive: bool,
    },
    For {
        symbol: usize,
        iterator: Box<HIRExpression>,
        body: HIRBlock,
    },
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
        return_type: PrimitiveType,
        arguments: Vec<HIRExpression>,
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
        result_type: PrimitiveType,
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
            Self::Enum { enum_type, .. } => enum_type.clone(),
            Self::Match { result_type, .. } => result_type.clone(),
            Self::Struct { struct_type, .. } => struct_type.clone(),
            Self::Field { field_type, .. } => field_type.clone(),
            Self::StoreField { .. } => PrimitiveType::Unit,
            Self::Array {
                elements,
                element_type,
            } => PrimitiveType::Array {
                type_kind: Box::new(element_type.clone()),
                size: elements.len() as u32,
            },
            Self::ArrayRepeat { value, count } => PrimitiveType::Array {
                type_kind: Box::new(value.to_type()),
                size: *count,
            },
            Self::Range { start, .. } => PrimitiveType::Range {
                type_kind: Box::new(start.to_type()),
            },
            Self::For { .. } | Self::Store { .. } => PrimitiveType::Unit,
            Self::Variable { id: _, symbol_type } => symbol_type.clone(),
            Self::CmpOp { .. } => PrimitiveType::Boolean,
            Self::BinOp { lhs, rhs, .. } => {
                let right = rhs.to_type();
                if matches!(right, PrimitiveType::Vector { .. }) {
                    right
                } else {
                    lhs.to_type()
                }
            }
            Self::Return(expr) => expr.as_ref().map_or(PrimitiveType::Unit, |e| e.to_type()),
            Self::Literal(v) => v.to_type(),
            Self::Continue => PrimitiveType::Unit,
            Self::Loop { result_type, .. } => result_type.clone(),
            Self::Break(expr) => expr.as_ref().map_or(PrimitiveType::Unit, |e| e.to_type()),
            Self::Block(b) => b.to_type(),
            Self::If {
                cond: _,
                then_block,
                else_block: _,
            } => then_block.to_type(),
            Self::Call {
                function: _,
                return_type,
                arguments: _,
            } => return_type.clone(),
        }
    }
}
