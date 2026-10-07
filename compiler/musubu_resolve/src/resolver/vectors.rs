use super::*;
use alloc::boxed::Box;

impl<'a> Resolver<'a> {
    pub(super) fn resolve_vector_constructor(
        &mut self,
        ty: PrimitiveType,
        arguments: &[Spanned<&'a Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let PrimitiveType::Vector {
            dimension,
            type_kind,
        } = &ty
        else {
            unreachable!();
        };
        if arguments.len() != *dimension as usize {
            return Err(ResolveError::InvalidVector {
                message: format!(
                    "vector constructor expects {dimension} components, found {}",
                    arguments.len()
                ),
            });
        }
        let mut fields = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let value = self.resolve_expression(argument)?;
            super::enumeration::check_payload_type(type_kind, &value.type_symbol.type_kind)
                .map_err(|e| e.at(argument.span))?;
            fields.push((index, value.hir));
        }
        Ok(Lowered {
            type_symbol: TypeSymbol::new(ty.clone()),
            hir: HIRExpression::Struct {
                fields,
                struct_type: ty,
            },
        })
    }

    pub(super) fn resolve_vector_field(
        &self,
        parent: Lowered<HIRExpression>,
        field: &str,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let PrimitiveType::Vector {
            dimension,
            type_kind,
        } = &parent.type_symbol.type_kind
        else {
            unreachable!();
        };
        let index = ["x", "y", "z", "w"]
            .iter()
            .position(|name| *name == field)
            .filter(|index| *index < *dimension as usize);
        let Some(index) = index else {
            return Err(ResolveError::InvalidVector {
                message: format!(
                    "vector {} has no component `{field}`",
                    parent.type_symbol.type_kind.to_string()
                ),
            });
        };
        let ty = type_kind.as_ref().clone();
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
}
