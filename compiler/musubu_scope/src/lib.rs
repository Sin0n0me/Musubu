// TODO
//#![no_std]

extern crate alloc;

pub mod errors;

use self::errors::ScopeError;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use musubu_hir::SymbolId;
use musubu_primitive::PrimitiveType;

pub type ScopeId = u64;
pub type ScopeResult<T> = Result<T, ScopeError>;
pub const ROOT_SCOPE_ID: ScopeId = 0;

pub trait ScopeControl<E> {
    fn on_enter_scope(&mut self) -> Result<(), E>;

    fn on_exit_scope(&mut self) -> Result<(), E>;
}

pub trait SymbolStore<'a, E> {
    fn add_variable(&mut self, id: SymbolId, name: &'a str, ty: TypeRequirement) -> Result<(), E>;

    fn get_variable_id(&self, name: &'a str) -> Option<&SymbolId>;

    fn add_type(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), E>;

    fn get_type(&self, name: &'a str) -> Option<&TypeSymbol>;

    fn get_type_option(&self, name: &'a str) -> Option<&TypeOption>;

    fn get_symbol(&self, name: &'a str) -> Option<&Symbol>;

    fn resolve_variable_type(&mut self, name: &'a str, ty: PrimitiveType) -> Result<(), E>;

    fn is_variable(&self, name: &'a str) -> bool;

    fn is_type(&self, name: &'a str) -> bool;

    fn contains(&self, name: &'a str) -> bool;
}

#[derive(Debug)]
pub struct Scope<'a> {
    label: Option<&'a str>,
    parent: Option<&'a Scope<'a>>,
    symbol_map: BTreeMap<&'a str, Symbol>,
    import_types: BTreeSet<&'a str>, // 今は考えない
    return_type: TypeSymbol,
}

impl<'a> Scope<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            parent: None,
            symbol_map: BTreeMap::new(),
            import_types: BTreeSet::new(),
            return_type: TypeSymbol::default(),
        }
    }

    pub fn set_return_type(&mut self, retrun_type: TypeSymbol) {
        self.return_type = retrun_type;
    }

    pub fn get_return_type(&self) -> &TypeSymbol {
        &self.return_type
    }

    pub fn add_import_type(&mut self, name: &'a str) -> ScopeResult<()> {
        if self.import_types.insert(name) {
            // TODO: 警告
        };
        Ok(())
    }

    fn add_symbol(&mut self, name: &'a str, symbol: Symbol) -> ScopeResult<()> {
        if let Some(symbol) = self.symbol_map.get(name) {
            match symbol {
                Symbol::Variable { .. } => {
                    return Err(ScopeError::DuplicateVariable {
                        name: name.to_string(),
                    });
                }
                Symbol::Type(_) => {
                    return Err(ScopeError::DuplicateType {
                        name: name.to_string(),
                    });
                }
            }
        }

        self.symbol_map.insert(name, symbol);

        Ok(())
    }

    pub fn contains_inferring_types(&self) -> bool {
        self.symbol_map
            .iter()
            .any(|(_, symbol)| symbol.contains_inferring())
    }

    pub fn get_inferring_names(&self) -> Vec<String> {
        self.symbol_map
            .iter()
            .filter_map(|(name, symbol)| {
                if symbol.contains_inferring() {
                    return Some(name.to_string());
                }
                None
            })
            .collect()
    }
}

impl<'a> SymbolStore<'a, ScopeError> for Scope<'a> {
    fn add_type(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ScopeError> {
        self.add_symbol(name, Symbol::Type(ty))
    }

    fn add_variable(
        &mut self,
        id: SymbolId,
        name: &'a str,
        ty: TypeRequirement,
    ) -> Result<(), ScopeError> {
        self.add_symbol(name, Symbol::Variable { id, ty })
    }

    fn get_variable_id(&self, name: &'a str) -> Option<&SymbolId> {
        let Symbol::Variable { id, ty: _ } = self.get_symbol(name)? else {
            return None;
        };

        Some(id)
    }

    // 推論中の型の確定は一度のみ
    fn resolve_variable_type(
        &mut self,
        name: &'a str,
        primitive_type: PrimitiveType,
    ) -> Result<(), ScopeError> {
        let Some(symbol) = self.symbol_map.get(name) else {
            return Err(ScopeError::UnresolveVariable {
                name: name.to_string(),
            });
        };

        // 参照や可変かの情報を取得
        // そもそも変数かチェック
        let Symbol::Variable { id: _, ty } = symbol else {
            return Err(ScopeError::NotVariable {
                name: name.to_string(),
                found: "Type".to_string(), //
            });
        };
        // 推論中かどうかチェック
        // 事前に確定している場合はエラー
        let TypeRequirement::Inferring(_) = ty else {
            return Err(ScopeError::TypeConflict {
                name: name.to_string(),
                expected: "Inferring".to_string(),
                found: "".to_string(),
            });
        };

        let Some(Symbol::Variable {
            id,
            ty: TypeRequirement::Inferring(option),
        }) = self.symbol_map.remove(name)
        else {
            unreachable!()
        };

        self.symbol_map.insert(
            name,
            Symbol::Variable {
                id,
                ty: TypeRequirement::Expect(TypeSymbol {
                    type_kind: primitive_type,
                    option,
                }),
            },
        );

        Ok(())
    }

    fn get_symbol(&self, name: &'a str) -> Option<&Symbol> {
        self.symbol_map.get(name)
    }

    fn get_type(&self, name: &'a str) -> Option<&TypeSymbol> {
        self.symbol_map.get(name).and_then(|s| match s {
            Symbol::Type(TypeRequirement::Expect(ty)) => Some(ty),
            Symbol::Variable {
                id: _,
                ty: TypeRequirement::Expect(ty),
            } => Some(ty),
            _ => None,
        })
    }

    fn get_type_option(&self, name: &'a str) -> Option<&TypeOption> {
        self.symbol_map.get(name).map(|s| s.get_type_option())
    }

    fn is_type(&self, name: &'a str) -> bool {
        self.symbol_map
            .get(name)
            .is_some_and(|s| matches!(s, Symbol::Type(_)))
    }

    fn is_variable(&self, name: &'a str) -> bool {
        self.symbol_map
            .get(name)
            .is_some_and(|s| matches!(s, Symbol::Variable { .. }))
    }

    fn contains(&self, name: &'a str) -> bool {
        self.symbol_map.contains_key(name)
    }
}

#[derive(Debug, Clone)]
pub enum Symbol {
    Variable { id: SymbolId, ty: TypeRequirement },
    Type(TypeRequirement),
}

impl Symbol {
    pub fn contains_inferring(&self) -> bool {
        match self {
            Self::Variable { id: _, ty } => ty.is_inferring(),
            Self::Type(t) => t.is_inferring(),
        }
    }

    pub fn get_type_option(&self) -> &TypeOption {
        match self {
            Self::Type(t) => t.get_type_option(),
            Self::Variable { id: _, ty } => ty.get_type_option(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeSymbol {
    pub type_kind: PrimitiveType,
    pub option: TypeOption,
}

impl TypeSymbol {
    pub fn new(type_kind: PrimitiveType) -> Self {
        Self {
            type_kind,
            option: TypeOption::default(),
        }
    }

    pub fn is_mutable(&self) -> bool {
        self.option.mutable
    }

    pub fn is_reference(&self) -> bool {
        self.option.reference
    }

    pub fn is_same_type(&self, ty: &Self) -> bool {
        self.type_kind == ty.type_kind && self.option.reference == ty.option.reference
    }

    pub fn get_type_option(&self) -> &TypeOption {
        &self.option
    }
}

impl Default for TypeSymbol {
    fn default() -> Self {
        Self {
            type_kind: PrimitiveType::Unit,
            option: TypeOption::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeOption {
    pub mutable: bool,
    pub reference: bool,
}

impl Default for TypeOption {
    fn default() -> Self {
        Self {
            mutable: false,
            reference: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TypeRequirement {
    Expect(TypeSymbol),
    Inferring(TypeOption),
}

impl TypeRequirement {
    pub fn is_inferring(&self) -> bool {
        matches!(self, Self::Inferring(_))
    }

    pub fn get_type_option(&self) -> &TypeOption {
        match self {
            Self::Expect(ty) => ty.get_type_option(),
            Self::Inferring(op) => op,
        }
    }

    pub fn get_type(&self) -> Option<&PrimitiveType> {
        match self {
            Self::Expect(ty) => Some(&ty.type_kind),
            Self::Inferring(_) => None,
        }
    }
}
