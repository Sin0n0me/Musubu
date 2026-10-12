use super::*;
use alloc::collections::BTreeSet;
use musubu_primitive::{EnumVariant, EnumVariantKind};

pub(super) fn enum_error(message: alloc::string::String) -> ResolveError {
    ResolveError::InvalidEnum { message }
}

impl<'a> Resolver<'a> {
    pub(super) fn enum_variant(
        &self,
        path: Spanned<&'a Path>,
    ) -> ResolveResult<(PrimitiveType, usize, EnumVariant)> {
        if path.node.segments.len() != 2 {
            return Err(
                enum_error(format!("expected an enum variant path `Enum::Variant`")).at(path.span),
            );
        }
        let name = &path.node.segments[0].node.ident;
        let ty = self
            .name_resolver
            .get_item(name)
            .and_then(|item| item.get_enumeration())
            .map(ToPrimitiveType::to_type)
            .ok_or_else(|| enum_error(format!("`{name}` is not an enum")).at(path.span))?;
        let PrimitiveType::NamedEnum { variants, .. } = &ty else {
            unreachable!()
        };
        let variant_name = path.node.last_ident();
        let (index, variant) = variants
            .iter()
            .enumerate()
            .find(|(_, v)| v.name == variant_name)
            .ok_or_else(|| {
                enum_error(format!("enum `{name}` has no variant `{variant_name}`")).at(path.span)
            })?;
        let variant = variant.clone();
        Ok((ty, index, variant))
    }

    pub(super) fn resolve_enum_unit(
        &mut self,
        path: Spanned<&'a Path>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (ty, index, variant) = self.enum_variant(path.clone())?;
        if variant.kind != EnumVariantKind::Unit {
            return Err(enum_error(format!(
                "variant `{}` requires a payload constructor",
                variant.name
            ))
            .at(path.span));
        }
        Ok(enum_value(ty, index, Vec::new()))
    }

    pub(super) fn resolve_enum_tuple(
        &mut self,
        path: Spanned<&'a Path>,
        args: &[Spanned<&'a Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (ty, index, variant) = self.enum_variant(path.clone())?;
        if variant.kind != EnumVariantKind::Tuple {
            return Err(
                enum_error(format!("variant `{}` is not a tuple variant", variant.name))
                    .at(path.span),
            );
        }
        if args.len() != variant.fields.len() {
            return Err(enum_error(format!(
                "variant `{}` expects {} fields, found {}",
                variant.name,
                variant.fields.len(),
                args.len()
            ))
            .at(path.span));
        }
        let mut fields = Vec::new();
        for (i, (arg, (_, expected))) in args.iter().zip(&variant.fields).enumerate() {
            let value = self.resolve_expression(arg)?;
            check_payload_type(expected, &value.type_symbol.type_kind)
                .map_err(|e| e.at(arg.span))?;
            fields.push((i, value.hir));
        }
        Ok(enum_value(ty, index, fields))
    }

    pub(super) fn resolve_enum_struct(
        &mut self,
        path: Spanned<&'a Path>,
        initializers: &'a [(Spanned<alloc::string::String>, SpannedBox<Expression>)],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let (ty, index, variant) = self.enum_variant(path.clone())?;
        if variant.kind != EnumVariantKind::Struct {
            return Err(enum_error(format!(
                "variant `{}` is not a named-field variant",
                variant.name
            ))
            .at(path.span));
        }
        let mut seen = BTreeSet::new();
        let mut fields = Vec::new();
        for (name, expr) in initializers {
            let (i, (_, expected)) = variant
                .fields
                .iter()
                .enumerate()
                .find(|(_, (field, _))| field == &name.node)
                .ok_or_else(|| {
                    enum_error(format!(
                        "variant `{}` has no field `{}`",
                        variant.name, name.node
                    ))
                    .at(name.span)
                })?;
            if !seen.insert(i) {
                return Err(enum_error(format!(
                    "field `{}` is initialized more than once",
                    name.node
                ))
                .at(name.span));
            }
            let value = self.resolve_expression(&expr.as_ref_spanned())?;
            check_payload_type(expected, &value.type_symbol.type_kind)
                .map_err(|e| e.at(expr.span))?;
            fields.push((i, value.hir));
        }
        if let Some((_, (name, _))) = variant
            .fields
            .iter()
            .enumerate()
            .find(|(i, _)| !seen.contains(i))
        {
            return Err(enum_error(format!(
                "missing field `{name}` in variant `{}`",
                variant.name
            ))
            .at(path.span));
        }
        Ok(enum_value(ty, index, fields))
    }
}

fn enum_value(
    enum_type: PrimitiveType,
    variant: usize,
    fields: Vec<(usize, HIRExpression)>,
) -> Lowered<HIRExpression> {
    Lowered {
        type_symbol: TypeSymbol::new(enum_type.clone()),
        hir: HIRExpression::Enum {
            variant,
            fields,
            enum_type,
        },
    }
}

pub(super) fn check_payload_type(
    expected: &PrimitiveType,
    found: &PrimitiveType,
) -> ResolveResult<()> {
    if expected == found {
        return Ok(());
    }
    Err(musubu_type_check::errors::TypeCheckError::TypeMismatch {
        expected: expected.clone(),
        found: found.clone(),
    }
    .into())
}
