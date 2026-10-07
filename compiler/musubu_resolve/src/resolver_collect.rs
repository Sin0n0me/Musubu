use crate::errors::ResolveError;
use crate::{ResolveResult, Resolver};
use alloc::collections::btree_set::BTreeSet;
use alloc::string::ToString;
use alloc::vec::Vec;
use musubu_ast::*;
use musubu_name_space::{FunctionItem, ItemStore, ItemSymbol, StructItem};
use musubu_primitive::{EnumVariantKind, PrimitiveType, ToPrimitiveType};
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
                    if let Item::Struct { name, .. } | Item::Enumeration { name, .. } = &item.node {
                        s.import_struct_dependency(name, nodes, &mut visiting)?;
                    }
                }
            }
            for node in nodes {
                match node {
                    ASTNode::Item {
                        item:
                            Spanned {
                                node: Item::Struct { .. } | Item::Enumeration { .. },
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
            ASTNode::Item { item, .. } if matches!(&item.node, Item::Struct { name: n, .. } | Item::Enumeration { name: n, .. } if n == name) => Some(item),
            _ => None,
        }) else { return Ok(()); };
        if !visiting.insert(name) {
            return Err(ResolveError::InvalidStruct {
                message: alloc::format!("recursive type `{name}` has infinite size"),
            }
            .at(item.span));
        }
        let fields: Vec<_> = match &item.node {
            Item::Struct { fields, .. } => fields.iter().collect(),
            Item::Enumeration { items, .. } => items
                .iter()
                .flat_map(|item| match &item.node {
                    EnumItem::StructItem { fields, .. } | EnumItem::TupleItem { fields, .. } => {
                        fields.as_slice()
                    }
                    EnumItem::UnitItem { .. } => &[],
                })
                .collect(),
            _ => unreachable!(),
        };
        for field in fields {
            self.import_type_dependencies(&field.node.field_type.node, nodes, visiting)?;
        }
        match &item.node {
            Item::Struct { fields, .. } => self.import_struct(name, fields),
            Item::Enumeration { items, .. } => self.import_enumeration(name, items),
            _ => unreachable!(),
        }
        .map_err(|error| error.at(item.span))?;
        visiting.remove(name);
        Ok(())
    }

    fn import_type_dependencies(
        &mut self,
        ty: &'a TypeKind,
        nodes: &[&'a ASTNode],
        visiting: &mut BTreeSet<&'a str>,
    ) -> ResolveResult<()> {
        match ty {
            TypeKind::PathType(path) => {
                self.import_struct_dependency(path.node.last_ident(), nodes, visiting)?
            }
            TypeKind::Tuple(elements) => {
                for element in elements {
                    self.import_type_dependencies(&element.node, nodes, visiting)?;
                }
            }
            _ => {}
        }
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
            let (name, kind, fields) = match &item.node {
                EnumItem::UnitItem { name } => (name, EnumVariantKind::Unit, &[][..]),
                EnumItem::TupleItem { name, fields, .. } => {
                    (name, EnumVariantKind::Tuple, fields.as_slice())
                }
                EnumItem::StructItem { name, fields, .. } => {
                    (name, EnumVariantKind::Struct, fields.as_slice())
                }
            };
            enum_item
                .add_variant(name)
                .map_err(|error| ResolveError::from(error).at(item.span))?;
            enum_item.variant_kinds.insert(name, kind);
            for field in fields {
                let ty = self.import_type(field.node.field_type.as_ref_spanned())?;
                enum_item
                    .add_variant_field(name, &field.node.name, ty)
                    .map_err(|error| ResolveError::from(error).at(field.span))?;
            }
        }

        self.name_resolver.add_enumeration(enum_item)?;
        self.collector.insert(enum_name)?;

        Ok(())
    }

    fn import_type(&mut self, type_kind: Spanned<&'a TypeKind>) -> ResolveResult<TypeSymbol> {
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
