mod collections;
mod control_flow;
mod enumeration;
mod match_coverage;
mod match_expression;
mod matrices;
mod structures;
mod tuples;
mod vectors;

use crate::errors::ResolveError;
use crate::{Lowered, ResolveResult, Resolver};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
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
        let diagnostic_span = item.span;
        (|| {
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
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
    }

    fn resolve_function(
        &mut self,
        name: &'a str,
        params: &'a [Spanned<FunctionParam>],
        body: Option<Spanned<&'a Expression>>,
        return_type: Option<Spanned<&'a TypeKind>>,
    ) -> ResolveResult<()> {
        // 事前importで解決済み
        if !self.collector.remove(name) || self.resolve_mode.is_squential() {
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

            let actual = body.to_type();
            if actual != return_type
                && (!actual.is_unit()
                    || (matches!(
                        return_type,
                        PrimitiveType::NamedStruct { .. }
                            | PrimitiveType::NamedEnum { .. }
                            | PrimitiveType::Tuple { .. }
                            | PrimitiveType::Vector { .. }
                            | PrimitiveType::Matrix { .. }
                    ) && control_flow::can_complete(&body)))
            {
                return Err(
                    musubu_type_check::errors::TypeCheckError::FunctionReturnMismatch {
                        expected: return_type,
                        found: actual,
                    }
                    .into(),
                );
            }

            let hir = s.desugar.lower_function(args, return_type, body)?;

            Ok(Lowered { type_symbol, hir })
        })?;

        self.desugar.set_function_range(
            id,
            body_expr.span.start as usize,
            body_expr.span.end as usize,
        );
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
                return Err(ResolveError::Unsupported {
                    feature: "function parameter patterns",
                });
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
        if self.collector.remove(name) && self.resolve_mode.is_unordered() {
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
        if self.collector.remove(enum_name) && self.resolve_mode.is_unordered() {
            return Ok(());
        }

        self.import_enumeration(enum_name, items)
    }

    fn resolve_expression(
        &mut self,
        expression: &Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let diagnostic_span = expression.span;
        (|| {
            let lowered = match expression.get_node() {
                Expression::Tuple(elements) => self.resolve_tuple(elements)?,
                Expression::Match { value, arms } => {
                    self.resolve_match(value.as_ref_spanned(), arms)?
                }
                Expression::StructLiteral { path, fields } => {
                    self.resolve_struct_literal(path.as_ref_spanned(), fields)?
                }
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
                Expression::Range {
                    start,
                    end,
                    inclusive,
                } => {
                    self.resolve_range(start.as_ref_spanned(), end.as_ref_spanned(), *inclusive)?
                }
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
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
    }

    fn resolve_break(
        &mut self,
        _label: Option<&'a str>,
        expression: Option<Spanned<&'a Expression>>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        if self.loops.is_empty() {
            return Err(ResolveError::IllegalBreak);
        }
        if _label.is_some() {
            return Err(ResolveError::Unsupported {
                feature: "loop labels",
            });
        }

        if expression.is_some()
            && !self
                .loops
                .last()
                .map(|context| context.allow_value)
                .unwrap_or(false)
        {
            return Err(ResolveError::InvalidBreakValue);
        }
        let (expr_hir, expr_type) = if let Some(expr) = expression {
            let (hir, ty) = self.resolve_expression(&expr)?.split();
            (Some(hir), Some(ty))
        } else {
            (None, None)
        };

        let break_type = expr_type.unwrap_or_default().type_kind;
        let context = self.loops.last_mut().ok_or(ResolveError::IllegalBreak)?;
        if let Some(expected) = &context.break_type {
            if expected != &break_type {
                return Err(musubu_type_check::errors::TypeCheckError::TypeMismatch {
                    expected: expected.clone(),
                    found: break_type,
                }
                .into());
            }
        } else {
            context.break_type = Some(break_type);
        }
        let type_symbol = TypeSymbol::default();
        let hir = self.desugar.lower_break(expr_hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_continue(
        &mut self,
        _label: Option<&'a str>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        if self.loops.is_empty() {
            return Err(ResolveError::IllegalContinue);
        }
        if _label.is_some() {
            return Err(ResolveError::Unsupported {
                feature: "loop labels",
            });
        }

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

        if !matches!(
            &lhs.hir,
            HIRExpression::Variable { .. }
                | HIRExpression::Field { .. }
                | HIRExpression::Index { .. }
        ) {
            return Err(ResolveError::from(
                musubu_desugar::errors::DesugarError::UnsupportedAssignTarget,
            )
            .at(left.span));
        }
        if !lhs.type_symbol.is_mutable() {
            let name = match left.node {
                Expression::Path(path) => path.node.to_string(),
                _ => alloc::string::String::from("assignment target"),
            };
            return Err(ResolveError::from(
                musubu_type_check::errors::TypeCheckError::NotMutable { name },
            )
            .at(left.span));
        }

        self.type_checker
            .check_assign_operator(operator, lhs.type_symbol, rhs.type_symbol)?;
        let hir = self
            .desugar
            .lower_assign_operator(operator.clone(), lhs.hir, rhs.hir)?;

        Ok(Lowered {
            type_symbol: TypeSymbol::default(),
            hir,
        })
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
        if let Expression::Path(path) = function.node {
            if path.node.segments.len() > 1 {
                return self.resolve_enum_tuple(path.as_ref_spanned(), arguments);
            }
            let name = path.node.last_ident();
            if let Some(ty @ PrimitiveType::Matrix { .. }) = PrimitiveType::from(name) {
                return self
                    .resolve_matrix_constructor(ty, arguments)
                    .map_err(|e| e.at(function.span));
            }
            if let Some(ty @ PrimitiveType::Vector { .. }) = PrimitiveType::from(name) {
                return self
                    .resolve_vector_constructor(ty, arguments)
                    .map_err(|e| e.at(function.span));
            }
        }
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
        let type_symbol = TypeSymbol::default();
        let hir = self.desugar.lower_return(expr_hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    fn resolve_field_access(
        &mut self,
        expression: &Spanned<&'a Expression>,
        field_name: &'a str,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        self.resolve_struct_field(expression, field_name)
    }

    fn resolve_method_call(
        &mut self,
        method: &'a MethodCall,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        for param in &method.params {
            self.resolve_expression(&param.as_ref_spanned())?;
        }

        return Err(ResolveError::Unsupported {
            feature: "method calls",
        });
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

    fn resolve_index(
        &mut self,
        parent: Spanned<&'a Expression>,
        index: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        self.resolve_numeric_index(parent, index)
    }

    fn resolve_path(
        &mut self,
        path: Spanned<&'a Path>,
    ) -> ResolveResult<Lowered<Option<HIRExpression>>> {
        if path.node.segments.len() > 1 {
            let value = self.resolve_enum_unit(path)?;
            return Ok(Lowered {
                type_symbol: value.type_symbol,
                hir: Some(value.hir),
            });
        }
        let path = path.node;
        let name = path.last_ident();

        if let Some(type_kind) = PrimitiveType::from(name) {
            return Ok(Lowered {
                type_symbol: TypeSymbol::new(type_kind),
                hir: None,
            });
        }

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
        let diagnostic_span = spanned_literal.span;
        (|| {
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
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
    }

    fn resolve_statement(
        &mut self,
        statement: Spanned<&'a Statement>,
    ) -> ResolveResult<Option<Lowered<HIRStatement>>> {
        let diagnostic_span = statement.span;
        (|| match statement.get_node() {
            Statement::Expression(expr) => {
                let (hir, type_symbol) = self.resolve_expression(&expr.as_ref_spanned())?.split();

                // The parser includes the semicolon in the statement span, but not in the expression span.
                let discarded = statement.span.end > expr.span.end;
                Ok(Some(Lowered {
                    type_symbol: if discarded {
                        TypeSymbol::default()
                    } else {
                        type_symbol
                    },
                    hir: if discarded {
                        HIRStatement::Discard(hir)
                    } else {
                        hir.to_statement()
                    },
                }))
            }
            Statement::Let {
                name,
                initializer,
                variable_type,
                label,
            } => {
                let (hir, _) = self
                    .resolve_let_statement(
                        name,
                        initializer.as_ref().map(|expr| expr.as_ref_spanned()),
                        variable_type.as_ref().map(|expr| expr.as_ref_spanned()),
                        label.as_ref().map(|s| s.as_str()),
                    )?
                    .split();

                Ok(hir.map(|hir| Lowered {
                    type_symbol: TypeSymbol::default(),
                    hir,
                }))
            }
            Statement::Item(item) => {
                self.resolve_item(item.as_ref_spanned())?;
                Ok(None)
            }
            Statement::Semicolon => Ok(None),
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
    }

    fn resolve_let_statement(
        &mut self,
        name: &'a Spanned<Pattern>,
        initializer: Option<Spanned<&'a Expression>>,
        variable_type: Option<Spanned<&'a TypeKind>>,
        _label: Option<&'a str>,
    ) -> ResolveResult<Lowered<Option<HIRStatement>>> {
        if matches!(name.node, Pattern::Tuple(_)) {
            return self.resolve_tuple_let(name, initializer, variable_type);
        }
        let initializer_span = initializer
            .as_ref()
            .map(|expression| expression.span)
            .unwrap_or(name.span);

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
        let (id, symbol_type) = self.resolve_pattern(&pattern, type_kind)?;

        // 型チェック
        let scope = self.get_scope()?;
        let type_symbol = self
            .type_checker
            .check_let_statenent(scope, &name.node, initializer_type, variable_type)
            .map_err(|error| ResolveError::from(error).at(initializer_span))?;

        // TODO: 推論中の場合後回しに
        let Some(type_symbol) = type_symbol else {
            return Err(ResolveError::UnresolvedType {
                name: format!("{:?}", pattern.get_node()),
            });
        };

        let hir =
            self.desugar
                .lower_let_statement(id, type_symbol.type_kind.clone(), initializer_hir)?;

        Ok(Lowered { type_symbol, hir })
    }

    // patternは定義でしか現れない
    fn resolve_pattern(
        &mut self,
        pattern: &Spanned<&'a Pattern>,
        type_kind: Option<&TypeSymbol>, // 事前に型が決まっている場合
    ) -> ResolveResult<(usize, TypeRequirement)> {
        let diagnostic_span = pattern.span;
        (|| {
            let span = pattern.span;
            let pattern = &pattern.node;
            let id = self.desugar.alloc_symbol(); // 変数の割り当て

            let ty = match pattern {
                Pattern::Tuple(_) => {
                    return Err(ResolveError::Unsupported {
                        feature: "tuple patterns outside let and match",
                    });
                }
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
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
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

    fn resolve_loop_body(
        &mut self,
        body: Spanned<&'a Expression>,
        allow_value: bool,
    ) -> ResolveResult<(Lowered<HIRExpression>, PrimitiveType)> {
        self.loops.push(crate::LoopContext {
            allow_value,
            break_type: None,
        });
        let result = self.resolve_expression(&body);
        let context = self.loops.pop().ok_or(ResolveError::IllegalBreak)?;
        let lowered = result?;
        let scope = self.get_scope()?;
        self.type_checker
            .check_loop_expr(scope, lowered.type_symbol.clone())
            .map_err(|error| ResolveError::from(error).at(body.span))?;
        Ok((lowered, context.break_type.unwrap_or(PrimitiveType::Unit)))
    }

    fn resolve_loop_expr(
        &mut self,
        body: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (body, result_type) = self.resolve_loop_body(body, true)?;
        Ok(Lowered {
            type_symbol: TypeSymbol::new(result_type.clone()),
            hir: HIRExpression::Loop {
                body: body.hir.to_block(),
                result_type,
            },
        })
    }

    fn resolve_while_expr(
        &mut self,
        condition: Spanned<&'a Expression>,
        body: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let condition_span = condition.span;
        let condition = self.resolve_expression(&condition)?;
        let scope = self.get_scope()?;
        self.type_checker
            .check_while_expr(scope, condition.type_symbol, TypeSymbol::default())
            .map_err(|error| ResolveError::from(error).at(condition_span))?;
        let (body, _) = self.resolve_loop_body(body, false)?;
        let hir = self
            .desugar
            .lower_while(condition.hir, body.hir.to_block())?;
        Ok(Lowered {
            type_symbol: TypeSymbol::default(),
            hir,
        })
    }

    fn resolve_for_expr(
        &mut self,
        pattern: Spanned<&'a Pattern>,
        iterator: Spanned<&'a Expression>,
        body: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let iterable = self.resolve_expression(&iterator)?;
        let element_type = match &iterable.type_symbol.type_kind {
            PrimitiveType::Array { type_kind, .. } | PrimitiveType::Range { type_kind } => {
                type_kind.as_ref().clone()
            }
            found => {
                return Err(ResolveError::from(
                    musubu_type_check::errors::TypeCheckError::NotIterable {
                        found: found.clone(),
                    },
                )
                .at(iterator.span));
            }
        };
        if !matches!(
            pattern.node,
            Pattern::Identifier {
                reference: false,
                ..
            } | Pattern::None
        ) {
            return Err(ResolveError::Unsupported {
                feature: "this for-loop binding pattern",
            }
            .at(pattern.span));
        }
        let hir = self.enter_scope(|resolver| {
            let symbol = if matches!(pattern.node, Pattern::None) {
                resolver.desugar.alloc_symbol()
            } else {
                resolver
                    .resolve_pattern(&pattern, Some(&TypeSymbol::new(element_type.clone())))?
                    .0
            };
            let (body, _) = resolver.resolve_loop_body(body.clone(), false)?;
            let hir = resolver.desugar.lower_for(
                symbol,
                element_type.clone(),
                iterable.hir.clone(),
                body.hir.to_block(),
            )?;
            Ok(Lowered {
                type_symbol: TypeSymbol::default(),
                hir,
            })
        })?;
        Ok(hir)
    }

    fn resolve_type(&mut self, type_kind: Spanned<&'a TypeKind>) -> ResolveResult<TypeSymbol> {
        if let TypeKind::Tuple(elements) = type_kind.node {
            return self.resolve_tuple_type(elements);
        }
        if let TypeKind::PathType(path) = type_kind.node {
            return self
                .import_path(path.as_ref_spanned())
                .map_err(|error| error.at(type_kind.span));
        }
        let diagnostic_span = type_kind.span;
        (|| {
            let type_kind = &type_kind.node;
            let scope = self.get_scope()?;

            let ty = self.type_checker.check_type(scope, type_kind)?;

            match type_kind {
                TypeKind::Primitive(_) | TypeKind::Tuple(_) => {}
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
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
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
