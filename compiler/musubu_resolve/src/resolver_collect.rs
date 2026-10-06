use crate::errors::ResolveError;
use crate::{ResolveResult, Resolver};
use alloc::collections::btree_set::BTreeSet;
use alloc::string::ToString;
use alloc::vec::Vec;
use musubu_ast::*;
use musubu_name_space::{FunctionItem, ItemStore, ItemSymbol, StructItem};
use musubu_primitive::{PrimitiveType, ToPrimitiveType};
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
    pub(crate) fn import(
        &mut self,
        module_name: &'a str,
        nodes: &[&'a ASTNode],
    ) -> ResolveResult<()> {
        self.enter_module(module_name, |s| {
            let mut names = BTreeSet::new();
            for node in nodes {
                if let ASTNode::Item { item, .. } = node {
                    let name = match &item.node {
                        Item::Struct { name, .. }
                        | Item::Function { name, .. }
                        | Item::Enumeration { name, .. }
                        | Item::Union { name, .. } => name,
                    };
                    if !names.insert(name) {
                        return Err(ResolveError::DuplicateDefinition {
                            name: name.to_string(),
                        }
                        .at(item.span));
                    }
                }
            }
            let mut visiting = BTreeSet::new();
            for node in nodes {
                if let ASTNode::Item { item, .. } = node {
                    if let Item::Struct { name, .. } = &item.node {
                        s.import_struct_dependency(name, nodes, &mut visiting)?;
                    }
                }
            }
            for node in nodes {
                match node {
                    ASTNode::Item {
                        item:
                            Spanned {
                                node: Item::Struct { .. },
                                ..
                            },
                        ..
                    } => {}
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

    fn import_struct_dependency(
        &mut self,
        name: &'a str,
        nodes: &[&'a ASTNode],
        visiting: &mut BTreeSet<&'a str>,
    ) -> ResolveResult<()> {
        if self.collector.contains(name) {
            return Ok(());
        }
        let Some(item) = nodes.iter().find_map(|node| match node {
            ASTNode::Item { item, .. } if matches!(&item.node, Item::Struct { name: n, .. } if n == name) => Some(item),
            _ => None,
        }) else { return Ok(()); };
        if !visiting.insert(name) {
            return Err(ResolveError::InvalidStruct {
                message: alloc::format!("recursive struct `{name}` has infinite size"),
            }
            .at(item.span));
        }
        let Item::Struct { fields, .. } = &item.node else {
            unreachable!()
        };
        for field in fields {
            if let TypeKind::PathType(path) = &field.node.field_type.node {
                self.import_struct_dependency(path.node.last_ident(), nodes, visiting)?;
            }
        }
        self.import_struct(name, fields)
            .map_err(|error| error.at(item.span))?;
        visiting.remove(name);
        Ok(())
    }

    pub(crate) fn import_item(
        &mut self,
        //visibility: &'a Visibility,
        item: Spanned<&'a Item>,
    ) -> ResolveResult<()> {
        let diagnostic_span = item.span;
        (|| {
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
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
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

        let full_name = self.name_resolver.get_full_path(name).join("::");
        let func_id = self.desugar.alloc_function(full_name);
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

            Ok(ty)
        })()
        .map_err(|error: ResolveError| error.at(diagnostic_span))
    }

    pub(super) fn import_path(&mut self, path: Spanned<&'a Path>) -> ResolveResult<TypeSymbol> {
        let path = &path.node;

        let name = path.last_ident();

        if let Some(type_kind) = PrimitiveType::from(name) {
            return Ok(TypeSymbol::new(type_kind));
        }

        // Type paths use the type namespace, independently of local value bindings.
        match self.name_resolver.get_item(name) {
            Some(ItemSymbol::Struct(item)) => Ok(TypeSymbol::new(item.to_type())),
            Some(ItemSymbol::Enumeration(item)) => Ok(TypeSymbol::new(item.to_type())),
            _ => Err(ResolveError::UnresolvedType {
                name: name.to_string(),
            }),
        }
    }
}
