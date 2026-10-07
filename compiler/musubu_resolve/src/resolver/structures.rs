use super::*;
use alloc::boxed::Box;
use alloc::collections::BTreeSet;

impl<'a> Resolver<'a> {
    pub(super) fn resolve_struct_literal(
        &mut self,
        path: Spanned<&'a Path>,
        initializers: &'a [(Spanned<alloc::string::String>, SpannedBox<Expression>)],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        if path.node.segments.len() > 1 {
            return self.resolve_enum_struct(path, initializers);
        }
        let name = path.node.last_ident();
        let struct_type = self
            .name_resolver
            .get_struct(name)
            .map(|item| item.to_type())
            .ok_or_else(|| {
                ResolveError::InvalidStruct {
                    message: alloc::format!("`{name}` is not a struct"),
                }
                .at(path.span)
            })?;
        let PrimitiveType::NamedStruct { fields, .. } = &struct_type else {
            unreachable!();
        };
        let mut seen = BTreeSet::new();
        let mut values = Vec::new();
        for (field, value) in initializers {
            let Some((index, (_, expected))) = fields
                .iter()
                .enumerate()
                .find(|(_, (name, _))| name == &field.node)
            else {
                return Err(ResolveError::InvalidStruct {
                    message: alloc::format!("struct `{name}` has no field `{}`", field.node),
                }
                .at(field.span));
            };
            if !seen.insert(index) {
                return Err(ResolveError::InvalidStruct {
                    message: alloc::format!("field `{}` is initialized more than once", field.node),
                }
                .at(field.span));
            }
            let resolved = self.resolve_expression(&value.as_ref_spanned())?;
            if &resolved.type_symbol.type_kind != expected {
                return Err(ResolveError::from(
                    musubu_type_check::errors::TypeCheckError::TypeMismatch {
                        expected: expected.clone(),
                        found: resolved.type_symbol.type_kind,
                    },
                )
                .at(value.span));
            }
            values.push((index, resolved.hir));
        }
        if let Some((_, (field, _))) = fields.iter().enumerate().find(|(i, _)| !seen.contains(i)) {
            return Err(ResolveError::InvalidStruct {
                message: alloc::format!("missing field `{field}` in struct `{name}`"),
            }
            .at(path.span));
        }
        Ok(Lowered {
            type_symbol: TypeSymbol::new(struct_type.clone()),
            hir: HIRExpression::Struct {
                fields: values,
                struct_type,
            },
        })
    }

    pub(super) fn resolve_struct_field(
        &mut self,
        expression: &Spanned<&'a Expression>,
        field_name: &str,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let parent = self.resolve_expression(expression)?;
        if matches!(parent.type_symbol.type_kind, PrimitiveType::Vector { .. }) {
            return self.resolve_vector_field(parent, field_name);
        }
        if matches!(parent.type_symbol.type_kind, PrimitiveType::Tuple { .. }) {
            return self.resolve_tuple_field(parent, field_name);
        }
        let PrimitiveType::NamedStruct { name, fields } = &parent.type_symbol.type_kind else {
            return Err(ResolveError::InvalidStruct {
                message: alloc::format!(
                    "type {} has no field `{field_name}`",
                    parent.type_symbol.type_kind.to_string()
                ),
            });
        };
        let Some((index, (_, ty))) = fields
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == field_name)
        else {
            return Err(ResolveError::InvalidStruct {
                message: alloc::format!("struct `{name}` has no field `{field_name}`"),
            });
        };
        let mut type_symbol = parent.type_symbol.clone();
        type_symbol.type_kind = ty.clone();
        Ok(Lowered {
            type_symbol,
            hir: HIRExpression::Field {
                parent: Box::new(parent.hir),
                index,
                field_type: ty.clone(),
            },
        })
    }
}
