use crate::errors::ResolveError;
use crate::{ResolveResult, Resolver};
use alloc::collections::btree_set::BTreeSet;
use alloc::string::ToString;
use alloc::vec::Vec;
use musubu_ast::*;
use musubu_name_space::{FunctionItem, ItemStore, ItemSymbol, StructItem};
use musubu_primitive::PrimitiveType;
use musubu_scope::TypeSymbol;
use musubu_span::{Spanned, SpannedAsRef};

#[derive(Debug)]
pub(crate) struct SymbolCollector<'a> {
    collector: BTreeSet<&'a str>,
    pending_items: Vec<ItemSymbol<'a>>,
    //pending_symbols: Vec<Symbol>
}

impl<'a> SymbolCollector<'a> {
    pub fn new() -> Self {
        Self {
            collector: BTreeSet::new(),
            pending_items: Vec::new(),
        }
    }

    pub fn contains(&self, name: &'a str) -> bool {
        self.collector.contains(name)
    }

    pub fn insert(&mut self, name: &'a str) -> ResolveResult<()> {
        if !self.collector.insert(name) {
            return Err(ResolveError::DuplicateDefinition {
                name: name.to_string(),
            });
        }

        Ok(())
    }

    pub fn remove(&mut self, name: &'a str) -> bool {
        self.collector.remove(name)
    }
}

impl<'a> Resolver<'a> {
    // 先にrevoleveだけを呼び出すとCのようなに後に定義されたシンボルは定義されていないものとする
    // 先に定義だけを収集する用(その分少し重い)
    pub fn import(&mut self, module_name: &'a str, nodes: &[&'a ASTNode]) -> ResolveResult<()> {
        self.enter_module(module_name, |s| {
            for node in nodes {
                match node {
                    ASTNode::Item {
                        visibility: _,
                        item,
                    } => s.import_item(item.as_ref_spanned())?,
                    _ => (),
                };
            }

            Ok(())
        })
    }

    pub(crate) fn import_item(
        &mut self,
        //visibility: &'a Visibility,
        item: Spanned<&'a Item>,
    ) -> ResolveResult<()> {
        match &item.node {
            Item::Struct { name, fields } => {
                self.import_struct(name, fields)?;
            }
            Item::Function {
                name,
                params,
                return_type,
                body: _,
            } => {
                self.import_function(
                    name,
                    params,
                    return_type.as_ref().map(|r| r.as_ref_spanned()),
                )?;
            }
            Item::Enumeration { name, items } => {
                self.import_enumeration(name, items)?;
            }
            Item::Union { name, fields } => {
                // TODO
                self.collector.insert(name)?;
            }
        };

        Ok(())
    }

    pub(crate) fn import_struct(
        &mut self,
        name: &'a str,
        fields: &'a [Spanned<StructField>],
    ) -> ResolveResult<()> {
        let mut struct_item = StructItem::new(name);
        for field in fields {
            let field = &field.node;
            let field_type = self.import_type(field.field_type.as_ref_spanned())?;
            struct_item.add_field(&field.name, field_type)?;
        }

        self.name_resolver.add_struct(struct_item)?;
        self.collector.insert(name)?;

        Ok(())
    }

    pub(crate) fn import_function(
        &mut self,
        name: &'a str,
        params: &'a [Spanned<FunctionParam>],
        return_type: Option<Spanned<&'a TypeKind>>,
    ) -> ResolveResult<()> {
        // 戻り型
        let return_type = if let Some(return_type) = return_type {
            self.import_type(return_type)?
        } else {
            TypeSymbol::default()
        };

        let func_id = self.desuger.alloc_function();
        let mut function_item = FunctionItem::new(func_id, name, return_type);

        // 引数
        for param in params {
            let param = &param.node;
            let arg_type = self.import_type(param.param_type.as_ref_spanned())?;
            function_item.add_argument(arg_type)?;
        }

        self.name_resolver.add_function(function_item)?;
        self.collector.insert(name)?;

        Ok(())
    }

    pub(crate) fn import_enumeration(
        &mut self,
        enum_name: &'a str,
        items: &'a [Spanned<musubu_ast::EnumItem>],
    ) -> ResolveResult<()> {
        let mut enum_item = musubu_name_space::EnumItem::new(enum_name);
        for item in items {
            match &item.node {
                musubu_ast::EnumItem::StructItem {
                    name,
                    fields,
                    visibility: _,
                } => {
                    for field in fields {
                        let field = &field.node;
                        let field_name = &field.name;
                        let field_type = self.import_type(field.field_type.as_ref_spanned())?;

                        enum_item.add_variant_field(name, field_name, field_type)?;
                    }
                }
                musubu_ast::EnumItem::TupleItem {
                    name,
                    visibility: _,
                } => {
                    enum_item.add_variant(name)?;
                }
            }
        }

        self.name_resolver.add_enumeration(enum_item)?;
        self.collector.insert(enum_name)?;

        Ok(())
    }

    fn import_type(&mut self, type_kind: Spanned<&'a TypeKind>) -> ResolveResult<TypeSymbol> {
        let type_kind = &type_kind.node;
        let scope = self.get_scope()?;
        //let ty = self.type_checker.check_type(scope, type_kind)?;

        let ty = match type_kind {
            TypeKind::Primitive(_) => self.type_checker.check_type(scope, type_kind)?,
            TypeKind::PathType(path) => self.import_path(path.as_ref_spanned())?,
            TypeKind::Function {
                arguments,
                return_type,
            } => {
                for arg in arguments {
                    self.import_type(arg.as_ref_spanned())?;
                }
                self.import_type(return_type.as_ref_spanned())?;

                TypeSymbol::default()
            }
        };

        Ok(ty)
    }

    fn import_path(&mut self, path: Spanned<&'a Path>) -> ResolveResult<TypeSymbol> {
        let path = &path.node;
        let name = path.last_ident();

        if let Some(type_kind) = PrimitiveType::from(name) {
            return Ok(TypeSymbol::new(type_kind));
        }

        self.name_resolver
            .get_type(name)
            .ok_or(ResolveError::UnresolveType {
                name: name.to_string(),
            })
            .cloned()
    }
}
