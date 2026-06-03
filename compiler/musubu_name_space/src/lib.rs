// TODO
// #![no_std]

extern crate alloc;

pub mod errors;

use crate::errors::NameSpaceError;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::hash::BuildHasherDefault;
use musubu_primitive::{PrimitiveType, ToPrimitiveType};
use musubu_scope::TypeSymbol;
use twox_hash::XxHash64;

pub type NameSpaceResult<T> = Result<T, NameSpaceError>;
type Hasher = BuildHasherDefault<XxHash64>;
type IndexMap<K, V> = indexmap::IndexMap<K, V, Hasher>;

pub trait ItemStore<'a, E> {
    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), E>;
    fn add_struct(&mut self, struct_item: StructItem<'a>) -> Result<(), E>;
    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), E>;
}

pub trait ItemStoreReader<'a> {
    fn get_function(&self, name: &'a str) -> Option<&FunctionItem<'a>>;
    fn get_struct(&self, name: &'a str) -> Option<&StructItem<'a>>;
    fn get_enumeration(&self, name: &'a str) -> Option<&EnumItem<'a>>;
}

#[derive(Debug)]
struct SymbolTable<'a> {
    item_symbol: ItemSymbol<'a>,
    type_symbol: TypeSymbol,
}

impl<'a> SymbolTable<'a> {
    fn new(item: ItemSymbol<'a>) -> Self {
        let type_symbol = TypeSymbol::new(item.to_type());
        Self {
            item_symbol: item,
            type_symbol,
        }
    }

    fn get_type(&self) -> &TypeSymbol {
        &self.type_symbol
    }

    fn get_item(&self) -> &ItemSymbol<'a> {
        &self.item_symbol
    }
}

impl<'a> ItemStoreReader<'a> for SymbolTable<'a> {
    fn get_function(&self, _: &'a str) -> Option<&FunctionItem<'a>> {
        self.item_symbol.get_function()
    }

    fn get_struct(&self, _: &'a str) -> Option<&StructItem<'a>> {
        self.item_symbol.get_struct()
    }

    fn get_enumeration(&self, _: &'a str) -> Option<&EnumItem<'a>> {
        self.item_symbol.get_enumeration()
    }
}

#[derive(Debug)]
pub struct Module<'a> {
    module_name: &'a str,
    children: BTreeMap<&'a str, Box<Module<'a>>>,
    items: BTreeMap<&'a str, SymbolTable<'a>>,
}

impl<'a> ItemStore<'a, NameSpaceError> for Module<'a> {
    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), NameSpaceError> {
        let function_name = function_item.name;
        let pre = self.items.insert(
            function_name,
            SymbolTable::new(ItemSymbol::Function(function_item)),
        );
        if pre.is_some() {
            return Err(NameSpaceError::DuplicateFunction {
                name: function_name.to_string(),
            });
        }

        Ok(())
    }

    fn add_struct(&mut self, struct_item: StructItem<'a>) -> Result<(), NameSpaceError> {
        let struct_name = struct_item.name;
        let pre = self.items.insert(
            struct_name,
            SymbolTable::new(ItemSymbol::Struct(struct_item)),
        );
        if pre.is_some() {
            return Err(NameSpaceError::DuplicateStruct {
                name: struct_name.to_string(),
            });
        }

        Ok(())
    }

    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), NameSpaceError> {
        let enum_name = enum_item.name;
        let pre = self.items.insert(
            enum_name,
            SymbolTable::new(ItemSymbol::Enumeration(enum_item)),
        );
        if pre.is_some() {
            return Err(NameSpaceError::DuplicateEnumeration {
                name: enum_name.to_string(),
            });
        }

        Ok(())
    }
}

impl<'a> ItemStoreReader<'a> for Module<'a> {
    fn get_function(&self, name: &'a str) -> Option<&FunctionItem<'a>> {
        self.items.get(name)?.get_function("")
    }

    fn get_struct(&self, name: &'a str) -> Option<&StructItem<'a>> {
        self.items.get(name)?.get_struct("")
    }

    fn get_enumeration(&self, name: &'a str) -> Option<&EnumItem<'a>> {
        self.items.get(name)?.get_enumeration("")
    }
}

impl<'a> Module<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            module_name: name,
            children: BTreeMap::new(),
            items: BTreeMap::new(),
        }
    }

    pub fn add_module(&mut self, name: &'a str) {
        self.children
            .entry(name)
            .or_insert(Box::new(Module::new(name)));
    }

    pub fn add_modules(&mut self, child_path: &[&'a str]) {
        let mut iter = child_path.iter();
        let Some(first) = iter.next() else {
            return;
        };
        let mut module = self
            .children
            .entry(first)
            .or_insert(Box::new(Module::new(first)));
        for name in iter {
            module = module
                .children
                .entry(name)
                .or_insert(Box::new(Module::new(name)));
        }
    }

    pub fn get_type(&self, name: &'a str) -> Option<&TypeSymbol> {
        Some(self.items.get(name)?.get_type())
    }

    pub fn get_item(&self, name: &'a str) -> Option<&ItemSymbol<'a>> {
        Some(self.items.get(name)?.get_item())
    }

    pub fn get_mut_child_module(&mut self, name: &'a str) -> Option<&mut Self> {
        self.children.get_mut(name).map(|c| c.as_mut())
    }

    pub fn get_mut_module_from_path(&mut self, child_path: &[&'a str]) -> Option<&mut Self> {
        let mut iter = child_path.iter();
        let Some(first) = iter.next() else {
            return Some(self);
        };
        let mut module = self.children.get_mut(first)?;
        loop {
            let Some(name) = iter.next() else {
                break Some(module.as_mut());
            };
            if !module.children.contains_key(name) {
                break Some(module.as_mut());
            }
            module = module.children.get_mut(name).unwrap();
        }
    }

    pub fn get_module_from_path(&self, child_path: &[&'a str]) -> Option<&Self> {
        let mut iter = child_path.iter();
        let Some(first) = iter.next() else {
            return Some(self);
        };
        let mut module = self.children.get(first)?;
        loop {
            let Some(name) = iter.next() else {
                break Some(module.as_ref());
            };
            if !module.children.contains_key(name) {
                break Some(module.as_ref());
            }
            module = module.children.get(name).unwrap();
        }
    }

    pub fn get_module(&self, name: &'a str) -> Option<&Module<'a>> {
        self.children.get(name).map(|child| child.as_ref())
    }

    pub fn get_mut_module(&mut self, name: &'a str) -> Option<&mut Module<'a>> {
        self.children.get_mut(name).map(|child| child.as_mut())
    }
}

#[derive(Debug)]
pub enum ItemSymbol<'a> {
    Function(FunctionItem<'a>),
    Struct(StructItem<'a>),
    Enumeration(EnumItem<'a>),
}

impl<'a> ItemSymbol<'a> {
    pub fn get_function(&self) -> Option<&FunctionItem<'a>> {
        let Self::Function(item) = self else {
            return None;
        };
        Some(item)
    }

    pub fn get_struct(&self) -> Option<&StructItem<'a>> {
        let Self::Struct(item) = self else {
            return None;
        };
        Some(item)
    }

    pub fn get_enumeration(&self) -> Option<&EnumItem<'a>> {
        let Self::Enumeration(item) = self else {
            return None;
        };
        Some(item)
    }
}

impl<'a> ToPrimitiveType for ItemSymbol<'a> {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Enumeration(enum_item) => enum_item.to_type(),
            Self::Struct(struct_item) => struct_item.to_type(),
            Self::Function(function_item) => function_item.to_type(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionItem<'a> {
    pub id: usize,
    pub name: &'a str,
    pub return_type: TypeSymbol,
    pub arguments: Vec<TypeSymbol>,
}

impl<'a> FunctionItem<'a> {
    pub fn new(id: usize, name: &'a str, return_type: TypeSymbol) -> Self {
        Self {
            id,
            name,
            return_type,
            arguments: Vec::new(),
        }
    }

    pub fn add_argument(&mut self, type_symbol: TypeSymbol) -> NameSpaceResult<()> {
        self.arguments.push(type_symbol);
        Ok(())
    }

    pub fn get_return_type(&self) -> &TypeSymbol {
        &self.return_type
    }

    pub fn get_arguments(&self) -> &[TypeSymbol] {
        &self.arguments
    }
}

impl<'a> ToPrimitiveType for FunctionItem<'a> {
    fn to_type(&self) -> musubu_primitive::PrimitiveType {
        PrimitiveType::Function {
            return_type: Box::new(self.return_type.type_kind.clone()),
            arguments: self
                .arguments
                .iter()
                .map(|arg| arg.type_kind.clone())
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct StructItem<'a> {
    pub name: &'a str,
    pub fields: IndexMap<&'a str, TypeSymbol>,
}

impl<'a> StructItem<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            fields: make_index_map(),
        }
    }

    pub fn add_field(&mut self, name: &'a str, type_symbol: TypeSymbol) -> NameSpaceResult<()> {
        if self.fields.insert(name, type_symbol).is_some() {
            return Err(NameSpaceError::DuplicateStructField {
                name: name.to_string(),
            });
        }

        Ok(())
    }
}

impl<'a> ToPrimitiveType for StructItem<'a> {
    fn to_type(&self) -> PrimitiveType {
        PrimitiveType::Struct {
            elements: self
                .fields
                .iter()
                .map(|(_, field_symbol)| field_symbol.type_kind.clone())
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct EnumItem<'a> {
    pub name: &'a str,
    pub variants: IndexMap<&'a str, IndexMap<&'a str, TypeSymbol>>,
}

impl<'a> EnumItem<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            variants: make_index_map(),
        }
    }

    pub fn add_variant(&mut self, variant_name: &'a str) -> NameSpaceResult<()> {
        if self
            .variants
            .insert(variant_name, make_index_map())
            .is_some()
        {
            return Err(NameSpaceError::DuplicateEnumVariant {
                name: variant_name.to_string(),
            });
        }

        Ok(())
    }

    pub fn add_variant_field(
        &mut self,
        variant_name: &'a str,
        field_name: &'a str,
        field_type: TypeSymbol,
    ) -> NameSpaceResult<()> {
        let Some(variant) = self.variants.get_mut(variant_name) else {
            return Err(NameSpaceError::UnresolvedEnumVariant {
                name: variant_name.to_string(),
            });
        };

        if variant.insert(field_name, field_type).is_some() {
            return Err(NameSpaceError::DuplicateEnumVariant {
                name: field_name.to_string(),
            });
        }

        Ok(())
    }
}

impl<'a> ToPrimitiveType for EnumItem<'a> {
    fn to_type(&self) -> PrimitiveType {
        PrimitiveType::Enumeration {
            variants: self
                .variants
                .iter()
                .map(|(_, variant)| PrimitiveType::Struct {
                    elements: variant
                        .iter()
                        .map(|(_, elem)| elem.type_kind.clone())
                        .collect(),
                })
                .collect(),
        }
    }
}

fn make_index_map<K, V>() -> IndexMap<K, V> {
    IndexMap::with_hasher(Hasher::default())
}
