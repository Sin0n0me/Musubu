use super::*;
use alloc::{boxed::Box, collections::BTreeSet};
use musubu_primitive::Value;

pub(super) fn tuple_error(message: alloc::string::String) -> ResolveError {
    ResolveError::InvalidTuple { message }
}

pub(super) fn tuple_elements(ty: &PrimitiveType, count: usize) -> ResolveResult<&[PrimitiveType]> {
    let elements = match ty {
        PrimitiveType::Tuple { elements } => elements.as_slice(),
        PrimitiveType::Unit => &[],
        _ => {
            return Err(tuple_error(format!(
                "tuple pattern requires a tuple, found {}",
                ty.to_string()
            )));
        }
    };
    if elements.len() != count {
        return Err(tuple_error(format!(
            "tuple pattern expects {} elements, found {count}",
            elements.len()
        )));
    }
    Ok(elements)
}

impl<'a> Resolver<'a> {
    pub(crate) fn resolve_tuple_type(
        &mut self,
        elements: &'a [Spanned<TypeKind>],
    ) -> ResolveResult<TypeSymbol> {
        let types = elements
            .iter()
            .map(|ty| self.resolve_type(ty.as_ref_spanned()).map(|t| t.type_kind))
            .collect::<ResolveResult<Vec<_>>>()?;
        Ok(TypeSymbol::new(if types.is_empty() {
            PrimitiveType::Unit
        } else {
            PrimitiveType::Tuple { elements: types }
        }))
    }

    pub(super) fn resolve_tuple(
        &mut self,
        elements: &'a [Spanned<Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        if elements.is_empty() {
            return Ok(Lowered {
                type_symbol: TypeSymbol::default(),
                hir: HIRExpression::Literal(Value::Unit),
            });
        }
        let mut fields = Vec::new();
        let mut types = Vec::new();
        for (index, expression) in elements.iter().enumerate() {
            let value = self.resolve_expression(&expression.as_ref_spanned())?;
            types.push(value.type_symbol.type_kind);
            fields.push((index, value.hir));
        }
        let ty = PrimitiveType::Tuple { elements: types };
        Ok(Lowered {
            type_symbol: TypeSymbol::new(ty.clone()),
            hir: HIRExpression::Struct {
                fields,
                struct_type: ty,
            },
        })
    }

    pub(super) fn resolve_tuple_field(
        &self,
        parent: Lowered<HIRExpression>,
        field: &str,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let PrimitiveType::Tuple { elements } = &parent.type_symbol.type_kind else {
            unreachable!();
        };
        let index = field
            .parse::<usize>()
            .ok()
            .filter(|_| field.bytes().all(|b| b.is_ascii_digit()));
        let Some((index, ty)) = index.and_then(|i| elements.get(i).map(|ty| (i, ty.clone())))
        else {
            return Err(tuple_error(format!(
                "tuple {} has no element `{field}`",
                parent.type_symbol.type_kind.to_string()
            )));
        };
        let mut type_symbol = parent.type_symbol;
        type_symbol.type_kind = ty.clone();
        Ok(Lowered {
            type_symbol,
            hir: HIRExpression::Field {
                parent: Box::new(parent.hir),
                index,
                field_type: ty,
            },
        })
    }

    pub(super) fn resolve_tuple_let(
        &mut self,
        pattern: &'a Spanned<Pattern>,
        initializer: Option<Spanned<&'a Expression>>,
        annotation: Option<Spanned<&'a TypeKind>>,
    ) -> ResolveResult<Lowered<Option<HIRStatement>>> {
        let initializer = initializer.ok_or_else(|| {
            tuple_error("tuple destructuring requires an initializer".to_string()).at(pattern.span)
        })?;
        let value = self.resolve_expression(&initializer)?;
        let ty = value.type_symbol.type_kind;
        if let Some(annotation) = annotation {
            let expected = self.resolve_type(annotation)?;
            enumeration::check_payload_type(&expected.type_kind, &ty)
                .map_err(|e| e.at(initializer.span))?;
        }
        // Snapshot once before bindings shadow names used by the initializer.
        let symbol = self.desugar.alloc_symbol();
        let mut statements = alloc::vec![HIRStatement::Let {
            symbol,
            symbol_type: ty.clone(),
            initializer: Some(value.hir)
        }];
        self.destructure_tuple_let(
            pattern.as_ref_spanned(),
            &ty,
            HIRExpression::Variable {
                id: symbol,
                symbol_type: ty.clone(),
            },
            &mut BTreeSet::new(),
            &mut statements,
        )?;
        Ok(Lowered {
            type_symbol: TypeSymbol::default(),
            hir: Some(HIRStatement::Discard(HIRExpression::Block(HIRBlock {
                statements,
            }))),
        })
    }

    fn destructure_tuple_let(
        &mut self,
        pattern: Spanned<&'a Pattern>,
        ty: &PrimitiveType,
        value: HIRExpression,
        names: &mut BTreeSet<&'a str>,
        statements: &mut Vec<HIRStatement>,
    ) -> ResolveResult<()> {
        match pattern.node {
            Pattern::Tuple(patterns) => {
                let elements =
                    tuple_elements(ty, patterns.len()).map_err(|e| e.at(pattern.span))?;
                for (index, (pattern, ty)) in patterns.iter().zip(elements).enumerate() {
                    let field = HIRExpression::Field {
                        parent: Box::new(value.clone()),
                        index,
                        field_type: ty.clone(),
                    };
                    self.destructure_tuple_let(
                        pattern.as_ref_spanned(),
                        ty,
                        field,
                        names,
                        statements,
                    )?;
                }
            }
            Pattern::Identifier {
                ident,
                reference: false,
                ..
            } => {
                if !names.insert(ident) {
                    return Err(
                        tuple_error(format!("duplicate tuple binding `{ident}`")).at(pattern.span)
                    );
                }
                let (symbol, _) =
                    self.resolve_pattern(&pattern, Some(&TypeSymbol::new(ty.clone())))?;
                statements.push(HIRStatement::Let {
                    symbol,
                    symbol_type: ty.clone(),
                    initializer: Some(value),
                });
            }
            Pattern::None => {}
            _ => {
                return Err(tuple_error(
                    "expected an irrefutable tuple pattern, binding or `_`".to_string(),
                )
                .at(pattern.span));
            }
        }
        Ok(())
    }
}
