use crate::{SemanticError, SemanticResult, TypeEnv, TypeSymbol};
use musubu_ast::{Expression, Item, Literal, LoopExpr, Path, Pattern, Statement, TypeKind};
use musubu_primitive::*;
use musubu_span::*;

type InferResult = SemanticResult<PrimitiveType>;

// TODO: TypedASTの構築
pub fn infer_expression(env: &mut TypeEnv, expr: &SpannedBox<Expression>) -> InferResult {
    match expr.node.as_ref() {
        Expression::Literal(literal) => infer_literal(env, literal),
        Expression::Path(path) => infer_path(env, &path.as_ref_spanned()),
        Expression::Binary {
            operator,
            left,
            right,
        } => infer_binary(env, operator, &left, &right),
        Expression::Assign { left, right, .. } => infer_assign(env, &left, &right),
        Expression::If {
            condition,
            then_body,
            else_body,
        } => infer_if(env, &condition, &then_body, else_body.as_ref()),
        Expression::Block(statements) => infer_block(env, &statements),
        Expression::Return(expr) => infer_return(env, expr.as_ref()),
        Expression::Call {
            function,
            arguments,
        } => infer_call(env, &function, &arguments),
        Expression::Loop(loop_expr) => infer_loop(env, &loop_expr),

        _ => Err(SemanticError::InvalidOperation {
            op: "expression".into(),
            reason: "unsupported expression".into(),
        }),
    }
}

fn infer_type(env: &mut TypeEnv, type_kind: &Spanned<&TypeKind>) -> InferResult {
    let type_kind = match &type_kind.node {
        TypeKind::Primitive(ty) => ty.clone(),
        TypeKind::Function {
            params,
            return_type,
        } => {
            let return_type = infer_type(env, &return_type.as_ref_spanned())?;
            let mut arguments = vec![];
            for param in params {
                arguments.push(infer_type(env, &param.as_ref_spanned())?);
            }
            PrimitiveType::Function {
                return_type: Box::new(return_type),
                arguments,
            }
        }
        TypeKind::PathType(path) => infer_path(env, &path.as_ref_spanned())?,
    };

    Ok(type_kind)
}

fn infer_literal(env: &mut TypeEnv, literal: &Spanned<Literal>) -> InferResult {
    match &literal.node {
        Literal::Integer { value_type, .. } => infer_type(
            env,
            &Spanned {
                node: value_type,
                span: literal.span,
            },
        ),
        Literal::Float { value_type, .. } => infer_type(
            env,
            &Spanned {
                node: value_type,
                span: literal.span,
            },
        ),
        Literal::Bool(_) => Ok(PrimitiveType::Integer {
            signed: false,
            byte: 1,
        }),
        _ => Ok(PrimitiveType::Unit),
    }
}

fn infer_path(env: &mut TypeEnv, path: &Spanned<&Path>) -> InferResult {
    let name = &path
        .node
        .segments
        .last()
        .ok_or(SemanticError::InvalidPath {
            name: String::new(),
        })?
        .node
        .ident;
    let sym = env.lookup(name)?;

    Ok(sym.type_kind.clone())
}

fn infer_binary(
    env: &mut TypeEnv,
    operator: &BinaryOperator,
    left: &SpannedBox<Expression>,
    right: &SpannedBox<Expression>,
) -> InferResult {
    let left_ty = infer_expression(env, left)?;
    let right_ty = infer_expression(env, right)?;

    if left_ty != right_ty {
        return Err(SemanticError::TypeMismatch {
            expected: left_ty,
            found: right_ty,
        });
    }

    match operator {
        BinaryOperator::Addition
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide => Ok(left_ty),

        _ => Err(SemanticError::InvalidOperation {
            op: format!("{:?}", operator),
            reason: "unsupported binary operator".into(),
        }),
    }
}

fn infer_assign(
    env: &mut TypeEnv,
    left: &SpannedBox<Expression>,
    right: &SpannedBox<Expression>,
) -> InferResult {
    let left_ty = infer_expression(env, left)?;
    let right_ty = infer_expression(env, right)?;

    if left_ty != right_ty {
        return Err(SemanticError::TypeMismatch {
            expected: left_ty,
            found: right_ty,
        });
    }

    Ok(PrimitiveType::Unit)
}

fn infer_if(
    env: &mut TypeEnv,
    condition: &SpannedBox<Expression>,
    then_body: &SpannedBox<Expression>,
    else_body: Option<&SpannedBox<Expression>>,
) -> InferResult {
    let cond_ty = infer_expression(env, condition)?;
    let bool_ty = PrimitiveType::Integer {
        signed: false,
        byte: 1,
    };

    if cond_ty != bool_ty {
        return Err(SemanticError::InvalidConditionType { found: cond_ty });
    }

    let then_ty = infer_expression(env, then_body)?;

    if let Some(else_expr) = else_body {
        let else_ty = infer_expression(env, else_expr)?;

        if then_ty != else_ty {
            return Err(SemanticError::TypeMismatch {
                expected: then_ty,
                found: else_ty,
            });
        }

        Ok(then_ty)
    } else {
        Ok(PrimitiveType::Unit)
    }
}

fn infer_block(env: &mut TypeEnv, statements: &[Spanned<Statement>]) -> InferResult {
    env.enter_scope();

    let mut last_type = PrimitiveType::Unit;
    for statement in statements {
        last_type = infer_statement(env, statement)?;
    }

    env.exit_scope();

    Ok(last_type)
}

fn infer_return(env: &mut TypeEnv, expr: Option<&SpannedBox<Expression>>) -> InferResult {
    let return_type = if let Some(expr) = expr {
        infer_expression(env, expr)?
    } else {
        PrimitiveType::Unit
    };

    let expected = env
        .return_type
        .clone()
        .ok_or(SemanticError::InvalidOperation {
            op: "return".into(),
            reason: "return outside function".into(),
        })?;

    if return_type != expected {
        return Err(SemanticError::FunctionReturnMismatch {
            expected,
            found: return_type,
        });
    }

    Ok(return_type)
}

fn infer_call(
    env: &mut TypeEnv,
    function: &SpannedBox<Expression>,
    params: &[SpannedBox<Expression>],
) -> InferResult {
    let function_type = infer_expression(env, function)?;

    // TODO: 削除
    // デモ用で呼び出し元が行列型の場合は関数として扱う
    if let PrimitiveType::Matrix {
        type_kind,
        rows,
        columns,
    } = &function_type
    {
        if *rows != 4 || *columns != 4 {
            unimplemented!();
        }
        if params.len() != 16 {
            return Err(SemanticError::ArgumentCountMismatch {
                expected: 16,
                found: params.len(),
            });
        }

        return Ok(function_type);
    }

    let PrimitiveType::Function {
        return_type,
        arguments,
    } = function_type
    else {
        return Err(SemanticError::NotCallable {
            found: function_type,
        });
    };

    if params.len() != arguments.len() {
        return Err(SemanticError::ArgumentCountMismatch {
            expected: arguments.len(),
            found: params.len(),
        });
    }

    for (arg_type, param_expr) in arguments.iter().zip(params.iter()) {
        let param_type = infer_expression(env, param_expr)?;
        if arg_type != &param_type {
            return Err(SemanticError::TypeMismatch {
                expected: arg_type.clone(),
                found: param_type,
            });
        }
    }

    Ok(*return_type)
}

fn infer_loop(env: &mut TypeEnv, loop_expr: &Spanned<LoopExpr>) -> InferResult {
    match &loop_expr.node {
        LoopExpr::Loop { body } => {
            infer_expression(env, body)?;
        }
        LoopExpr::While { condition, body } => {
            let cond_ty = infer_expression(env, condition)?;
            let bool_ty = PrimitiveType::Integer {
                signed: false,
                byte: 1,
            };

            if cond_ty != bool_ty {
                return Err(SemanticError::InvalidConditionType { found: cond_ty });
            }

            infer_expression(env, body)?;
        }
        LoopExpr::For {
            pattern,
            iterator,
            body,
        } => {
            let iter_type = infer_expression(env, iterator)?;
            env.enter_scope();

            bind_pattern(env, pattern, iter_type)?;
            infer_expression(env, body)?;

            env.exit_scope();
        }
    };

    Ok(PrimitiveType::Unit)
}

fn infer_statement(env: &mut TypeEnv, statement: &Spanned<Statement>) -> InferResult {
    match &statement.node {
        Statement::Expression(expr) => infer_expression(env, expr),
        Statement::Let {
            name,
            variable_type,
            initializer,
            ..
        } => infer_let_statement(env, name, variable_type.as_ref(), initializer.as_ref()),
        Statement::Item(item) => infer_item(item, env),
        Statement::Semicolon => Ok(PrimitiveType::Unit),
    }
}

fn infer_let_statement(
    env: &mut TypeEnv,
    name: &Spanned<Pattern>,
    variable_type: Option<&Spanned<TypeKind>>,
    initializer: Option<&SpannedBox<Expression>>,
) -> InferResult {
    let init_type = if let Some(expr) = initializer {
        infer_expression(env, expr)?
    } else {
        PrimitiveType::Unit
    };

    let final_type = if let Some(declared_type) = variable_type {
        let declared_type = infer_type(env, &declared_type.as_ref_spanned())?;

        if init_type != declared_type {
            return Err(SemanticError::TypeMismatch {
                expected: declared_type.clone(),
                found: init_type,
            });
        }

        declared_type
    } else {
        init_type
    };

    bind_pattern(env, name, final_type)?;

    Ok(PrimitiveType::Unit)
}

fn bind_pattern(
    env: &mut TypeEnv,
    pattern: &Spanned<Pattern>,
    type_kind: PrimitiveType,
) -> SemanticResult<()> {
    match &pattern.node {
        Pattern::Identifier { ident, mutable, .. } => env.insert(
            ident.clone(),
            TypeSymbol {
                type_kind,
                mutable: *mutable,
            },
        ),

        Pattern::Multiply(patterns) => {
            for pattern in patterns {
                bind_pattern(env, &pattern, type_kind.clone())?;
            }
            Ok(())
        }

        _ => Ok(()),
    }
}

pub fn infer_item(item: &Spanned<Item>, env: &mut TypeEnv) -> InferResult {
    match &item.node {
        Item::Function {
            name,
            params,
            return_type,
            body,
        } => {
            let return_type = return_type.clone().unwrap_or(Spanned {
                node: TypeKind::Primitive(PrimitiveType::Unit),
                span: item.span,
            });
            let return_type = infer_type(env, &return_type.as_ref_spanned())?;
            env.return_type = Some(return_type.clone());

            let arguments = params
                .iter()
                .map(|param| infer_type(env, &param.node.param_type.as_ref_spanned()))
                .collect::<Result<Vec<_>, _>>()?;
            env.insert(
                name.clone(),
                TypeSymbol {
                    type_kind: PrimitiveType::Function {
                        arguments,
                        return_type: Box::new(return_type.clone()),
                    },
                    mutable: false,
                },
            )?;

            if let Some(body_expr) = body {
                env.enter_scope();

                for param in params {
                    let node = &param.node;
                    let param_type = infer_type(env, &node.param_type.as_ref_spanned())?;
                    bind_pattern(env, &node.pattern.as_ref().unwrap(), param_type)?;
                }

                let body_ty = infer_expression(env, &body_expr)?;
                if body_ty != return_type {
                    return Err(SemanticError::FunctionReturnMismatch {
                        expected: return_type,
                        found: body_ty,
                    });
                }

                env.exit_scope();
            }

            Ok(PrimitiveType::Unit)
        }

        _ => Ok(PrimitiveType::Unit),
    }
}
