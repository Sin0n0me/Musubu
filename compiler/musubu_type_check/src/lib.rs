// TODO
//#![no_std]

extern crate alloc;

pub mod errors;

use crate::errors::TypeCheckError;
use alloc::boxed::Box;
use alloc::vec::Vec;
use musubu_ast::{AssignOperator, Expression, Literal, LogicalOperator, Path, Pattern, TypeKind};
use musubu_primitive::*;
use musubu_scope::errors::ScopeError;
use musubu_scope::{Scope, ScopeControl, SymbolStore, TypeSymbol};

pub type TypeCheckResult<T> = Result<T, TypeCheckError>;

#[derive(Debug)]
pub struct TypeChecker {
    scope_return_stack: Vec<TypeSymbol>,
    function_return_stack: Vec<TypeSymbol>,
}

impl ScopeControl<TypeCheckError> for TypeChecker {
    fn on_exit_scope(&mut self) -> Result<(), TypeCheckError> {
        if self.scope_return_stack.pop().is_none() {
            return Err(TypeCheckError::ScopeError(ScopeError::InvalidScope));
        }

        Ok(())
    }

    fn on_enter_scope(&mut self) -> Result<(), TypeCheckError> {
        self.scope_return_stack.push(TypeSymbol::default());
        Ok(())
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            scope_return_stack: Vec::new(),
            function_return_stack: Vec::new(),
        }
    }

    pub fn enter_function(&mut self, return_type: TypeSymbol) {
        self.function_return_stack.push(return_type);
    }

    pub fn exit_function(&mut self) {
        self.function_return_stack.pop();
    }

    pub fn set_scope_return_type(&mut self, return_type: TypeSymbol) -> TypeCheckResult<()> {
        if self.scope_return_stack.pop().is_none() {
            return Err(TypeCheckError::ScopeError(ScopeError::InvalidScope));
        }
        self.scope_return_stack.push(return_type);

        Ok(())
    }

    pub fn check_binary_operator(
        &self,
        operator: &BinaryOperator,
        lhs: TypeSymbol,
        rhs: TypeSymbol,
    ) -> TypeCheckResult<TypeSymbol> {
        self.validate_binary_operand(&lhs, &rhs)?;

        match operator {
            BinaryOperator::Addition
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide => {
                if !lhs.type_kind.is_scalar_type() {
                    return Err(TypeCheckError::InvalidOperation {
                        op: format!("{:?} lhs: {:?} rhs: {:?}", operator, lhs, rhs),
                        reason: "unsupported binary operator".into(),
                    });
                }
            }
            BinaryOperator::Modulo
            | BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::Xor
            | BinaryOperator::LeftShift
            | BinaryOperator::RightShift => {
                if !lhs.type_kind.is_integer() {
                    return Err(TypeCheckError::InvalidOperation {
                        op: format!("{:?}", operator),
                        reason: "unsupported binary operator".into(),
                    });
                }
            }
        }

        Ok(lhs)
    }

    pub fn check_assign_operator(
        &self,
        operator: &AssignOperator,
        lhs: TypeSymbol,
        rhs: TypeSymbol,
    ) -> TypeCheckResult<TypeSymbol> {
        self.validate_binary_operand(&lhs, &rhs)?;

        if !lhs.is_mutable() {
            return Err(TypeCheckError::NotMutable {
                name: lhs.type_kind.to_string(), // TODO
            });
        }

        match operator {
            AssignOperator::Assign
            | AssignOperator::AddAssign
            | AssignOperator::SubAssign
            | AssignOperator::MulAssign
            | AssignOperator::DivAssign => {
                if !lhs.type_kind.is_scalar_type() {
                    return Err(TypeCheckError::InvalidOperation {
                        op: format!("{:?}", operator),
                        reason: "unsupported assign operator".into(),
                    });
                }
            }
            AssignOperator::ModAssign
            | AssignOperator::AndAssign
            | AssignOperator::OrAssign
            | AssignOperator::XorAssign
            | AssignOperator::LeftShiftAssign
            | AssignOperator::RightShiftAssign => {
                if !lhs.type_kind.is_integer() {
                    return Err(TypeCheckError::InvalidOperation {
                        op: format!("{:?}", operator),
                        reason: "unsupported assign operator".into(),
                    });
                }
            }
        }

        Ok(lhs)
    }

    pub fn check_comparison_operator(
        &self,
        operator: &ComparisonOperator,
        lhs: TypeSymbol,
        rhs: TypeSymbol,
    ) -> TypeCheckResult<TypeSymbol> {
        self.validate_binary_operand(&lhs, &rhs)?;

        match operator {
            ComparisonOperator::Equal | ComparisonOperator::NotEqual => {
                if !lhs.type_kind.is_integer() {
                    return Err(TypeCheckError::InvalidOperation {
                        op: format!("{:?}", operator),
                        reason: "unsupported comparison operator".into(),
                    });
                }
            }
            ComparisonOperator::LessThan
            | ComparisonOperator::LessThanEqual
            | ComparisonOperator::GreaterThan
            | ComparisonOperator::GreaterThanEqual => {
                if !lhs.type_kind.is_scalar_type() {
                    return Err(TypeCheckError::InvalidOperation {
                        op: format!("{:?}", operator),
                        reason: "unsupported comparison operator".into(),
                    });
                }
            }
        }

        Ok(lhs)
    }

    pub fn check_logical_operator(
        &self,
        operator: &LogicalOperator,
        lhs: TypeSymbol,
        rhs: TypeSymbol,
    ) -> TypeCheckResult<TypeSymbol> {
        self.validate_binary_operand(&lhs, &rhs)?;

        if !lhs.type_kind.is_boolean() {
            return Err(TypeCheckError::InvalidOperation {
                op: format!("{:?}", operator),
                reason: "unsupported logical operator".into(),
            });
        }

        Ok(lhs)
    }

    pub fn check_literal<'a>(
        &self,
        scope: &Scope<'a>,
        literal: &Literal,
    ) -> TypeCheckResult<TypeSymbol> {
        let type_kind = match literal {
            Literal::Float { value_type, .. }
            | Literal::Integer { value_type, .. }
            | Literal::Char { value_type, .. }
            | Literal::UnicodeChar { value_type, .. }
            | Literal::String { value_type, .. } => value_type.clone(),
            Literal::Bool(_) => TypeKind::Primitive(PrimitiveType::Boolean),
        };

        self.check_type(scope, &type_kind)
    }

    pub fn check_let_statenent<'a>(
        &self,
        scope: &Scope<'a>,
        pattern: &'a Pattern,
        initializer: Option<TypeSymbol>,
        variable_type: Option<TypeSymbol>,
    ) -> TypeCheckResult<Option<TypeSymbol>> {
        let type_symbol = match (initializer, variable_type) {
            (Some(initializer), Some(variable_type)) => {
                if !initializer.is_same_type(&variable_type) {
                    return Err(TypeCheckError::TypeMismatch {
                        expected: variable_type.type_kind,
                        found: initializer.type_kind,
                    });
                }
                self.resolve_pattern(scope, pattern, &variable_type.type_kind)?;
                Some(variable_type)
            }
            (Some(initializer), None) => {
                self.resolve_pattern(scope, pattern, &initializer.type_kind)?;
                Some(initializer)
            }
            (None, Some(variable_type)) => {
                self.resolve_pattern(scope, pattern, &variable_type.type_kind)?;
                Some(variable_type)
            }
            (None, None) => {
                None // 型がない場合は推論
            }
        };

        Ok(type_symbol)
    }

    pub fn check_if_statement(
        &self,
        condition: TypeSymbol,
        then_body: TypeSymbol,
        else_body: Option<TypeSymbol>,
    ) -> TypeCheckResult<TypeSymbol> {
        // 条件式
        if !matches!(condition.type_kind, PrimitiveType::Boolean) {
            return Err(TypeCheckError::TypeMismatch {
                expected: PrimitiveType::Boolean,
                found: condition.type_kind.clone(),
            });
        }

        if condition.type_kind.is_pointer() {
            return Err(TypeCheckError::TypeMismatch {
                expected: PrimitiveType::Boolean,
                found: PrimitiveType::Pointer {
                    point: Box::new(condition.type_kind.clone()),
                },
            });
        }

        // 条件式の戻り型チェック
        let Some(else_body) = else_body else {
            return Ok(then_body);
        };

        if !then_body.is_same_type(&else_body) {
            return Err(TypeCheckError::TypeMismatch {
                expected: then_body.type_kind,
                found: else_body.type_kind,
            });
        }

        Ok(then_body)
    }

    pub fn check_return(&mut self, return_type: TypeSymbol) -> TypeCheckResult<TypeSymbol> {
        let expected = self
            .function_return_stack
            .last()
            .ok_or(TypeCheckError::InvalidReturnScope)?;

        if !expected.is_same_type(&return_type) {
            return Err(TypeCheckError::FunctionReturnMismatch {
                expected: expected.type_kind.clone(),
                found: return_type.type_kind,
            });
        }

        Ok(return_type)
    }

    pub fn check_type<'a>(
        &self,
        scope: &Scope<'a>,
        type_kind: &TypeKind,
    ) -> TypeCheckResult<TypeSymbol> {
        let ty = match type_kind {
            TypeKind::Primitive(ty) => TypeSymbol::new(ty.clone()),
            TypeKind::Function {
                arguments,
                return_type,
            } => {
                let return_type = self.check_type(scope, &return_type.node)?;
                let mut params = Vec::new();
                for arg in arguments {
                    params.push(self.check_type(scope, &arg.node)?.type_kind);
                }

                TypeSymbol::new(PrimitiveType::Function {
                    return_type: Box::new(return_type.type_kind),
                    arguments: params,
                })
            }
            TypeKind::PathType(path) => self.check_path(scope, &path.node)?,
        };

        Ok(ty)
    }

    pub fn check_path<'a>(&self, scope: &Scope<'a>, path: &Path) -> TypeCheckResult<TypeSymbol> {
        // ジェネリクスの型チェック
        for segment in &path.segments {
            let segment = &segment.node;
            for arg in &segment.arguments {
                let arg = &arg.node;
                self.check_type(scope, &arg)?;
            }
        }

        // TODO: パスの処理
        let name = path.last_ident();
        let ty = scope
            .get_type(name)
            .ok_or(TypeCheckError::UnknownType {
                name: name.to_string(),
            })?
            .clone();

        Ok(ty)
    }

    pub fn check_function_call(
        &self,
        function: &TypeSymbol,
        arguments: &[&TypeSymbol],
    ) -> TypeCheckResult<TypeSymbol> {
        let PrimitiveType::Function {
            return_type,
            arguments,
        } = function.type_kind
        else {
            return Err(TypeCheckError::NotCallable {
                found: function.type_kind.clone(),
            });
        };

        Ok(())
    }

    fn validate_binary_operand(&self, lhs: &TypeSymbol, rhs: &TypeSymbol) -> TypeCheckResult<()> {
        if !lhs.is_same_type(rhs) {
            return Err(TypeCheckError::TypeMismatch {
                expected: lhs.type_kind.clone(),
                found: rhs.type_kind.clone(),
            });
        }

        if lhs.type_kind.is_pointer() {
            return Err(TypeCheckError::TypeMismatch {
                expected: lhs.type_kind.clone(),
                found: rhs.type_kind.clone(),
            });
        }

        Ok(())
    }

    pub fn resolve_pattern<'a>(
        &self,
        scope: &Scope<'a>,
        pattern: &'a Pattern,
        variable_type: &PrimitiveType,
    ) -> TypeCheckResult<()> {
        match pattern {
            Pattern::Identifier { ident, .. } => {
                if !scope.contains(ident) {
                    return Err(TypeCheckError::UnknownPattern {
                        name: ident.to_string(),
                    });
                }

                //scope.resolve_variable_type(ident, variable_type)?;
            }
            Pattern::Multiply(patterns) => {
                let PrimitiveType::Struct { elements } = variable_type else {
                    unimplemented!() // TODO
                    // return Err(TypeCheckError::);
                };

                let expected_count = patterns.len();
                let found_count = elements.len();
                if expected_count != found_count {
                    return Err(TypeCheckError::TupleCountMismatch {
                        expected: expected_count,
                        found: found_count,
                    });
                }

                // TODO
            }
            Pattern::Literal(literal) => {}
            Pattern::None => {}
        }

        Ok(())
    }
}
