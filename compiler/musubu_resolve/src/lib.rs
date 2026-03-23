use musubu_ast::*;
use musubu_primitive::*;
use musubu_span::*;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Debug)]
struct PathMap {
    paths: HashMap<String, PathMap>,
    scope: Rc<ResolvedSymbol>,
}

#[derive(Debug)]
struct ResolvedPath {
    paths: Vec<Rc<PathMap>>,
}

#[derive(Debug, Clone)]
pub enum ResolvedSymbol {
    Type,
    Variable,
    Function,
    Variant,
    Field,
}

#[derive(Debug)]
struct Scope {
    import_paths: Vec<PathMap>,
    symbols: HashMap<String, ResolvedSymbol>,
    parent_scope: Option<u64>,
}

impl Scope {
    pub fn new(parent: Option<u64>) -> Self {
        Self {
            import_paths: vec![],
            symbols: HashMap::new(),
            parent_scope: parent,
        }
    }
}

type ScopeId = u64;

#[derive(Debug)]
pub(crate) struct ScopeResolver {
    current_scope: ScopeId,
    scope_map: HashMap<ScopeId, Scope>,
}

impl ScopeResolver {
    pub fn new() -> Self {
        const ROOT_SCOPE: ScopeId = 0;
        let mut resolver = Self {
            current_scope: ROOT_SCOPE,
            scope_map: HashMap::new(),
        };
        resolver.scope_map.insert(ROOT_SCOPE, Scope::new(None));

        // TODO type_checkと統合
        resolver.insert("vec4".to_string(), ResolvedSymbol::Type);
        resolver.insert("matrix".to_string(), ResolvedSymbol::Type);

        resolver
    }

    pub fn enter(&mut self) {
        self.current_scope += 1;
        self.scope_map
            .insert(self.current_scope, Scope::new(Some(self.current_scope - 1)));
    }

    pub fn pop(&mut self) -> ResolveResult<()> {
        let key = self.current_scope;
        let scope = self.scope_map.get(&key).ok_or(ResolveError::InvalidScope)?;
        self.current_scope = scope.parent_scope.ok_or(ResolveError::InvalidScope)?;
        Ok(())
    }

    pub fn insert_path(&mut self, path: &Spanned<Path>) {
        let key = self.current_scope;
        let Some(scope) = self.scope_map.get_mut(&key) else {
            return;
        };

        let mut current = PathMap {
            paths: HashMap::new(),
            scope: Rc::new(ResolvedSymbol::Type),
        };

        for seg in &path.node.segments {
            current.paths.insert(
                seg.node.ident.clone(),
                PathMap {
                    paths: HashMap::new(),
                    scope: Rc::new(ResolvedSymbol::Type),
                },
            );
        }

        scope.import_paths.push(current);
    }

    pub fn insert(&mut self, name: String, symbol: ResolvedSymbol) -> ResolveResult<()> {
        let key = self.current_scope;
        let scope = self
            .scope_map
            .get_mut(&key)
            .ok_or(ResolveError::InvalidScope)?;

        if scope.symbols.contains_key(&name) {
            return Err(ResolveError::DuplicateDefinition { name });
        }

        scope.symbols.insert(name, symbol);

        Ok(())
    }

    pub fn find(&self, scope_id: ScopeId, name: &str) -> Option<&ResolvedSymbol> {
        let mut key = scope_id;
        loop {
            let scope = self.scope_map.get(&key)?;
            if let Some(symbol) = scope.symbols.get(name) {
                return Some(symbol);
            }
            key = scope.parent_scope?;
        }
    }

    pub fn find_current_scope(&self, name: &str) -> Option<&ResolvedSymbol> {
        let scope_id = self.current_scope;
        self.find(scope_id, name)
    }
}

#[derive(Debug)]
pub struct NameResolver {
    ast_to_scope: HashMap<u64, ScopeId>,
    scope: ScopeResolver,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    UndefinedVariable { name: String },
    DuplicateDefinition { name: String },
    InvalidScope,
}

type ResolveResult<T> = Result<T, ResolveError>;

// 解決部分
impl NameResolver {
    pub fn resolve(&mut self, node: &ASTNode) -> ResolveResult<()> {
        self.make_map(node)
    }
}

// neme map作成部分
impl NameResolver {
    pub fn new() -> Self {
        Self {
            ast_to_scope: HashMap::new(),
            scope: ScopeResolver::new(),
        }
    }

    pub fn find<T>(&self, node: &Spanned<T>, name: &str) -> Option<&ResolvedSymbol>
    where
        T: Hash,
    {
        let hash_value = self.get_hash(node);
        let scope_id = self.ast_to_scope.get(&hash_value)?;
        self.scope.find(*scope_id, name)
    }

    fn make_map(&mut self, node: &ASTNode) -> ResolveResult<()> {
        match node {
            ASTNode::Item {
                visibility: _,
                item,
            } => self.make_map_from_item(item.as_ref_spanned())?,
            ASTNode::Expression(expr) => self.make_map_from_expression(expr.as_ref_spanned())?,
            ASTNode::Path(path) => self.make_map_from_path(path.as_ref_spanned())?,
            ASTNode::Type(ty) => self.make_map_from_type(ty.as_ref_spanned())?,
            ASTNode::TypeAlias(alias) => self.make_map_from_type_alias(alias.as_ref_spanned())?,
            ASTNode::Statement(statement) => {
                self.make_map_from_statement(statement.as_ref_spanned())?
            }
            ASTNode::Loop(loop_expr) => self.make_map_from_loop(loop_expr.as_ref_spanned())?,
            ASTNode::Pattern(pattern) => self.make_map_from_pattern(pattern.as_ref_spanned())?,
            _ => unimplemented!(),
        };

        Ok(())
    }

    fn make_map_from_expression(&mut self, expr: Spanned<&Expression>) -> Result<(), ResolveError> {
        self.linking_ast(&expr);

        match &expr.node {
            Expression::Literal(literal) => self.make_map_from_literal(literal.as_ref_spanned())?,
            Expression::Path(path) => self.make_map_from_path(path.as_ref_spanned())?,
            Expression::Binary { left, right, .. }
            | Expression::Assign { left, right, .. }
            | Expression::Comparison { left, right, .. }
            | Expression::Logical { left, right, .. } => {
                self.make_map_from_expression(left.as_ref_spanned())?;
                self.make_map_from_expression(right.as_ref_spanned())?;
            }
            Expression::Call {
                function,
                arguments,
            } => {
                self.make_map_from_expression(function.as_ref_spanned())?;
                for argument in arguments {
                    self.make_map_from_expression(argument.as_ref_spanned())?;
                }
            }
            Expression::Block(statements) => {
                self.enter_scope();
                for statement in statements {
                    self.make_map_from_statement(statement.as_ref_spanned())?;
                }
                self.exit_scope()?;
            }
            Expression::If {
                condition,
                then_body,
                else_body,
            } => {
                self.make_map_from_expression(condition.as_ref_spanned())?;
                self.make_map_from_expression(then_body.as_ref_spanned())?;
                if let Some(expr) = else_body {
                    self.make_map_from_expression(expr.as_ref_spanned())?;
                }
            }
            Expression::Loop(loop_expr) => self.make_map_from_loop(loop_expr.as_ref_spanned())?,
            Expression::Return(expr_opt) => {
                if let Some(expr) = expr_opt {
                    self.make_map_from_expression(expr.as_ref_spanned())?;
                }
            }
            Expression::Array { elements } => match elements {
                ArrayElements::List(list) => {
                    for expr in list {
                        self.make_map_from_expression(expr.as_ref_spanned())?;
                    }
                }
                ArrayElements::Repeat { value, count } => {
                    self.make_map_from_expression(value.as_ref_spanned())?;
                    self.make_map_from_expression(count.as_ref_spanned())?;
                }
            },
            Expression::FieldAccess { parent, .. } => {
                self.make_map_from_expression(parent.as_ref_spanned())?;
            }
            Expression::MethodCall(method) => {
                for param in &method.params {
                    self.make_map_from_expression(param.as_ref_spanned())?;
                }
            }
            Expression::Index { parent, index } => {
                self.make_map_from_expression(parent.as_ref_spanned())?;
                self.make_map_from_expression(index.as_ref_spanned())?;
            }
            Expression::Continue { .. } => {}
            Expression::Break { expression, .. } => {
                if let Some(expr) = expression {
                    self.make_map_from_expression(expr.as_ref_spanned())?;
                }
            }

            _ => (),
        };

        Ok(())
    }

    fn make_map_from_path(&mut self, path: Spanned<&Path>) -> ResolveResult<()> {
        self.linking_ast(&path);

        let last_segment = path
            .node
            .segments
            .last()
            .ok_or(ResolveError::UndefinedVariable {
                name: String::new(),
            })?
            .node
            .clone();
        let name = &last_segment.ident;

        if self.scope.find_current_scope(&name).is_none() {
            return Err(ResolveError::UndefinedVariable {
                name: name.to_string(),
            });
        }
        //self.insert(name.clone(), ResolvedSymbol::Variable)?;

        Ok(())
    }

    fn make_map_from_literal(&mut self, literal: Spanned<&Literal>) -> ResolveResult<()> {
        self.linking_ast(&literal);

        let type_kind = match &literal.node {
            Literal::Float { value_type, .. }
            | Literal::Integer { value_type, .. }
            | Literal::Char { value_type, .. }
            | Literal::UnicodeChar { value_type, .. }
            | Literal::String { value_type, .. } => value_type,
            Literal::Bool(_) => &TypeKind::Primitive(PrimitiveType::Unit),
        };

        self.make_map_from_type(Spanned {
            node: type_kind,
            span: literal.span,
        })?;

        Ok(())
    }

    fn make_map_from_statement(&mut self, statement: Spanned<&Statement>) -> ResolveResult<()> {
        self.linking_ast(&statement);

        match &statement.node {
            Statement::Expression(expr) => self.make_map_from_expression(expr.as_ref_spanned())?,
            Statement::Let {
                name, initializer, ..
            } => {
                if let Some(init_expr) = initializer {
                    self.make_map_from_expression(init_expr.as_ref_spanned())?;
                }
                self.make_map_from_pattern(name.as_ref_spanned())?;
            }
            Statement::Item(item) => self.make_map_from_item(item.as_ref_spanned())?,
            Statement::Semicolon => (),
        }

        Ok(())
    }

    fn make_map_from_pattern(&mut self, pattern: Spanned<&Pattern>) -> ResolveResult<()> {
        self.linking_ast(&pattern);

        match &pattern.node {
            Pattern::Identifier { ident, .. } => {
                self.insert(ident.clone(), ResolvedSymbol::Variable)?;
            }
            Pattern::Multiply(patterns) => {
                for pattern in patterns {
                    self.make_map_from_pattern(pattern.as_ref_spanned())?;
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn make_map_from_item(&mut self, item: Spanned<&Item>) -> ResolveResult<()> {
        self.linking_ast(&item);

        match &item.node {
            Item::Function {
                name, params, body, ..
            } => {
                self.insert(name.clone(), ResolvedSymbol::Function)?;
                let Some(body_expr) = body else {
                    return Ok(());
                };

                self.enter_scope();
                for param in params {
                    let pattern = param.node.clone().pattern.unwrap_or(Spanned {
                        node: Pattern::None,
                        span: param.span,
                    });
                    self.make_map_from_pattern(pattern.as_ref_spanned())?;
                }
                self.make_map_from_expression(body_expr.as_ref_spanned())?;
                self.exit_scope()?;
            }
            Item::Struct { name, fields } => {
                self.insert(name.clone(), ResolvedSymbol::Type)?;

                for field in fields {
                    self.make_map_from_type(field.node.field_type.as_ref_spanned())?;
                }
            }
            Item::Enumeration { name, items } => {
                self.insert(name.clone(), ResolvedSymbol::Type)?;

                for item in items {
                    match &item.node {
                        EnumItem::StructItem { name, fields, .. } => {
                            self.insert(name.clone(), ResolvedSymbol::Variant)?;
                            for field in fields {
                                self.make_map_from_type(field.node.field_type.as_ref_spanned())?;
                            }
                        }
                        EnumItem::TupleItem { name, .. } => {
                            self.insert(name.clone(), ResolvedSymbol::Variant)?;
                        }
                    }
                }
            }
            Item::Union { name, fields } => {
                self.insert(name.clone(), ResolvedSymbol::Type)?;

                for field in fields {
                    self.make_map_from_type(field.node.field_type.as_ref_spanned())?;
                }
            }
        }

        Ok(())
    }

    fn make_map_from_loop(&mut self, loop_expr: Spanned<&LoopExpr>) -> ResolveResult<()> {
        self.linking_ast(&loop_expr);

        match &loop_expr.node {
            LoopExpr::Loop { body } => self.make_map_from_expression(body.as_ref_spanned())?,
            LoopExpr::While { condition, body } => {
                self.make_map_from_expression(condition.as_ref_spanned())?;
                self.make_map_from_expression(body.as_ref_spanned())?;
            }
            LoopExpr::For {
                pattern,
                iterator,
                body,
            } => {
                self.make_map_from_expression(iterator.as_ref_spanned())?;

                self.enter_scope();
                self.make_map_from_pattern(pattern.as_ref_spanned())?;
                self.make_map_from_expression(body.as_ref_spanned())?;
                self.exit_scope()?;
            }
        }
        Ok(())
    }

    fn make_map_from_type(&mut self, ty: Spanned<&TypeKind>) -> ResolveResult<()> {
        self.linking_ast(&ty);

        match &ty.node {
            TypeKind::Primitive(_) => {}
            TypeKind::PathType(path) => {
                self.make_map_from_path(path.as_ref_spanned())?;
            }
            TypeKind::Function {
                params,
                return_type,
            } => {
                for param in params {
                    self.make_map_from_type(param.as_ref_spanned())?;
                }
                self.make_map_from_type(return_type.as_ref_spanned())?;
            }
        }

        Ok(())
    }

    fn make_map_from_type_alias(&mut self, alias: Spanned<&TypeAlias>) -> ResolveResult<()> {
        self.linking_ast(&alias);

        self.insert(alias.node.name.clone(), ResolvedSymbol::Type)?;
        self.make_map_from_type(Spanned {
            node: &alias.node.target,
            span: alias.span,
        })?;

        Ok(())
    }

    fn enter_scope(&mut self) {
        self.scope.enter();
    }

    fn exit_scope(&mut self) -> ResolveResult<()> {
        self.scope.pop()
    }

    fn insert(&mut self, name: String, symbol: ResolvedSymbol) -> ResolveResult<()> {
        self.scope.insert(name, symbol)
    }

    fn linking_ast<T>(&mut self, node: &Spanned<T>)
    where
        T: Hash,
    {
        let hash_value = self.get_hash(node);
        self.ast_to_scope
            .insert(hash_value, self.scope.current_scope);
    }

    fn get_hash<T>(&self, node: &Spanned<T>) -> u64
    where
        T: Hash,
    {
        let mut s = DefaultHasher::new();
        node.hash(&mut s);
        s.finish()
    }
}
