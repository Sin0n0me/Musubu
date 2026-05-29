// TODO
//#![no_std]

extern crate alloc;

pub mod errors;

use crate::errors::DesugarError;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::{vec, vec::Vec};
use musubu_ast::*;
use musubu_cache::Allocator;
use musubu_hir::*;
use musubu_primitive::*;

pub type DesugarResult<T> = Result<T, DesugarError>;

#[derive(Debug)]
pub struct Desugar<'a> {
    next_symbol: usize,
    root_module: &'a mut HIRModule,
    function_allocator: &'a mut dyn Allocator,
}

// HIRに変換するだけ
// あくまでASTからHIRできるかだけを見る
impl<'a> Desugar<'a> {
    const INITIAL_ID: usize = 0;

    pub fn new(module: &'a mut HIRModule, allocator: &'a mut impl Allocator) -> Self {
        Self {
            next_symbol: Self::INITIAL_ID,
            root_module: module,
            function_allocator: allocator,
        }
    }

    pub fn alloc_function(&mut self, name: String) -> usize {
        self.function_allocator.alloc(name)
    }

    pub fn add_function_to_module(&mut self, id: usize, function: HIRFunction) {
        self.root_module.add_function(id, function);
    }

    pub fn alloc_symbol(&mut self) -> usize {
        let id = self.next_symbol;
        self.next_symbol += 1;

        id
    }

    pub fn lower_function(
        &mut self,
        params: Vec<(usize, PrimitiveType)>,
        return_type: PrimitiveType,
        body: HIRBlock,
    ) -> DesugarResult<HIRFunction> {
        let hir = HIRFunction {
            params,
            return_type,
            body,
        };

        Ok(hir)
    }

    pub fn lower_function_symbol(&self, id: usize) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Call {
            function: id,
            args: Vec::new(),
        };

        Ok(hir)
    }

    pub fn lower_symbol(
        &mut self,
        id: usize,
        symbol_type: PrimitiveType,
    ) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Variable { id, symbol_type };

        Ok(hir)
    }

    pub fn lower_block(&mut self, body: Vec<HIRStatement>) -> DesugarResult<HIRBlock> {
        let hir = HIRBlock { statements: body };
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

    pub fn lower_return(&mut self, expr: Option<HIRExpression>) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Return(expr.map(|hir| Box::new(hir)));
        Ok(hir)
    }

    pub fn lower_let_statement(
        &mut self,
        pattern: &Pattern,
        initializer: Option<HIRExpression>,
    ) -> DesugarResult<Option<HIRStatement>> {
        let Pattern::Identifier { .. } = pattern else {
            return Ok(None);
        };
        let symbol = self.alloc_symbol();
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
        let HIRExpression::Variable {
            id: target,
            symbol_type: _,
        } = lhs
        else {
            return Err(DesugarError::UnsupportedAssignTarget);
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

    // 論理演算は条件分岐に変換
    pub fn lower_logical_operator(
        &mut self,
        operator: LogicalOperator,
        lhs: HIRExpression,
        rhs: HIRExpression,
    ) -> DesugarResult<HIRExpression> {
        let hir = match operator {
            LogicalOperator::Or => {
                let then_body = self.lower_literal(&Literal::Bool(true))?.to_block();
                let else_body = rhs.to_block();
                self.lower_if_statement(lhs, then_body, Some(else_body))?
            }
            LogicalOperator::And => {
                let then_body = rhs.to_block();
                let else_body = self.lower_literal(&Literal::Bool(false))?.to_block();
                self.lower_if_statement(lhs, then_body, Some(else_body))?
            }
            LogicalOperator::Not => {
                // TODO ASTの構築部分がまだなので
                unimplemented!()
            }
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
        let mut function = function;
        match &mut function {
            HIRExpression::Call { function: _, args } => {
                *args = arguments;
                Ok(function)
            }

            // 関数ポインタ
            HIRExpression::Variable { id, symbol_type } => {
                let PrimitiveType::Function {
                    return_type: _,
                    arguments: _,
                } = symbol_type
                else {
                    return Err(DesugarError::NotFunction);
                };
                let hir = HIRExpression::Call {
                    function: *id,
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
        pattern: &Pattern,
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
            _ => unimplemented!(), // TODO
        };

        Ok(HIRExpression::Literal(value))
    }
}
