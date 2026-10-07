use super::enumeration::{check_payload_type, enum_error};
use super::*;
use alloc::{boxed::Box, collections::BTreeSet};
use musubu_hir::{HIRMatchArm, HIRMatchPattern};

impl<'a> Resolver<'a> {
    pub(super) fn resolve_match(
        &mut self,
        value: Spanned<&'a Expression>,
        arms: &'a [MatchArm],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let resolved = self.resolve_expression(&value)?;
        let ty = &resolved.type_symbol.type_kind;
        if !matches!(
            ty,
            PrimitiveType::NamedEnum { .. } | PrimitiveType::Tuple { .. } | PrimitiveType::Unit
        ) {
            return Err(enum_error(format!(
                "match requires an enum value or tuple, found {}",
                ty.to_string()
            ))
            .at(value.span));
        }
        let mut lowered = Vec::new();
        let mut patterns = Vec::new();
        let mut result_type = None;
        for arm in arms {
            let arm_value = self.enter_scope(|s| {
                let pattern = s.resolve_match_pattern(
                    arm.pattern.as_ref_spanned(),
                    ty,
                    &mut BTreeSet::new(),
                )?;
                if !match_coverage::useful(&patterns, &pattern, ty) {
                    return Err(enum_error(format!("unreachable match arm")).at(arm.pattern.span));
                }
                let body = s.resolve_expression(&arm.body.as_ref_spanned())?;
                Ok(Lowered {
                    type_symbol: body.type_symbol,
                    hir: (pattern, body.hir),
                })
            })?;
            let (pattern, body_hir) = arm_value.hir;
            let body = Lowered {
                type_symbol: arm_value.type_symbol,
                hir: body_hir,
            };
            if control_flow::can_complete(&body.hir.clone().to_block()) {
                if let Some(expected) = &result_type {
                    check_payload_type(expected, &body.type_symbol.type_kind)
                        .map_err(|e| e.at(arm.body.span))?;
                } else {
                    result_type = Some(body.type_symbol.type_kind.clone());
                }
            }
            patterns.push(pattern.clone());
            lowered.push(HIRMatchArm {
                pattern,
                body: body.hir,
            });
        }
        if match_coverage::useful(&patterns, &HIRMatchPattern::Wildcard, ty) {
            return Err(enum_error(format!(
                "non-exhaustive match for {}; add the missing variants or `_`",
                ty.to_string()
            ))
            .at(value.span));
        }
        let result_type = result_type.unwrap_or(PrimitiveType::Unit);
        Ok(Lowered {
            type_symbol: TypeSymbol::new(result_type.clone()),
            hir: HIRExpression::Match {
                value: Box::new(resolved.hir),
                arms: lowered,
                result_type,
            },
        })
    }

    fn resolve_match_pattern(
        &mut self,
        pattern: Spanned<&'a MatchPattern>,
        ty: &PrimitiveType,
        names: &mut BTreeSet<&'a str>,
    ) -> ResolveResult<HIRMatchPattern> {
        let result = match pattern.node {
            MatchPattern::Tuple(patterns) => {
                let elements = super::tuples::tuple_elements(ty, patterns.len())
                    .map_err(|e| e.at(pattern.span))?;
                let fields = patterns
                    .iter()
                    .zip(elements)
                    .enumerate()
                    .map(|(i, (p, ty))| {
                        self.resolve_match_pattern(p.as_ref_spanned(), ty, names)
                            .map(|p| (i, p))
                    })
                    .collect::<ResolveResult<Vec<_>>>()?;
                Ok(HIRMatchPattern::Tuple(fields))
            }
            MatchPattern::Wildcard => Ok(HIRMatchPattern::Wildcard),
            MatchPattern::Binding { name, mutable } => {
                self.bind_match_name(name, *mutable, ty, names)
            }
            MatchPattern::Variant {
                path,
                kind,
                fields,
                rest,
            } => {
                let (enum_type, index, variant) = self.enum_variant(path.as_ref_spanned())?;
                check_payload_type(ty, &enum_type).map_err(|e| e.at(path.span))?;
                if kind != &variant.kind {
                    return Err(enum_error(format!(
                        "wrong pattern shape for variant `{}`",
                        variant.name
                    ))
                    .at(pattern.span));
                }
                let mut seen = BTreeSet::new();
                let mut patterns = Vec::new();
                for (field, inner) in fields {
                    let (i, (_, field_type)) = variant
                        .fields
                        .iter()
                        .enumerate()
                        .find(|(_, (name, _))| name == &field.node)
                        .ok_or_else(|| {
                            enum_error(format!(
                                "variant `{}` has no field `{}`",
                                variant.name, field.node
                            ))
                            .at(field.span)
                        })?;
                    if !seen.insert(i) {
                        return Err(enum_error(format!(
                            "duplicate pattern field `{}`",
                            field.node
                        ))
                        .at(field.span));
                    }
                    patterns.push((
                        i,
                        self.resolve_match_pattern(inner.as_ref_spanned(), field_type, names)?,
                    ));
                }
                if !rest && seen.len() != variant.fields.len() {
                    return Err(enum_error(format!(
                        "variant `{}` pattern expects {} fields, found {}",
                        variant.name,
                        variant.fields.len(),
                        seen.len()
                    ))
                    .at(pattern.span));
                }
                Ok(HIRMatchPattern::Variant {
                    index,
                    fields: patterns,
                })
            }
        };
        result.map_err(|e: ResolveError| e.at(pattern.span))
    }

    fn bind_match_name(
        &mut self,
        name: &'a str,
        mutable: bool,
        ty: &PrimitiveType,
        names: &mut BTreeSet<&'a str>,
    ) -> ResolveResult<HIRMatchPattern> {
        if !names.insert(name) {
            return Err(enum_error(format!("duplicate pattern binding `{name}`")));
        }
        let id = self.desugar.alloc_symbol();
        self.name_resolver.add_variable(
            id,
            name,
            TypeRequirement::Expect(TypeSymbol {
                type_kind: ty.clone(),
                option: TypeOption {
                    mutable,
                    reference: false,
                },
            }),
        )?;
        Ok(HIRMatchPattern::Binding(id))
    }
}
