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

        let id = SymbolId(self.next_symbol);
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
            id: FunctionType::UserDefined(self.next_function),
        };
        self.next_function += 1;
        self.functions.insert(name, id);
        id
    }

    fn resolve_built_in(&self, name: &str) -> FunctionId {
        // TODO: 別Crateで定義して呼び出す形に(VM側も)
        let id = match name {
            "matrix" => FunctionType::BuiltIn(0),
            _ => panic!("undefined function name: {name:?}"),
        };

        FunctionId { id }
    }

    fn resolve_function(&self, name: &str) -> FunctionId {
        if let Some(func) = self.functions.get(name) {
            return *func;
        }

        self.resolve_built_in(name)
    }

    fn lower_item(&mut self, item: &'a Spanned<Item>) -> HIRModule {
        let mut functions = Vec::new();
        let mut globals = Vec::new();

        match item.get_node() {
            Item::Function {
                name,
                params,
                return_type: _,
                body,
            } => {
                let func = self.lower_function(name, params, body.as_ref());
                functions.push(func);
            }
            _ => {}
        }

        HIRModule { functions, globals }
    }

    fn lower_function(
        &mut self,
        name: &'a str,
        params: &'a [Spanned<FunctionParam>],
        body: Option<&'a SpannedBox<Expression>>,
    ) -> HIRFunction {
        const DEFAULT_TYPE: TypeId = TypeId(0);

        let func_id = self.alloc_function(&name);
        let mut hir_params = Vec::new();

        for param in params {
            let Some(pattern) = &param.get_node().pattern else {
                unreachable!();
            };

            if let Pattern::Identifier { ident, .. } = &pattern.node {
                let sym = self.alloc_symbol(&ident);
                hir_params.push((sym, DEFAULT_TYPE));
            }
        }

        let body = body
            .map(|b| self.lower_block_expr(&b))
            .unwrap_or_else(|| HIRBlock {
                statements: Vec::new(),
                result: None,
            });

        HIRFunction {
            id: func_id,
            params: hir_params,
            return_type: DEFAULT_TYPE,
            body,
        }
    }

    fn lower_block_expr(&mut self, expr: &'a SpannedBox<Expression>) -> HIRBlock {
        match expr.get_node() {
            Expression::Block(statements) => {
                let mut hir_statements = Vec::new();
                for statement in statements {
                    if let Some(s) = self.lower_statement(statement) {
                        hir_statements.push(s);
                    }
                }

                HIRBlock {
                    statements: hir_statements,
                    result: None,
                }
            }
            _ => HIRBlock {
                statements: Vec::new(),
                result: Some(Box::new(self.lower_expr(expr))),
            },
        }
    }

    fn lower_statement(&mut self, statement: &'a Spanned<Statement>) -> Option<HIRStatement> {
        const DEFAULT_TYPE: TypeId = TypeId(0);

        match statement.get_node() {
            Statement::Expression(expr) => Some(HIRStatement::Expr(self.lower_expr(&expr))),

            _ => None,
        }
    }

    // 削除
    fn lower_expr(&mut self, expr: &'a Expression) -> HIRExpression {
        match expr {
            Expression::Path(path) => {
                let name = path.node.last_ident();
                HIRExpression::Variable(self.resolve_symbol(name))
            }

            Expression::Return(expr) => {
                HIRExpression::Return(expr.as_ref().map(|e| Box::new(self.lower_expr(&e))))
            }

            Expression::Break { expression, .. } => {
                HIRExpression::Break(expression.as_ref().map(|e| Box::new(self.lower_expr(&e))))
            }

            _ => unimplemented!(),
        }
    }

    pub fn lower_let_statement(
        &mut self,
        pattern: &Pattern,
        initializer: Option<HIRExpression>,
    ) -> DesugarResult<Option<HIRStatement>> {
        let Pattern::Identifier { ident, .. } = pattern else {
            return Ok(None);
        };
        let symbol = self.alloc_symbol(&ident);
        let init = initializer.as_ref().map(|e| self.lower_expr(&e));

        let hir = HIRStatement::Let {
            symbol,
            ty: INITIAL_ID,
            init,
        };

        Ok(Some(hir))
    }

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

        let lhs = HIRExpression::Variable(target);
        let hir = HIRExpression::Store {
            target,
            value: Box::new(HIRExpression::BinOp {
                op: operator,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }),
        };

        Ok(hir)
    }

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
        // TODO

        let hir = HIRExpression::CmpOp {
            op: ComparisonOperator::Equal,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };

        Ok(hir)
    }

    pub fn lower_call(
        &mut self,
        function: HIRExpression,
        arguments: Vec<HIRExpression>,
    ) -> DesugarResult<HIRExpression> {
        let HIRExpression::Variable(id) = function else {
            return Err(DesugarError::NotFunction);
        };

        let hir = HIRExpression::Call {
            function: Box::new(function),
            args: arguments,
        };

        Ok(hir)
    }

    pub fn lower_if_statement(
        &mut self,
        condition: HIRExpression,
        then_body: HIRExpression,
        else_body: Option<HIRExpression>,
    ) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::If {
            cond: Box::new(condition),
            then_block: Box::new(then_body),
            else_block: else_body.map(|e| Box::new(e)),
        };

        Ok(hir)
    }

    pub fn lower_loop(&mut self, body: HIRExpression) -> DesugarResult<HIRExpression> {
        let hir = HIRExpression::Loop {
            body: Box::new(body),
        };
        Ok(hir)
    }

    pub fn lower_while(
        &mut self,
        condition: HIRExpression,
        body: HIRExpression,
    ) -> DesugarResult<HIRExpression> {
        // while cond { body }
        // を以下のようにする
        // loop { if cond { body } else { break } }
        let if_expr = HIRExpression::If {
            cond: Box::new(condition),
            then_block: Box::new(body),
            else_block: Some(Box::new(HIRExpression::Block {
                statements: vec![HIRStatement::Expr(HIRExpression::Break(None))],
                result: None,
            })),
        };

        let hir = HIRExpression::Loop {
            body: Box::new(HIRExpression::Block {
                statements: vec![HIRStatement::Expr(if_expr)],
                result: None,
            }),
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
