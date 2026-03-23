use musubu_ast::*;
use musubu_hir::*;
use musubu_primitive::*;
use musubu_resolve::*;
use musubu_span::*;
use std::collections::HashMap;

pub struct Desugar<'a> {
    pub next_symbol: usize,
    pub next_function: usize,
    pub variables: HashMap<String, SymbolId>,
    pub functions: HashMap<String, FunctionId>,
    resolver: &'a NameResolver,
}

impl<'a> Desugar<'a> {
    const INITIAL_ID: usize = 0;

    pub fn new(resolver: &'a NameResolver) -> Self {
        Self {
            next_symbol: Self::INITIAL_ID,
            next_function: Self::INITIAL_ID,
            variables: HashMap::new(),
            functions: HashMap::new(),
            resolver,
        }
    }

    pub fn desugar(&mut self, node: &ASTNode) -> HIRModule {
        match &node {
            ASTNode::Item { item, .. } => self.lower_item(item),
            _ => unimplemented!(),
        }
    }

    fn alloc_symbol(&mut self, name: &str) -> SymbolId {
        if let Some(id) = self.variables.get(name) {
            return *id;
        }

        let id = SymbolId(self.next_symbol);
        self.next_symbol += 1;
        self.variables.insert(name.to_string(), id);
        id
    }

    fn resolve_symbol(&self, name: &str) -> SymbolId {
        *self.variables.get(name).expect("undefined variable")
    }

    fn alloc_function(&mut self, name: &str) -> FunctionId {
        if let Some(id) = self.functions.get(name) {
            return *id;
        }

        let id = FunctionId {
            id: FunctionType::UserDefined(self.next_function),
        };
        self.next_function += 1;
        self.functions.insert(name.to_string(), id);
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

    fn lower_item(&mut self, item: &Spanned<Item>) -> HIRModule {
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
        name: &str,
        params: &[Spanned<FunctionParam>],
        body: Option<&SpannedBox<Expression>>,
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
                statements: vec![],
                result: None,
            });

        HIRFunction {
            id: func_id,
            params: hir_params,
            return_type: DEFAULT_TYPE,
            body,
        }
    }

    fn lower_block_expr(&mut self, expr: &SpannedBox<Expression>) -> HIRBlock {
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
                statements: vec![],
                result: Some(Box::new(self.lower_expr(expr))),
            },
        }
    }

    fn lower_statement(&mut self, statement: &Spanned<Statement>) -> Option<HIRStatement> {
        const DEFAULT_TYPE: TypeId = TypeId(0);

        match statement.get_node() {
            Statement::Let {
                name, initializer, ..
            } => {
                let Pattern::Identifier { ident, .. } = &name.node else {
                    return None;
                };
                let sym = self.alloc_symbol(&ident);
                let init = initializer.as_ref().map(|e| self.lower_expr(&e));

                Some(HIRStatement::Let {
                    symbol: sym,
                    ty: DEFAULT_TYPE,
                    init,
                })
            }

            Statement::Expression(expr) => Some(HIRStatement::Expr(self.lower_expr(&expr))),

            _ => None,
        }
    }

    fn lower_expr(&mut self, expr: &SpannedBox<Expression>) -> HIRExpression {
        match expr.get_node() {
            Expression::Literal(l) => self.lower_literal(l),
            Expression::Path(path) => {
                let name = path.node.last_ident();
                HIRExpression::Variable(self.resolve_symbol(name))
            }
            Expression::Binary {
                operator,
                left,
                right,
            } => HIRExpression::BinOp {
                op: operator.clone(),
                lhs: Box::new(self.lower_expr(&left)),
                rhs: Box::new(self.lower_expr(&right)),
            },
            // 脱糖
            // +=, -= など
            Expression::Assign {
                operator,
                left,
                right,
            } => self.lower_assign(operator, &left, &right),
            Expression::Call {
                function,
                arguments,
            } => {
                let name = match &*function.node {
                    Expression::Path(path) => path.node.last_ident(),
                    _ => panic!("unsupported call"),
                };

                HIRExpression::Call {
                    function: self.resolve_function(&name),
                    args: arguments.into_iter().map(|a| self.lower_expr(a)).collect(),
                }
            }

            Expression::If {
                condition,
                then_body,
                else_body,
            } => HIRExpression::If {
                cond: Box::new(self.lower_expr(condition)),
                then_block: self.lower_block_expr(then_body),
                else_block: else_body.as_ref().map(|e| self.lower_block_expr(&e)),
            },

            // while -> loop + if break
            Expression::Loop(loop_expr) => self.lower_loop(loop_expr),

            Expression::Return(expr) => {
                HIRExpression::Return(expr.as_ref().map(|e| Box::new(self.lower_expr(&e))))
            }

            Expression::Break { expression, .. } => {
                HIRExpression::Break(expression.as_ref().map(|e| Box::new(self.lower_expr(&e))))
            }

            _ => unimplemented!(),
        }
    }

    fn lower_assign(
        &mut self,
        operator: &AssignOperator,
        left: &SpannedBox<Expression>,
        right: &SpannedBox<Expression>,
    ) -> HIRExpression {
        let target_name = match &*left.node {
            Expression::Path(path) => path.node.last_ident(),
            _ => panic!("unsupported assign target"),
        };

        let sym = self.resolve_symbol(&target_name);
        let rhs = self.lower_expr(right);

        match operator {
            AssignOperator::Assign => {
                return HIRExpression::Store {
                    target: sym,
                    value: Box::new(rhs),
                };
            }
            _ => {
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
                    AssignOperator::Assign => unreachable!(),
                };

                let lhs_expr = HIRExpression::Variable(sym);
                HIRExpression::Store {
                    target: sym,
                    value: Box::new(HIRExpression::BinOp {
                        op: operator,
                        lhs: Box::new(lhs_expr),
                        rhs: Box::new(rhs),
                    }),
                }
            }
        }
    }

    fn lower_loop(&mut self, loop_expr: &Spanned<LoopExpr>) -> HIRExpression {
        match &loop_expr.node {
            LoopExpr::Loop { body } => HIRExpression::Loop {
                body: self.lower_block_expr(body),
            },

            LoopExpr::While { condition, body } => {
                // while -> loop { if !cond { break } body }
                let cond_expr = self.lower_expr(condition);
                let break_statement = HIRStatement::Expr(HIRExpression::Break(None));

                let if_expr = HIRExpression::If {
                    cond: Box::new(cond_expr),
                    then_block: self.lower_block_expr(body),
                    else_block: Some(HIRBlock {
                        statements: vec![break_statement],
                        result: None,
                    }),
                };

                HIRExpression::Loop {
                    body: HIRBlock {
                        statements: vec![HIRStatement::Expr(if_expr)],
                        result: None,
                    },
                }
            }
            _ => unimplemented!(),
        }
    }

    fn lower_literal(&mut self, literal: &Spanned<Literal>) -> HIRExpression {
        let value = match &literal.node {
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

        HIRExpression::Literal(value)
    }
}
