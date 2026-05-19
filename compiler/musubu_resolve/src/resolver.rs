use crate::errors::ResolveError;
use crate::{HIR, Lowered, ResolveResult, Resolver};
use alloc::string::ToString;
use musubu_ast::*;
use musubu_hir::{HIRExpression, HIRFunction, HIRStatement};
use musubu_name_space::errors::NameSpaceError;
use musubu_name_space::{FunctionItem, ItemStoreReader};
use musubu_primitive::{BinaryOperator, ComparisonOperator};
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
            name: _,
            return_type,
            arguments,
        } = self
            .name_resolver
            .get_function(name)
            .ok_or(ResolveError::NameSpaceError(
                NameSpaceError::UnresolveFunction {
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
        let params = arguments
            .clone() // 後でcloneした要素を渡すので丸ごとcloneして構わない
            .into_iter()
            .zip(params);

        // 関数本体
        self.enter_function(return_type.clone(), |s| {
            // 引数
            for (resolved_type, param) in params.clone() {
                let param = &param.node;
                if let Some(ref pattern) = param.pattern {
                    s.resolve_pattern(&pattern.as_ref_spanned(), Some(&resolved_type))?;
                };
            }

            s.resolve_expression(&body_expr)
        })?;

        self.desuger;

        Ok(())
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
        expression: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let ty = match expression.node {
            Expression::Literal(literal) => self.resolve_literal(literal.as_ref_spanned())?,
            Expression::Path(path) => self.resolve_path(path.as_ref_spanned())?,
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
                arguments.iter().map(|arg| arg.as_ref_spanned()).collect(),
            )?,
            Expression::Block(statements) => self.resolve_block(statements)?,
            Expression::If {
                condition,
                then_body,
                else_body,
            } => self.resolve_if_statement(
                condition.as_ref_spanned(),
                then_body.as_ref_spanned(),
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

    fn resolve_binary_operator(
        &mut self,
        operator: &BinaryOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(left)?;
        let rhs = self.resolve_expression(right)?;

        let type_symbol =
            self.type_checker
                .check_binary_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desuger
            .lower_binary_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_assign_operator(
        &mut self,
        operator: &AssignOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(left)?;
        let rhs = self.resolve_expression(right)?;

        let type_symbol =
            self.type_checker
                .check_assign_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desuger
            .lower_assign_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_comparison_operator(
        &mut self,
        operator: &ComparisonOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(left)?;
        let rhs = self.resolve_expression(right)?;

        let type_symbol = self.type_checker.check_comparison_operator(
            operator,
            lhs.type_symbol,
            rhs.type_symbol,
        )?;
        let hir = self
            .desuger
            .lower_comparison_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_logical_operator(
        &mut self,
        operator: &LogicalOperator,
        left: Spanned<&'a Expression>,
        right: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let lhs = self.resolve_expression(left)?;
        let rhs = self.resolve_expression(right)?;

        let type_symbol =
            self.type_checker
                .check_logical_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desuger
            .lower_logical_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_call_expression(
        &mut self,
        function: Spanned<&'a Expression>,
        arguments: Vec<Spanned<&'a Expression>>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let call = self.resolve_expression(function)?;
        let args = arguments
            .into_iter()
            .map(|arg| self.resolve_expression(arg))
            .collect::<Result<Vec<_>, ResolveError>>()?;

        let function = function.get_node();
        let arguments = arguments
            .iter()
            .map(|arg| arg.get_node())
            .collect::<Vec<_>>();

        let type_symbol = self.type_checker.check_function_call(
            &call.type_symbol,
            args.iter().map(|l| &l.type_symbol).collect(),
        )?;
        let hir = self
            .desuger
            .lower_call(call.hir, args.into_iter().map(|l| l.hir).collect())?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_block(
        &mut self,
        statements: &'a [Spanned<Statement>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        if statements.is_empty() {
            return Ok(Lowered {
                type_symbol: TypeSymbol::default(),
                hir: HIRExpression::Block {
                    statements: Vec::new(),
                    result: TypeSymbol::default(),
                },
            });
        }

        self.enter_scope(|s| {
            let mut return_type = TypeSymbol::default();
            let mut hir_statements = Vec::new();
            for statement in statements {
                if let Some(stat) = s.resolve_statement(statement.as_ref_spanned())? {
                    hir_statements.push(stat.hir);
                }
            }

            let hir = Lowered {
                type_symbol: return_type.clone(),
                hir: HIRExpression::Block {
                    statements: hir_statements,
                    result: return_type,
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
    ) -> ResolveResult<TypeSymbol> {
        let condition_ty = self.resolve_expression(condition)?;
        let then_body = self.enter_scope(|s| s.resolve_expression(then_body))?;
        let else_body = if let Some(expr) = else_body {
            let return_type = self.enter_scope(|s| s.resolve_expression(&expr))?;
            Some(return_type)
        } else {
            None
        };

        let return_type =
            self.type_checker
                .check_if_statement(condition_ty, then_body, else_body)?;

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
                    self.resolve_expression(expr.as_ref_spanned())?;
                }
            }
            ArrayElements::Repeat { value, count } => {
                self.resolve_expression(value.as_ref_spanned())?;
                self.resolve_expression(count.as_ref_spanned())?;
            }
        }

        Ok(TypeSymbol::default())
    }

    fn resolve_path(&mut self, path: Spanned<&'a Path>) -> ResolveResult<TypeSymbol> {
        //
        let path = path.node;
        let last_name = path.last_ident();

        // 変数が優先される
        if let Some(symbol) = self.name_resolver.get_type(last_name).cloned() {
            return Ok(symbol);
        }

        Err(ResolveError::UnresolvePath {
            name: last_name.to_string(),
        })
    }

    fn resolve_literal(
        &mut self,
        spanned_literal: Spanned<&'a Literal>,
    ) -> ResolveResult<Lowered<Expression>> {
        let literal = &spanned_literal.node;
        match literal {
            Literal::Float { value, .. } => {}
            Literal::Integer { value, .. } => {}
            Literal::Char { value, .. } => {}
            Literal::UnicodeChar { value, .. } => {}
            Literal::String { value, .. } => {}
            Literal::Bool(_) => {}
        };

        let scope = self.get_scope()?;
        let ty = self.type_checker.check_literal(scope, literal)?;
        let hir = self.desuger.lower_literal(literal)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_statement(
        &mut self,
        statement: Spanned<&'a Statement>,
    ) -> ResolveResult<Option<Lowered<HIRStatement>>> {
        let hir = match statement.get_node() {
            Statement::Expression(expr) => {
                let expr = self.resolve_expression(expr.as_ref_spanned())?;
                Lowered {
                    type_symbol: expr.type_symbol,
                    hir: HIRStatement::Expr(expr.hir),
                }
            }
            Statement::Let {
                name,
                initializer,
                variable_type,
                label,
            } => {
                let let_stat = self.resolve_let_statement(
                    name,
                    initializer.as_ref().map(|expr| expr.as_ref_spanned()),
                    variable_type.as_ref().map(|expr| expr.as_ref_spanned()),
                    label.as_ref().map(|s| s.as_str()),
                )?;
                Lowered {
                    type_symbol: let_stat.type_symbol,
                    hir: HIRStatement::Let(let_stat.hir),
                }
            }
            Statement::Item(item) => {
                self.resolve_item(item.as_ref_spanned())?;
                return Ok(None);
            }
            Statement::Semicolon => return Ok(None),
        };

        Ok(Some(hir))
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
        let initializer = if let Some(init_expr) = initializer {
            Some(self.resolve_expression(init_expr)?)
        } else {
            None
        };

        // パターン
        let pattern = name.as_ref_spanned();
        let type_kind = variable_type.as_ref().or(initializer.as_ref());
        self.resolve_pattern(&pattern, type_kind)?;

        let (init_ty, init_hir) = initializer
            .map(|l| (Some(l.type_symbol), Some(l.hir)))
            .unwrap_or((None, None));

        // 型チェック
        let scope = self.get_scope()?;
        let type_symbol =
            self.type_checker
                .check_let_statenent(scope, &name.node, init_ty, variable_type)?;
        let hir = self
            .desuger
            .lower_let_statement(pattern.get_node(), init_hir)?;

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
    ) -> ResolveResult<TypeRequirement> {
        let span = pattern.span;
        let pattern = &pattern.node;

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

                self.name_resolver.add_variable(ident, ty.clone())?;
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
                self.resolve_expression(&body.as_ref_spanned())?;
                self.resolve_pattern(&pattern.as_ref_spanned(), None)?; // TODO
            }
        }

        Ok(TypeSymbol::default())
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
