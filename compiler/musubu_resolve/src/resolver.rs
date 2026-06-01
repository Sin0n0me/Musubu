use crate::errors::ResolveError;
use crate::{Lowered, ResolveResult, Resolver};
use alloc::string::ToString;
use musubu_ast::*;
use musubu_hir::{HIRBlock, HIRExpression, HIRFunction, HIRFunctionParam, HIRStatement};
use musubu_name_space::errors::NameSpaceError;
use musubu_name_space::{FunctionItem, ItemStoreReader, ItemSymbol};
use musubu_primitive::{
    BinaryOperator, ComparisonOperator, LogicalOperator, PrimitiveType, ToPrimitiveType,
};
use musubu_scope::{SymbolStore, TypeOption, TypeRequirement, TypeSymbol};
use musubu_span::*;

impl<'a> Resolver<'a> {
    pub(crate) fn resolve_item(
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
                    self.resolve_type(field.node.field_type.as_ref_spanned())?;
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
        // 事前importで解決済み
        if !self.collector.remove(name) {
            self.import_function(name, params, return_type)?;
        }

        let FunctionItem {
            id,
            name: _,
            return_type,
            arguments,
        } = self
            .name_resolver
            .get_function(name)
            .cloned()
            .ok_or(ResolveError::NameSpaceError(
                NameSpaceError::UnresolvedFunction {
                    name: name.to_string(),
                },
            ))?;

        // 定義のみ
        let Some(body_expr) = &body else {
            return Ok(());
        };
        if arguments.len() != params.len() {
            unreachable!()
        }
        let arguments = &arguments;

        // 関数本体
        let hir = self.enter_function(return_type.clone(), |s| {
            let params = arguments.into_iter().zip(params).collect::<Vec<_>>();
            let args = s.resolve_arguments(params)?;
            let type_symbol = return_type.clone();
            let return_type = return_type.type_kind.clone();
            let body = s.resolve_expression(body_expr)?.hir.to_block();

            let hir = s.desugar.lower_function(args, return_type, body)?;

            Ok(Lowered { type_symbol, hir })
        })?;

        self.desugar.add_function_to_module(id, hir);

        Ok(())
    }

    fn resolve_arguments(
        &mut self,
        arguments: Vec<(&TypeSymbol, &'a Spanned<FunctionParam>)>,
    ) -> ResolveResult<Vec<HIRFunctionParam>> {
        let mut args = Vec::with_capacity(arguments.len());
        for (resolved_type, param) in arguments {
            let param = param.get_node();
            let pattern = param.pattern.as_ref_spanned();

            let (id, type_requirement) = self.resolve_pattern(&pattern, Some(&resolved_type))?;
            let TypeRequirement::Expect(type_symbol) = type_requirement else {
                unimplemented!()
            };

            args.push(HIRFunctionParam {
                argument: id,
                argument_type: type_symbol.type_kind,
            });
        }

        Ok(args)
    }

    fn resolve_struct(
        &mut self,
        name: &'a str,
        fields: &'a [Spanned<StructField>],
    ) -> ResolveResult<()> {
        // 事前importで解決済み
        if self.collector.remove(name) {
            return Ok(());
        }

        self.import_struct(name, fields)
    }

    fn resolve_enumeration(
        &mut self,
        enum_name: &'a str,
        items: &'a [Spanned<EnumItem>],
    ) -> ResolveResult<()> {
        // 事前importで解決済み
        if self.collector.remove(enum_name) {
            return Ok(());
        }

        self.import_enumeration(enum_name, items)
    }

    fn resolve_expression(
        &mut self,
        expression: &Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lowered = match expression.get_node() {
            Expression::Literal(literal) => self.resolve_literal(literal.as_ref_spanned())?,
            Expression::Path(path) => {
                let (hir, type_symbol) = self.resolve_path(path.as_ref_spanned())?.split();
                let Some(hir) = hir else {
                    return Err(ResolveError::ExpectedValuePathButFoundType {
                        name: path.get_node().to_string(),
                    });
                };
                Lowered { type_symbol, hir }
            }
            Expression::Binary {
                left,
                right,
                operator,
            } => self.resolve_binary_operator(
                operator,
                left.as_ref_spanned(),
                right.as_ref_spanned(),
            )?,
            Expression::Assign {
                left,
                right,
                operator,
            } => self.resolve_assign_operator(
                operator,
                left.as_ref_spanned(),
                right.as_ref_spanned(),
            )?,
            Expression::Comparison {
                left,
                right,
                operator,
            } => self.resolve_comparison_operator(
                operator,
                left.as_ref_spanned(),
                right.as_ref_spanned(),
            )?,
            Expression::Logical {
                left,
                right,
                operator,
            } => self.resolve_logical_operator(
                operator,
                left.as_ref_spanned(),
                right.as_ref_spanned(),
            )?,
            Expression::Call {
                function,
                arguments,
            } => self.resolve_call_expression(
                function.as_ref_spanned(),
                arguments
                    .iter()
                    .map(|arg| arg.as_ref_spanned())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?,
            Expression::Block(statements) => {
                let (hir, type_symbol) = self.resolve_block(statements)?.split();
                Lowered {
                    hir: HIRExpression::Block(hir),
                    type_symbol,
                }
            }
            Expression::If {
                condition,
                then_body,
                else_body,
            } => self.resolve_if_statement(
                condition.as_ref_spanned(),
                then_body.as_ref_spanned(),
                else_body.as_ref().map(|body| body.as_ref_spanned()),
            )?,
            Expression::Loop(loop_expr) => self.resolve_loop(loop_expr.as_ref_spanned())?,
            Expression::Return(expr_opt) => {
                self.resolve_return(expr_opt.as_ref().map(|expr| expr.as_ref_spanned()))?
            }
            Expression::Array { elements } => self.resolve_array(elements)?,
            Expression::FieldAccess { parent, field_name } => {
                self.resolve_field_access(&parent.as_ref_spanned(), &field_name)?
            }
            Expression::MethodCall(method) => self.resolve_method_call(method)?,
            Expression::Index { parent, index } => {
                self.resolve_index(parent.as_ref_spanned(), index.as_ref_spanned())?
            }
            Expression::Continue { label } => {
                self.resolve_continue(label.as_ref().map(|s| s.as_str()))?
            }
            Expression::Break { label, expression } => self.resolve_break(
                label.as_ref().map(|s| s.as_str()),
                expression.as_ref().map(|expr| expr.as_ref_spanned()),
            )?,
        };

        Ok(lowered)
    }

    fn resolve_break(
        &mut self,
        _label: Option<&'a str>,
        expression: Option<Spanned<&'a Expression>>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (expr_hir, expr_type) = if let Some(expr) = expression {
            let (hir, ty) = self.resolve_expression(&expr)?.split();
            (Some(hir), Some(ty))
        } else {
            (None, None)
        };

        let type_symbol = expr_type.unwrap_or_default();
        let hir = self.desugar.lower_break(expr_hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_continue(
        &mut self,
        _label: Option<&'a str>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let type_symbol = TypeSymbol::default();
        let hir = self.desugar.lower_continue()?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_binary_operator(
        &mut self,
        operator: &BinaryOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(&left)?;
        let rhs = self.resolve_expression(&right)?;

        let type_symbol =
            self.type_checker
                .check_binary_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desugar
            .lower_binary_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_assign_operator(
        &mut self,
        operator: &AssignOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(&left)?;
        let rhs = self.resolve_expression(&right)?;

        let type_symbol =
            self.type_checker
                .check_assign_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desugar
            .lower_assign_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_comparison_operator(
        &mut self,
        operator: &ComparisonOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(&left)?;
        let rhs = self.resolve_expression(&right)?;

        let type_symbol = self.type_checker.check_comparison_operator(
            operator,
            lhs.type_symbol,
            rhs.type_symbol,
        )?;
        let hir = self
            .desugar
            .lower_comparison_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_logical_operator(
        &mut self,
        operator: &LogicalOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(&left)?;
        let rhs = self.resolve_expression(&right)?;

        let type_symbol =
            self.type_checker
                .check_logical_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desugar
            .lower_logical_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_call_expression(
        &mut self,
        function: Spanned<&'a Expression>,
        arguments: &[Spanned<&'a Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let call = self.resolve_expression(&function)?;
        let args = arguments
            .into_iter()
            .map(|arg| self.resolve_expression(&arg))
            .collect::<Result<Vec<_>, ResolveError>>()?;

        let type_symbol = self.type_checker.check_function_call(
            &call.type_symbol,
            args.iter()
                .map(|l| &l.type_symbol)
                .collect::<Vec<_>>()
                .as_slice(),
        )?;
        let hir = self
            .desugar
            .lower_call(call.hir, args.into_iter().map(|l| l.hir).collect())?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_block(
        &mut self,
        statements: &'a [Spanned<Statement>],
    ) -> ResolveResult<Lowered<HIRBlock>> {
        if statements.is_empty() {
            return Ok(Lowered {
                type_symbol: TypeSymbol::default(),
                hir: HIRBlock {
                    statements: Vec::new(),
                },
            });
        }

        self.enter_scope(|s| {
            let mut return_type = TypeSymbol::default();
            let mut hir_statements = Vec::new();
            for statement in statements {
                if let Some(stat) = s.resolve_statement(statement.as_ref_spanned())? {
                    hir_statements.push(stat.hir);
                    return_type = stat.type_symbol;
                }
            }

            let hir = Lowered {
                type_symbol: return_type,
                hir: HIRBlock {
                    statements: hir_statements,
                },
            };

            Ok(hir)
        })
    }

    fn resolve_if_statement(
        &mut self,
        condition: Spanned<&'a Expression>,
        then_body: Spanned<&'a Expression>,
        else_body: Option<Spanned<&'a Expression>>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let condition = self.resolve_expression(&condition)?;
        let then_body = self.resolve_expression(&then_body)?;
        let (else_body_hir, else_body_type) = if let Some(expr) = else_body {
            let (hir, ty) = self.resolve_expression(&expr)?.split();
            (Some(hir), Some(ty))
        } else {
            (None, None)
        };

        let type_symbol = self.type_checker.check_if_statement(
            condition.type_symbol,
            then_body.type_symbol,
            else_body_type,
        )?;
        let hir = self.desugar.lower_if_statement(
            condition.hir,
            then_body.hir.to_block(),
            else_body_hir.map(|l| l.to_block()),
        )?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_return(
        &mut self,
        expression: Option<Spanned<&'a Expression>>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (expr_hir, expr_ty) = if let Some(expr) = expression {
            let (hir, ty) = self.resolve_expression(&expr)?.split();
            (Some(hir), Some(ty))
        } else {
            (None, None)
        };

        self.type_checker.check_return(expr_ty.as_ref())?;
        let type_symbol = expr_ty.unwrap_or_default();
        let hir = self.desugar.lower_return(expr_hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_field_access(
        &mut self,
        expression: &Spanned<&'a Expression>,
        field_name: &'a str,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        unimplemented!()
        //Ok(Lowered { type_symbol, hir })
    }

    fn resolve_method_call(
        &mut self,
        method: &'a MethodCall,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        for param in &method.params {
            self.resolve_expression(&param.as_ref_spanned())?;
        }

        unimplemented!();
        // Ok(Lowered { type_symbol, hir })
    }

    fn resolve_array(
        &mut self,
        elements: &'a ArrayElements,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        match elements {
            ArrayElements::List(list) => self.resolve_array_list(
                list.iter()
                    .map(|expr| expr.as_ref_spanned())
                    .collect::<Vec<_>>()
                    .as_slice(),
            ),
            ArrayElements::Repeat { value, count } => {
                self.resolve_array_repeat(value.as_ref_spanned(), count.as_ref_spanned())
            }
        }
    }

    fn resolve_array_list(
        &mut self,
        list: &[Spanned<&'a Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        for expr in list {
            self.resolve_expression(&expr)?;
        }

        unimplemented!();

        //Ok(Lowered { type_symbol, hir })
    }

    fn resolve_array_repeat(
        &mut self,
        value: Spanned<&'a Expression>,
        count: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        unimplemented!()
        //Ok(Lowered { type_symbol, hir })
    }

    fn resolve_index(
        &mut self,
        parent: Spanned<&'a Expression>,
        index: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let p = self.resolve_expression(&parent)?;
        let i = self.resolve_expression(&index)?;

        Ok(p)
    }

    fn resolve_path(
        &mut self,
        path: Spanned<&'a Path>,
    ) -> ResolveResult<Lowered<Option<HIRExpression>>> {
        //
        let path = path.node;
        let name = path.last_ident();

        let Some(type_symbol) = self.name_resolver.get_type(name).cloned() else {
            return Err(ResolveError::UnresolvedPath {
                name: name.to_string(),
            });
        };

        if let Some(id) = self.name_resolver.get_variable_id(name) {
            let hir = self
                .desugar
                .lower_symbol(id.clone(), type_symbol.type_kind.clone())?;

            return Ok(Lowered {
                type_symbol,
                hir: Some(hir),
            });
        }

        if self.name_resolver.is_type(name) {
            return Ok(Lowered {
                type_symbol,
                hir: None,
            });
        }

        let Some(item) = self.name_resolver.get_item(name) else {
            return Err(ResolveError::UnresolvedPath {
                name: name.to_string(),
            });
        };

        let hir = match item {
            ItemSymbol::Function(function_item) => {
                let id = function_item.id.clone();
                let return_type = function_item.return_type.type_kind.clone();
                let hir = self.desugar.lower_function_symbol(id, return_type)?;
                Some(hir)
            }
            ItemSymbol::Enumeration(enum_item) => None,
            ItemSymbol::Struct(struct_item) => None,
        };

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_literal(
        &mut self,
        spanned_literal: Spanned<&'a Literal>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        // TODO
        let literal = &spanned_literal.node;
        match literal {
            Literal::Float { value: _, .. } => {}
            Literal::Integer { value: _, .. } => {}
            Literal::Char { value: _, .. } => {}
            Literal::UnicodeChar { value: _, .. } => {}
            Literal::String { value: _, .. } => {}
            Literal::Bool(_) => {}
        };

        let scope = self.get_scope()?;
        let type_symbol = self.type_checker.check_literal(scope, literal)?;
        let hir = self.desugar.lower_literal(literal)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_statement(
        &mut self,
        statement: Spanned<&'a Statement>,
    ) -> ResolveResult<Option<Lowered<HIRStatement>>> {
        match statement.get_node() {
            Statement::Expression(expr) => {
                let (hir, type_symbol) = self.resolve_expression(&expr.as_ref_spanned())?.split();

                Ok(Some(Lowered {
                    type_symbol,
                    hir: hir.to_statement(),
                }))
            }
            Statement::Let {
                name,
                initializer,
                variable_type,
                label,
            } => {
                let (hir, type_symbol) = self
                    .resolve_let_statement(
                        name,
                        initializer.as_ref().map(|expr| expr.as_ref_spanned()),
                        variable_type.as_ref().map(|expr| expr.as_ref_spanned()),
                        label.as_ref().map(|s| s.as_str()),
                    )?
                    .split();

                Ok(hir.map(|hir| Lowered { type_symbol, hir }))
            }
            Statement::Item(item) => {
                self.resolve_item(item.as_ref_spanned())?;
                Ok(None)
            }
            Statement::Semicolon => Ok(None),
        }
    }

    fn resolve_let_statement(
        &mut self,
        name: &'a Spanned<Pattern>,
        initializer: Option<Spanned<&'a Expression>>,
        variable_type: Option<Spanned<&'a TypeKind>>,
        _label: Option<&'a str>,
    ) -> ResolveResult<Lowered<Option<HIRStatement>>> {
        // 型
        let variable_type = if let Some(variable_type) = variable_type {
            Some(self.resolve_type(variable_type)?)
        } else {
            None
        };

        // 初期化式
        let (initializer_hir, initializer_type) = if let Some(init_expr) = initializer {
            let (hir, ty) = self.resolve_expression(&init_expr)?.split();
            (Some(hir), Some(ty))
        } else {
            (None, None)
        };

        // パターン
        let pattern = name.as_ref_spanned();
        let type_kind = variable_type.as_ref().or(initializer_type.as_ref());
        let _ = self.resolve_pattern(&pattern, type_kind)?;

        // 型チェック
        let scope = self.get_scope()?;
        let type_symbol = self.type_checker.check_let_statenent(
            scope,
            &name.node,
            initializer_type,
            variable_type,
        )?;
        let hir = self
            .desugar
            .lower_let_statement(pattern.get_node(), initializer_hir)?;

        // TODO: 推論中
        let Some(type_symbol) = type_symbol else {
            unimplemented!();
        };

        Ok(Lowered { type_symbol, hir })
    }

    // patternは定義でしか現れない
    fn resolve_pattern(
        &mut self,
        pattern: &Spanned<&'a Pattern>,
        type_kind: Option<&TypeSymbol>, // 事前に型が決まっている場合
    ) -> ResolveResult<(usize, TypeRequirement)> {
        let span = pattern.span;
        let pattern = &pattern.node;
        let id = self.desugar.alloc_symbol(); // 変数の割り当て

        let ty = match pattern {
            Pattern::Identifier {
                ident,
                mutable,
                reference,
            } => {
                let option = TypeOption {
                    mutable: *mutable,
                    reference: *reference,
                };
                let ty = if let Some(type_symbol) = type_kind {
                    TypeRequirement::Expect(TypeSymbol {
                        type_kind: type_symbol.type_kind.clone(),
                        option,
                    })
                } else {
                    TypeRequirement::Inferring(option)
                };

                self.name_resolver.add_variable(id, ident, ty.clone())?;
                ty
            }
            Pattern::Multiply(patterns) => {
                for pattern in patterns {
                    self.resolve_pattern(&pattern.as_ref_spanned(), None)?;
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

        Ok((id, ty))
    }

    fn resolve_loop(
        &mut self,
        loop_expr: Spanned<&'a LoopExpr>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        match &loop_expr.node {
            LoopExpr::Loop { body } => self.resolve_loop_expr(body.as_ref_spanned()),
            LoopExpr::While { condition, body } => {
                self.resolve_while_expr(condition.as_ref_spanned(), body.as_ref_spanned())
            }
            LoopExpr::For {
                pattern,
                iterator,
                body,
            } => self.resolve_for_expr(
                pattern.as_ref_spanned(),
                iterator.as_ref_spanned(),
                body.as_ref_spanned(),
            ),
        }
    }

    fn resolve_loop_expr(
        &mut self,
        body: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (body_hir, body_ty) = self.resolve_expression(&body)?.split();

        let scope = self.get_scope()?;
        let type_symbol = self.type_checker.check_loop_expr(scope, body_ty)?;
        let hir = self.desugar.lower_loop(body_hir.to_block())?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_while_expr(
        &mut self,
        condition: Spanned<&'a Expression>,
        body: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (condition_hir, condition_ty) = self.resolve_expression(&condition)?.split();
        let (body_hir, body_ty) = self.resolve_expression(&body)?.split();

        let scope = self.get_scope()?;
        let type_symbol = self
            .type_checker
            .check_while_expr(scope, condition_ty, body_ty)?;
        let hir = self
            .desugar
            .lower_while(condition_hir, body_hir.to_block())?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_for_expr(
        &mut self,
        pattern: Spanned<&'a Pattern>,
        iterator: Spanned<&'a Expression>,
        body: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (iterator_hir, iterator_ty) = self.resolve_expression(&iterator)?.split();
        let (body_hir, body_ty) = self.resolve_expression(&body)?.split();
        let _ = self.resolve_pattern(&pattern, None)?; // TODO

        let scope = self.get_scope()?;
        let type_symbol = self
            .type_checker
            .check_for_expr(scope, iterator_ty, body_ty)?;
        let hir = self
            .desugar
            .lower_for(pattern.get_node(), iterator_hir, body_hir.to_block())?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_type(&mut self, type_kind: Spanned<&'a TypeKind>) -> ResolveResult<TypeSymbol> {
        let type_kind = &type_kind.node;
        let scope = self.get_scope()?;
        let ty = self.type_checker.check_type(scope, type_kind)?;

        match type_kind {
            TypeKind::Primitive(_) => {}
            TypeKind::PathType(path) => {
                self.resolve_path(path.as_ref_spanned())?;
            }
            TypeKind::Function {
                arguments,
                return_type,
            } => {
                for arg in arguments {
                    self.resolve_type(arg.as_ref_spanned())?;
                }
                self.resolve_type(return_type.as_ref_spanned())?;
            }
        }

        Ok(ty)
    }

    fn resolve_type_alias(&mut self, alias: Spanned<&'a TypeAlias>) -> ResolveResult<()> {
        //self.insert(&alias.node.name, ResolvedSymbol::Type)?;

        self.resolve_type(Spanned {
            node: &alias.node.target,
            span: alias.span,
        })?;

        Ok(())
    }
}
