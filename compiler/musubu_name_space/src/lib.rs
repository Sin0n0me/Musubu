// TODO
// #![no_std]

extern crate alloc;

pub mod errors;

use crate::errors::NameSpaceError;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::hash::BuildHasherDefault;
use musubu_scope::TypeSymbol;
use twox_hash::XxHash64;

pub type NameSpaceResult<T> = Result<T, NameSpaceError>;
type Hasher = BuildHasherDefault<XxHash64>;
type IndexMap<K, V> = indexmap::IndexMap<K, V, Hasher>;

pub trait ItemStore<'a, E> {
    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), E>;
    fn add_struct(&mut self, struct_item: StructItem<'a>) -> Result<(), E>;
    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), E>;

    fn get_function(&self, name: &'a str) -> Result<&FunctionItem<'a>, E>;
    fn get_struct(&self, name: &'a str) -> Result<&StructItem<'a>, E>;
    fn get_enumeration(&self, name: &'a str) -> Result<&EnumItem<'a>, E>;
}

#[derive(Debug)]
pub struct Module<'a> {
    module_name: &'a str,
    children: BTreeMap<&'a str, Box<Module<'a>>>,
    items: BTreeMap<&'a str, ItemSymbol<'a>>,
}

impl<'a> ItemStore<'a, NameSpaceError> for Module<'a> {
    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), NameSpaceError> {
        let function_name = function_item.name;
        let pre = self
            .items
            .insert(function_name, ItemSymbol::Function(function_item));
        if pre.is_some() {
            return Err(NameSpaceError::DuplicateFunction {
                name: function_name.to_string(),
            });
        }

        Ok(())
    }

    fn add_struct(&mut self, struct_item: StructItem<'a>) -> Result<(), NameSpaceError> {
        let struct_name = struct_item.name;
        let pre = self
            .items
            .insert(struct_name, ItemSymbol::Struct(struct_item));
        if pre.is_some() {
            return Err(NameSpaceError::DuplicateStruct {
                name: struct_name.to_string(),
            });
        }

        Ok(())
    }

    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), NameSpaceError> {
        let enum_name = enum_item.name;
        let pre = self
            .items
            .insert(enum_name, ItemSymbol::Enumeration(enum_item));
        if pre.is_some() {
            return Err(NameSpaceError::DuplicateEnumeration {
                name: enum_name.to_string(),
            });
        }

        Ok(())
    }

    fn get_function(&self, name: &'a str) -> Result<&FunctionItem<'a>, NameSpaceError> {
        self.items
            .get(name)
            .and_then(ItemSymbol::get_function)
            .ok_or(NameSpaceError::UnresolveFunction {
                name: name.to_string(),
            })
    }

    fn get_struct(&self, name: &'a str) -> Result<&StructItem<'a>, NameSpaceError> {
        self.items.get(name).and_then(ItemSymbol::get_struct).ok_or(
            NameSpaceError::UnresolveStruct {
                name: name.to_string(),
            },
        )
    }

    fn get_enumeration(&self, name: &'a str) -> Result<&EnumItem<'a>, NameSpaceError> {
        self.items
            .get(name)
            .and_then(ItemSymbol::get_enumeration)
            .ok_or(NameSpaceError::UnresolveEnumeration {
                name: name.to_string(),
            })
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

    pub fn is_defined(&self, name: &'a str) -> bool {
        self.is_function(name) || self.is_struct(name) || self.is_enumeration(name)
    }

    pub fn is_function(&self, name: &'a str) -> bool {
        self.items
            .get(name)
            .filter(|item| matches!(item, ItemSymbol::Function(_)))
            .is_some()
    }

    pub fn is_struct(&self, name: &'a str) -> bool {
        self.items
            .get(name)
            .filter(|item| matches!(item, ItemSymbol::Struct(_)))
            .is_some()
    }

    pub fn is_enumeration(&self, name: &'a str) -> bool {
        self.items
            .get(name)
            .filter(|item| matches!(item, ItemSymbol::Enumeration(_)))
            .is_some()
    }

    pub fn add_module(&mut self, name: &'a str) {
        self.children
            .entry(name)
            .or_insert(Box::new(Module::new(name)));
    }
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

#[derive(Debug)]
pub enum ItemSymbol<'a> {
    Function(FunctionItem<'a>),
    Struct(StructItem<'a>),
    Enumeration(EnumItem<'a>),
}

#[derive(Debug)]
pub struct FunctionItem<'a> {
    pub name: &'a str,
    return_type: TypeSymbol,
    arguments: Vec<TypeSymbol>,
}

impl<'a> FunctionItem<'a> {
    pub fn new(name: &'a str, return_type: TypeSymbol) -> Self {
        Self {
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

#[derive(Debug)]
pub struct StructItem<'a> {
    name: &'a str,
    fields: IndexMap<&'a str, TypeSymbol>,
}

impl<'a> StructItem<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            fields: make_index_map(),
        }
    }

    pub fn add_field(&mut self, name: &'a str, type_symbol: TypeSymbol) -> NameSpaceResult<()> {
        self.fields
            .insert(name, type_symbol)
            .ok_or(NameSpaceError::DuplicateStructField {
                name: name.to_string(),
            })?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct EnumItem<'a> {
    name: &'a str,
    variants: IndexMap<&'a str, IndexMap<&'a str, TypeSymbol>>,
}

impl<'a> EnumItem<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            variants: make_index_map(),
        }
    }

    pub fn add_variant(&mut self, variant_name: &'a str) -> NameSpaceResult<()> {
        self.variants.insert(variant_name, make_index_map()).ok_or(
            NameSpaceError::DuplicateEnumeVariant {
                name: variant_name.to_string(),
            },
        )?;

        Ok(())
    }

    pub fn add_variant_field(
        &mut self,
        variant_name: &'a str,
        field_name: &'a str,
        field_type: TypeSymbol,
    ) -> NameSpaceResult<()> {
        let Some(variant) = self.variants.get_mut(variant_name) else {
            return Err(NameSpaceError::UnresolveEnumVariant {
                name: variant_name.to_string(),
            });
        };

        variant
            .insert(field_name, field_type)
            .ok_or(NameSpaceError::DuplicateEnumeVariant {
                name: field_name.to_string(),
            })?;

        Ok(())
    }
}

fn make_index_map<K, V>() -> IndexMap<K, V> {
    IndexMap::with_hasher(Hasher::default())
}
