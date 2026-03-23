pub mod errors;
mod infer;

use errors::SemanticError;
use infer::infer_expression::{infer_expression, infer_item};
use musubu_ast::ASTNode;
use musubu_primitive::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TypeSymbol {
    pub type_kind: PrimitiveType,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct Scope {
    symbols: HashMap<String, TypeSymbol>,
}

#[derive(Debug)]
pub struct TypeEnv {
    scopes: Vec<Scope>,
    return_type: Option<PrimitiveType>,
}

pub(crate) type SemanticResult<T> = Result<T, SemanticError>;

// Resolvedと似たようなことしているのでResolveをmutで受け取って書き換えるような形にする
impl TypeEnv {
    pub fn new() -> Self {
        let mut env = Self {
            return_type: None,
            scopes: vec![Scope {
                symbols: HashMap::new(),
            }],
        };

        env.insert(
            "matrix".to_string(),
            TypeSymbol {
                type_kind: PrimitiveType::Matrix {
                    type_kind: Box::new(PrimitiveType::default_float()),
                    rows: 4,
                    columns: 4,
                },
                mutable: true,
            },
        );
        env.insert(
            "vec3".to_string(),
            TypeSymbol {
                type_kind: PrimitiveType::Vector {
                    type_kind: Box::new(PrimitiveType::default_float()),
                    dimension: 3,
                },
                mutable: true,
            },
        );
        env.insert(
            "vec4".to_string(),
            TypeSymbol {
                type_kind: PrimitiveType::Vector {
                    type_kind: Box::new(PrimitiveType::default_float()),
                    dimension: 4,
                },
                mutable: true,
            },
        );

        env
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(Scope {
            symbols: HashMap::new(),
        });
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn insert(&mut self, name: String, symbol: TypeSymbol) -> SemanticResult<()> {
        let current = self
            .scopes
            .last_mut()
            .ok_or(SemanticError::InvalidScope {})?;

        if current.symbols.contains_key(&name) {
            return Err(SemanticError::DuplicateDefinition { name });
        }

        current.symbols.insert(name, symbol);
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> SemanticResult<&TypeSymbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.symbols.get(name) {
                return Ok(sym);
            }
        }

        Err(SemanticError::UndefinedVariable {
            name: name.to_string(),
        })
    }
}

pub fn type_check(ast: &ASTNode) -> Result<(), SemanticError> {
    let mut env = TypeEnv::new();
    match &ast {
        ASTNode::Expression(expr) => infer_expression(&mut env, expr)?,
        ASTNode::Item { item, .. } => infer_item(item, &mut env)?,
        _ => unreachable!(),
    };

    Ok(())
}
