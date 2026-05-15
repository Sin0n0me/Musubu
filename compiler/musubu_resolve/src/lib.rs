#![no_std]

extern crate alloc;

pub mod errors;

mod name_resolver;

use alloc::vec::Vec;
use errors::ResolveError;
use musubu_ast::*;
use musubu_name_space::*;
use musubu_primitive::PrimitiveType;
use musubu_scope::{Scope, ScopeControl, SymbolStore, TypeOption, TypeRequirement, TypeSymbol};
use musubu_span::*;
use musubu_type_check::TypeChecker;
use name_resolver::NameResolver;

pub type ResolveResult<T> = Result<T, ResolveError>;

#[derive(Debug)]
pub struct Resolver<'a> {
    pub name_resolver: NameResolver<'a>,
    type_checker: TypeChecker,
    scope_stack: Vec<Scope<'a>>,
}

#[derive(Debug)]
struct MetaData {
    expect_type: Option<TypeSymbol>,
}

impl<'a> Resolver<'a> {
    pub fn new(project_name: &'a str) -> Self {
        Self {
            name_resolver: NameResolver::new(project_name),
            type_checker: TypeChecker::new(),
            scope_stack: Vec::new(),
        }
    }

    pub fn resolve(&mut self, module_name: &'a str, node: &'a ASTNode) -> ResolveResult<()> {
        self.enter_module(module_name, |s| {
            match node {
                ASTNode::Item { visibility, item } => {
                    s.resolve_item(item.as_ref_spanned())?;
                }
                _ => unreachable!(),
            }

            Ok(())
        })
    }

    fn resolve_item(
        &mut self,
        //visibility: &'a Visibility,
        item: Spanned<&'a Item>,
    ) -> ResolveResult<TypeSymbol> {
        match &item.node {
            Item::Function {
                name,
                params,
                body,
                return_type,
            } => self.resolve_function(
                &name,
                &params,
                body.as_ref().map(|b| b.as_ref_spanned()),
                return_type.as_ref().map(|r| r.as_ref_spanned()),
            )?,
            Item::Struct { name, fields } => self.resolve_struct(&name, fields)?,
            Item::Enumeration { name, items } => self.resolve_enumeration(&name, items)?,
            Item::Union { name, fields } => {
                for field in fields {
                    self.resolve_type(field.node.field_type.as_ref_spanned(), false)?;
                }
            }
        }

        Ok(TypeSymbol::default())
    }

    fn resolve_function(
        &mut self,
        name: &'a str,
        params: &'a [Spanned<FunctionParam>],
        body: Option<Spanned<&'a Expression>>,
        return_type: Option<Spanned<&'a TypeKind>>,
    ) -> ResolveResult<()> {
        // 戻り型
        let return_type = if let Some(return_type) = return_type {
            self.resolve_type(return_type, false)?
        } else {
            TypeSymbol::default()
        };

        // 関数本体+引数
        self.enter_function(return_type, |s| {
            let return_type = s
                .type_checker
                .get_return_type()
                .ok_or(ResolveError::InvalidRetrunType)?;
            let mut function_item = FunctionItem::new(name, return_type.clone());

            // 引数
            for param in params {
                let param = &param.node;
                if let Some(ref pattern) = param.pattern {
                    s.resolve_pattern(&pattern.as_ref_spanned())?;
                };

                let arg_type = s.resolve_type(param.param_type.as_ref_spanned(), false)?;
                function_item.add_argument(arg_type)?;
            }

            // 関数本体
            let Some(body_expr) = &body else {
                return Ok(());
            };
            let return_type = s.resolve_expression(&body_expr)?;

            s.name_resolver.add_function(function_item)?;

            Ok(())
        })
    }

    fn resolve_struct(
        &mut self,
        name: &'a str,
        fields: &'a [Spanned<StructField>],
    ) -> ResolveResult<()> {
        let mut struct_item = StructItem::new(name);
        for field in fields {
            let field = &field.node;
            let field_type = self.resolve_type(field.field_type.as_ref_spanned(), false)?;
            struct_item.add_field(&field.name, field_type);
        }

        self.name_resolver.add_struct(struct_item)?;

        Ok(())
    }

    fn resolve_enumeration(
        &mut self,
        enum_name: &'a str,
        items: &'a [Spanned<musubu_ast::EnumItem>],
    ) -> ResolveResult<()> {
        let mut enum_item = musubu_name_space::EnumItem::new(enum_name);
        for item in items {
            match &item.node {
                musubu_ast::EnumItem::StructItem {
                    name,
                    fields,
                    visibility,
                } => {
                    for field in fields {
                        let field = &field.node;
                        let field_name = &field.name;
                        let field_type =
                            self.resolve_type(field.field_type.as_ref_spanned(), false)?;

                        enum_item.add_variant_field(name, field_name, field_type);
                    }
                }
                musubu_ast::EnumItem::TupleItem { name, visibility } => {
                    enum_item.add_variant(name);
                }
            }
        }

        //self.type_checker.add_enumeration(enum_name, variants)?;

        Ok(())
    }

    fn resolve_expression(
        &mut self,
        expression: &Spanned<&'a Expression>,
    ) -> ResolveResult<TypeSymbol> {
        let ty = match expression.node {
            Expression::Literal(literal) => self.resolve_literal(literal.as_ref_spanned())?,
            Expression::Path(path) => self.resolve_path(path.as_ref_spanned(), false)?,
            Expression::Binary {
                left,
                right,
                operator,
            } => {
                let lhs = self.resolve_expression(&left.as_ref_spanned())?;
                let rhs = self.resolve_expression(&right.as_ref_spanned())?;
                self.type_checker
                    .check_binary_operator(operator, lhs, rhs)?
            }
            Expression::Assign {
                left,
                right,
                operator,
            } => {
                let lhs = self.resolve_expression(&left.as_ref_spanned())?;
                let rhs = self.resolve_expression(&right.as_ref_spanned())?;
                self.type_checker
                    .check_assign_operator(operator, lhs, rhs)?
            }
            Expression::Comparison {
                left,
                right,
                operator,
            } => {
                let lhs = self.resolve_expression(&left.as_ref_spanned())?;
                let rhs = self.resolve_expression(&right.as_ref_spanned())?;
                self.type_checker
                    .check_comparison_operator(operator, lhs, rhs)?
            }
            Expression::Logical {
                left,
                right,
                operator,
            } => {
                let lhs = self.resolve_expression(&left.as_ref_spanned())?;
                let rhs = self.resolve_expression(&right.as_ref_spanned())?;
                self.type_checker
                    .check_logical_operator(operator, lhs, rhs)?
            }
            Expression::Call {
                function,
                arguments,
            } => {
                let ty = self.resolve_expression(&function.as_ref_spanned())?;
                for argument in arguments {
                    self.resolve_expression(&argument.as_ref_spanned())?;
                }
                ty
            }
            Expression::Block(statements) => self.resolve_block(statements)?,
            Expression::If {
                condition,
                then_body,
                else_body,
            } => self.resolve_if(
                &condition.as_ref_spanned(),
                &then_body.as_ref_spanned(),
                else_body.as_ref().map(|body| body.as_ref_spanned()),
            )?,
            Expression::Loop(loop_expr) => self.resolve_loop(&loop_expr.as_ref_spanned())?,
            Expression::Return(expr_opt) => {
                self.resolve_return(expr_opt.as_ref().map(|expr| expr.as_ref_spanned()))?
            }
            Expression::Array { elements } => self.resolve_array(elements)?,
            Expression::FieldAccess { parent, field_name } => {
                self.resolve_field_access(&parent.as_ref_spanned(), &field_name)?
            }
            Expression::MethodCall(method) => self.resolve_method_call(method)?,
            Expression::Index { parent, index } => {
                let p = self.resolve_expression(&parent.as_ref_spanned())?;
                let i = self.resolve_expression(&index.as_ref_spanned())?;
                p
            }
            Expression::Continue { .. } => TypeSymbol::default(),
            Expression::Break { expression, .. } => {
                if let Some(expr) = expression {
                    self.resolve_expression(&expr.as_ref_spanned())?
                } else {
                    TypeSymbol::default()
                }
            }
        };

        Ok(ty)
    }

    fn resolve_block(&mut self, statements: &'a [Spanned<Statement>]) -> ResolveResult<TypeSymbol> {
        if statements.is_empty() {
            return Ok(TypeSymbol::default());
        }

        self.enter_scope(TypeRequirement::Inferring(TypeOption::default()), |s| {
            let mut return_type = TypeSymbol::default();
            for statement in statements {
                s.resolve_statement(statement.as_ref_spanned())?;
            }

            Ok(return_type)
        })
    }

    fn resolve_if(
        &mut self,
        condition: &Spanned<&'a Expression>,
        then_body: &Spanned<&'a Expression>,
        else_body: Option<Spanned<&'a Expression>>,
    ) -> ResolveResult<TypeSymbol> {
        let condition_ty = self.resolve_expression(condition)?;
        let then_body = self
            .enter_scope(TypeRequirement::Inferring(TypeOption::default()), |s| {
                s.resolve_expression(then_body)
            })?;
        let else_body = if let Some(expr) = else_body {
            let return_type = self
                .enter_scope(TypeRequirement::Inferring(TypeOption::default()), |s| {
                    s.resolve_expression(&expr)
                })?;
            Some(return_type)
        } else {
            None
        };

        let return_type = self
            .type_checker
            .check_if(condition_ty, then_body, else_body)?;

        Ok(return_type)
    }

    fn resolve_return(
        &mut self,
        expression: Option<Spanned<&'a Expression>>,
    ) -> ResolveResult<TypeSymbol> {
        let return_type = if let Some(expr) = expression {
            self.resolve_expression(&expr)?
        } else {
            TypeSymbol::default()
        };

        let return_type = self.type_checker.check_return(return_type)?;

        Ok(return_type)
    }

    fn resolve_field_access(
        &mut self,
        expression: &Spanned<&'a Expression>,
        field_name: &'a str,
    ) -> ResolveResult<TypeSymbol> {
        Ok(TypeSymbol::default())
    }

    fn resolve_method_call(&mut self, method: &'a MethodCall) -> ResolveResult<TypeSymbol> {
        for param in &method.params {
            self.resolve_expression(&param.as_ref_spanned())?;
        }

        Ok(TypeSymbol::default())
    }

    fn resolve_array(&mut self, elements: &'a ArrayElements) -> ResolveResult<TypeSymbol> {
        match elements {
            ArrayElements::List(list) => {
                for expr in list {
                    self.resolve_expression(&expr.as_ref_spanned())?;
                }
            }
            ArrayElements::Repeat { value, count } => {
                self.resolve_expression(&value.as_ref_spanned())?;
                self.resolve_expression(&count.as_ref_spanned())?;
            }
        }

        Ok(TypeSymbol::default())
    }

    fn resolve_path(
        &mut self,
        path: Spanned<&'a Path>,
        is_declaration: bool,
    ) -> ResolveResult<TypeSymbol> {
        //
        let ty = if is_declaration {
        } else {
            //self.name_resolver.use_path(&mut self.scope_resolver, path);
            //self.type_checker.check_path(path.node)?;
        };

        // TODO
        Ok(TypeSymbol::default())
    }

    fn resolve_literal(
        &mut self,
        spanned_literal: Spanned<&'a Literal>,
    ) -> ResolveResult<TypeSymbol> {
        let literal = &spanned_literal.node;
        let type_kind = match literal {
            Literal::Float { value_type, .. }
            | Literal::Integer { value_type, .. }
            | Literal::Char { value_type, .. }
            | Literal::UnicodeChar { value_type, .. }
            | Literal::String { value_type, .. } => value_type,
            Literal::Bool(_) => &TypeKind::Primitive(PrimitiveType::Unit),
        };

        self.resolve_type(
            Spanned {
                node: type_kind,
                span: spanned_literal.span,
            },
            false,
        )?;

        let ty = self.type_checker.check_literal(&literal)?;

        Ok(ty)
    }

    fn resolve_statement(
        &mut self,
        statement: Spanned<&'a Statement>,
    ) -> ResolveResult<TypeSymbol> {
        let ty = match &statement.node {
            Statement::Expression(expr) => self.resolve_expression(&expr.as_ref_spanned())?,
            Statement::Let {
                name,
                initializer,
                variable_type,
                label: _, // TODO
            } => {
                // パターン
                self.resolve_pattern(&name.as_ref_spanned())?;

                // 初期化式
                let initializer = if let Some(init_expr) = initializer {
                    Some(self.resolve_expression(&init_expr.as_ref_spanned())?)
                } else {
                    None
                };

                // 型
                let variable_type = if let Some(variable_type) = variable_type {
                    Some(self.resolve_type(variable_type.as_ref_spanned(), false)?)
                } else {
                    None
                };

                let scope = self.scope_stack.last().ok_or(ResolveError::InvalidScope)?;
                self.type_checker
                    .check_let(scope, &name.node, initializer, variable_type)?;

                TypeSymbol::default()
            }
            Statement::Item(item) => self.resolve_item(item.as_ref_spanned())?,
            Statement::Semicolon => TypeSymbol::default(),
        };

        Ok(ty)
    }

    // patternは定義でしか現れない
    fn resolve_pattern(
        &mut self,
        pattern: &Spanned<&'a Pattern>,
    ) -> ResolveResult<TypeRequirement> {
        let span = pattern.span;
        let pattern = &pattern.node;

        let ty = match pattern {
            Pattern::Identifier {
                ident,
                mutable,
                reference,
            } => {
                let ty = TypeRequirement::Inferring(TypeOption {
                    mutable: *mutable,
                    reference: *reference,
                });
                self.name_resolver.add_variable(&ident, ty.clone());
                ty
            }
            Pattern::Multiply(patterns) => {
                for pattern in patterns {
                    self.resolve_pattern(&pattern.as_ref_spanned())?;
                }

                TypeRequirement::Inferring(TypeOption::default())
            }
            Pattern::Literal(literal) => {
                self.resolve_literal(Spanned {
                    node: literal,
                    span,
                })?;

                TypeRequirement::Inferring(TypeOption::default())
            }
            Pattern::None => TypeRequirement::Inferring(TypeOption::default()),
        };

        //self.type_check(&pattern)?;

        Ok(ty)
    }

    fn resolve_loop(&mut self, loop_expr: &Spanned<&'a LoopExpr>) -> ResolveResult<TypeSymbol> {
        match &loop_expr.node {
            LoopExpr::Loop { body } => {
                self.resolve_expression(&body.as_ref_spanned())?;
            }
            LoopExpr::While { condition, body } => {
                self.resolve_expression(&condition.as_ref_spanned())?;
                self.resolve_expression(&body.as_ref_spanned())?;
            }
            LoopExpr::For {
                pattern,
                iterator,
                body,
            } => {
                self.resolve_expression(&iterator.as_ref_spanned())?;
                self.resolve_pattern(&pattern.as_ref_spanned())?;
                self.resolve_expression(&body.as_ref_spanned())?;
            }
        }

        Ok(TypeSymbol::default())
    }

    fn resolve_type(
        &mut self,
        type_kind: Spanned<&'a TypeKind>,
        is_declaration: bool,
    ) -> ResolveResult<TypeSymbol> {
        let type_kind = &type_kind.node;
        let ty = self.type_checker.check_type(type_kind)?;

        match type_kind {
            TypeKind::Primitive(_) => {}
            TypeKind::PathType(path) => {
                self.resolve_path(path.as_ref_spanned(), is_declaration)?;
            }
            TypeKind::Function {
                arguments,
                return_type,
            } => {
                for arg in arguments {
                    self.resolve_type(arg.as_ref_spanned(), is_declaration)?;
                }
                self.resolve_type(return_type.as_ref_spanned(), is_declaration)?;
            }
        }

        Ok(ty)
    }

    fn resolve_type_alias(
        &mut self,
        alias: Spanned<&'a TypeAlias>,
        is_declaration: bool,
    ) -> ResolveResult<()> {
        //self.insert(&alias.node.name, ResolvedSymbol::Type)?;

        self.resolve_type(
            Spanned {
                node: &alias.node.target,
                span: alias.span,
            },
            is_declaration,
        )?;

        Ok(())
    }

    fn enter_function<T>(
        &mut self,
        return_type: TypeSymbol,
        function: impl Fn(&mut Self) -> ResolveResult<T>,
    ) -> ResolveResult<T> {
        self.type_checker.enter_function(return_type.clone());
        let result = self.enter_scope(TypeRequirement::Expect(return_type), function)?;
        self.type_checker.exit_function();
        Ok(result)
    }

    fn enter_module<T>(
        &mut self,
        module_name: &'a str,
        function: impl Fn(&mut Self) -> ResolveResult<T>,
    ) -> ResolveResult<T> {
        self.name_resolver.enter_module(module_name)?;
        let result = function(self)?;
        self.name_resolver.exit_module()?;
        Ok(result)
    }

    fn enter_scope<T>(
        &mut self,
        return_type: TypeRequirement,
        function: impl Fn(&mut Self) -> ResolveResult<T>,
    ) -> ResolveResult<T> {
        self.name_resolver.on_enter_scope()?;
        self.type_checker.on_enter_scope()?;

        let result = function(self)?;

        self.type_checker.on_exit_scope()?;
        self.name_resolver.on_exit_scope()?;

        Ok(result)
    }
}
