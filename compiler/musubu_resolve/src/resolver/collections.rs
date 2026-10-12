use super::*;
use alloc::boxed::Box;
use musubu_type_check::errors::TypeCheckError;

impl<'a> Resolver<'a> {
    pub(super) fn resolve_range(
        &mut self,
        start: Spanned<&'a Expression>,
        end: Spanned<&'a Expression>,
        inclusive: bool,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let start_span = start.span;
        let end_span = end.span;
        let start = self.resolve_expression(&start)?;
        let end = self.resolve_expression(&end)?;
        if !start.type_symbol.type_kind.is_integer() {
            return Err(ResolveError::from(TypeCheckError::InvalidOperation {
                op: "range".into(),
                reason: "range endpoints must be integers".into(),
            })
            .at(start_span));
        }
        if start.type_symbol.type_kind != end.type_symbol.type_kind {
            return Err(ResolveError::from(TypeCheckError::TypeMismatch {
                expected: start.type_symbol.type_kind,
                found: end.type_symbol.type_kind,
            })
            .at(end_span));
        }
        let type_symbol = TypeSymbol::new(PrimitiveType::Range {
            type_kind: Box::new(start.type_symbol.type_kind),
        });
        let hir = HIRExpression::Range {
            start: Box::new(start.hir),
            end: Box::new(end.hir),
            inclusive,
        };
        Ok(Lowered { type_symbol, hir })
    }

    pub(super) fn resolve_array_list(
        &mut self,
        list: &[Spanned<&'a Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let mut elements = Vec::with_capacity(list.len());
        let mut element_type: Option<PrimitiveType> = None;
        for expression in list {
            let element = self.resolve_expression(expression)?;
            if let Some(expected) = &element_type {
                if expected != &element.type_symbol.type_kind {
                    return Err(ResolveError::from(TypeCheckError::TypeMismatch {
                        expected: expected.clone(),
                        found: element.type_symbol.type_kind,
                    })
                    .at(expression.span));
                }
            } else {
                element_type = Some(element.type_symbol.type_kind.clone());
            }
            elements.push(element.hir);
        }
        let element_type = element_type.unwrap_or(PrimitiveType::Unit);
        let size = u32::try_from(elements.len()).map_err(|_| ResolveError::InvalidArrayLength)?;
        let type_symbol = TypeSymbol::new(PrimitiveType::Array {
            type_kind: Box::new(element_type.clone()),
            size,
        });
        Ok(Lowered {
            type_symbol,
            hir: HIRExpression::Array {
                elements,
                element_type,
            },
        })
    }

    pub(super) fn resolve_array_repeat(
        &mut self,
        value: Spanned<&'a Expression>,
        count: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let Expression::Literal(literal) = count.node else {
            return Err(ResolveError::InvalidArrayLength.at(count.span));
        };
        let Literal::Integer { value: length, .. } = &literal.node else {
            return Err(ResolveError::InvalidArrayLength.at(count.span));
        };
        self.resolve_expression(&count)?;
        let count_value = length
            .replace('_', "")
            .parse::<u32>()
            .map_err(|_| ResolveError::InvalidArrayLength.at(count.span))?;
        let value = self.resolve_expression(&value)?;
        let type_symbol = TypeSymbol::new(PrimitiveType::Array {
            type_kind: Box::new(value.type_symbol.type_kind),
            size: count_value,
        });
        Ok(Lowered {
            type_symbol,
            hir: HIRExpression::ArrayRepeat {
                value: Box::new(value.hir),
                count: count_value,
            },
        })
    }
}
