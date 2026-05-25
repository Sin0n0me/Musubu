// TODO
//#![no_std]

extern crate alloc;

pub mod errors;

use crate::errors::DesugarError;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::{vec, vec::Vec};
use musubu_ast::*;
use musubu_hir::*;
use musubu_primitive::*;
use musubu_span::*;

pub type DesugarResult<T> = Result<T, DesugarError>;

#[derive(Debug)]
pub struct Desugar<'a> {
    next_symbol: usize,
    next_function: usize,
    pub variables: BTreeMap<&'a str, SymbolId>,
    pub functions: BTreeMap<&'a str, FunctionId>,
}

// HIRに変換するだけ
// あくまでASTからHIRできるかだけを見る
impl<'a> Desugar<'a> {
    const INITIAL_ID: usize = 0;

    pub fn new() -> Self {
        Self {
            next_symbol: Self::INITIAL_ID,
            next_function: Self::INITIAL_ID,
            variables: BTreeMap::new(),
            functions: BTreeMap::new(),
        }
    }

    fn alloc_symbol(&mut self, name: &'a str) -> SymbolId {
        if let Some(id) = self.variables.get(name) {
            return *id;
        }

        let id = SymbolId {
            id: self.next_symbol,
        };
        self.next_symbol += 1;
        self.variables.insert(name, id);
        id
    }

    fn resolve_symbol(&self, name: &str) -> SymbolId {
        *self.variables.get(name).expect("undefined variable")
    }

    pub fn alloc_function(&mut self, name: &'a str) -> FunctionId {
        if let Some(id) = self.functions.get(name) {
            return *id;
        }

        let id = FunctionId {
            id: self.next_function,
        };
        self.next_function += 1;
        self.functions.insert(name, id);
        id
    }

    fn resolve_function(&self, name: &str) -> FunctionId {
        if let Some(func) = self.functions.get(name) {
            return *func;
        }

        // self.resolve_built_in(name)

        FunctionId { id: 0usize }
    }

    pub fn lower_block(&mut self, body: Vec<HIRStatement>) -> DesugarResult<HIRBlock> {
        let hir = HIRBlock { statements: body };
        Ok(hir)
    }

    pub fn lower_path(
        &mut self,
        name: &'a str,
        symbol_type: PrimitiveType,
    ) -> DesugarResult<HIRExpression> {
        let id = self.resolve_symbol(name);
        let hir = HIRExpression::Variable { id, symbol_type };
        Ok(hir)
    }

    pub fn lower_continue(&mut self) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Continue;
        Ok(hir)
    }

    pub fn lower_break(&mut self, expr: Option<HIRExpression>) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Break(expr.map(|hir| Box::new(hir)));
        Ok(hir)
    }

    pub fn lower_retrun(&mut self, expr: Option<HIRExpression>) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Return(expr.map(|hir| Box::new(hir)));
        Ok(hir)
    }

    pub fn lower_let_statement(
        &mut self,
        pattern: &'a Pattern,
        initializer: Option<HIRExpression>,
    ) -> DesugarResult<Option<HIRStatement>> {
        let Pattern::Identifier { ident, .. } = pattern else {
            return Ok(None);
        };
        let symbol = self.alloc_symbol(&ident);
        let symbol_type = initializer
            .as_ref()
            .map_or(PrimitiveType::Unit, |e| e.to_type());

        let hir = HIRStatement::Let {
            symbol,
            symbol_type,
            initializer,
        };

        Ok(Some(hir))
    }

    // バイナリ演算はそのまま変換
    pub fn lower_binary_operator(
        &mut self,
        operator: BinaryOperator,
        lhs: HIRExpression,
        rhs: HIRExpression,
    ) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::BinOp {
            op: operator,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        Ok(hir)
    }

    // a += b, a -= b
    // などを以下のように分解する
    // a = a + b
    // a = a - b
    pub fn lower_assign_operator(
        &mut self,
        operator: AssignOperator,
        lhs: HIRExpression,
        rhs: HIRExpression,
    ) -> DesugarResult<HIRExpression> {
        let HIRExpression::Store { target, value: _ } = lhs else {
            return Err(DesugarError::UnsupportAssignTarget);
        };

        let operator = match operator {
            AssignOperator::AddAssign => BinaryOperator::Addition,
            AssignOperator::SubAssign => BinaryOperator::Subtract,
            AssignOperator::MulAssign => BinaryOperator::Multiply,
            AssignOperator::DivAssign => BinaryOperator::Divide,
            AssignOperator::ModAssign => BinaryOperator::Modulo,
            AssignOperator::AndAssign => BinaryOperator::And,
            AssignOperator::OrAssign => BinaryOperator::Or,
            AssignOperator::XorAssign => BinaryOperator::Xor,
            AssignOperator::LeftShiftAssign => BinaryOperator::LeftShift,
            AssignOperator::RightShiftAssign => BinaryOperator::RightShift,
            AssignOperator::Assign => {
                return Ok(HIRExpression::Store {
                    target,
                    value: Box::new(rhs),
                });
            }
        };

        let lhs = HIRExpression::Variable {
            id: target,
            symbol_type: lhs.to_type(),
        };
        let hir = HIRExpression::Store {
            target,
            value: Box::new(self.lower_binary_operator(operator, lhs, rhs)?),
        };

        Ok(hir)
    }

    // 比較演算子もそのまま変換
    pub fn lower_comparison_operator(
        &mut self,
        operator: ComparisonOperator,
        lhs: HIRExpression,
        rhs: HIRExpression,
    ) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::CmpOp {
            op: operator,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        Ok(hir)
    }

    pub fn lower_logical_operator(
        &mut self,
        operator: LogicalOperator,
        lhs: HIRExpression,
        rhs: HIRExpression,
    ) -> DesugarResult<HIRExpression> {
        // TODO 条件分岐に変換
        let hir = HIRExpression::LogOp {
            op: operator,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };

        Ok(hir)
    }

    // Expression
    // Pathであれば定義された関数の呼び出し
    // 直接構築された関数であればその展開
    pub fn lower_call(
        &mut self,
        function: HIRExpression,
        arguments: Vec<HIRExpression>,
    ) -> DesugarResult<HIRExpression> {
        /*
        let HIRExpression::Variable { id, symbol_type } = function else {
            return Err(DesugarError::NotFunction);
        };
         * */

        match function {
            HIRExpression::Variable { id, symbol_type } => {
                let hir = HIRExpression::Call {
                    function: FunctionId { id: id.id },
                    args: arguments,
                };

                Ok(hir)
            }
            _ => Err(DesugarError::NotFunction),
        }
    }

    pub fn lower_array(&mut self) -> DesugarResult<HIRExpression> {
        // TODO
        let hir = HIRExpression::Continue;

        Ok(hir)
    }

    pub fn lower_if_statement(
        &mut self,
        condition: HIRExpression,
        then_body: HIRBlock,
        else_body: Option<HIRBlock>,
    ) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::If {
            cond: Box::new(condition),
            then_block: then_body,
            else_block: else_body,
        };

        Ok(hir)
    }

    // loopはそのまま
    pub fn lower_loop(&mut self, body: HIRBlock) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Loop { body };
        Ok(hir)
    }

    // while cond {
    //  body
    // }
    // を以下のようにする
    // loop {
    //  if cond {
    //   body
    //  } else {
    //   break
    //  }
    // }
    pub fn lower_while(
        &mut self,
        condition: HIRExpression,
        body: HIRBlock,
    ) -> DesugarResult<HIRExpression> {
        let else_body = HIRExpression::Break(None).to_block();
        let if_expr = self.lower_if_statement(condition, body, Some(else_body))?;

        let hir = HIRExpression::Loop {
            body: if_expr.to_block(),
        };

        Ok(hir)
    }

    // for i in iter {
    //  body
    // }
    //
    // loop {
    //   if let Some(i) = iter.next() {
    //      body
    //   } else {
    //      break;
    //   }
    // }
    pub fn lower_for(
        &mut self,
        pattern: &'a Pattern,
        iterator: HIRExpression,
        body: HIRBlock,
    ) -> DesugarResult<HIRExpression> {
        unimplemented!();

        // TODO
        let initializer = Some(HIRExpression::Continue);
        let iter = self.lower_let_statement(pattern, initializer)?;
        let condition = HIRExpression::Continue;
        let then_body = body;
        let else_body = HIRExpression::Break(None).to_block();
        let if_expr = self.lower_if_statement(condition, then_body, Some(else_body))?;

        let hir = HIRExpression::Loop {
            body: if_expr.to_block(),
        };

        Ok(hir)
    }

    pub fn lower_literal(&mut self, literal: &Literal) -> DesugarResult<HIRExpression> {
        let value = match literal {
            Literal::Integer { value, value_type } => Value::Integer(match value_type {
                TypeKind::Primitive(ty) => Integer::new(&value, ty).expect(""),
                _ => unimplemented!(), // TODO
            }),
            Literal::Float { value, value_type } => Value::Float(match value_type {
                TypeKind::Primitive(ty) => Float::new(&value, ty).expect(""),
                _ => unimplemented!(), // TODO
            }),
            Literal::Bool(b) => Value::Bool(*b),
            Literal::String { value, .. } => Value::String(value.clone()),
            _ => unimplemented!(),
        };

        Ok(HIRExpression::Literal(value))
    }
}
