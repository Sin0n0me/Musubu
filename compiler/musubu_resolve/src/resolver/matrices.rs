use super::*;
use alloc::boxed::Box;

impl<'a> Resolver<'a> {
    pub(super) fn resolve_matrix_constructor(
        &mut self,
        ty: PrimitiveType,
        arguments: &[Spanned<&'a Expression>],
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let PrimitiveType::Matrix {
            columns,
            rows,
            type_kind,
        } = &ty
        else {
            unreachable!();
        };
        if arguments.len() != *columns as usize {
            return Err(ResolveError::InvalidMatrix {
                message: format!(
                    "matrix constructor expects {columns} columns, found {}",
                    arguments.len()
                ),
            });
        }
        let expected = PrimitiveType::Vector {
            dimension: *rows,
            type_kind: type_kind.clone(),
        };
        let mut fields = Vec::new();
        for (i, argument) in arguments.iter().enumerate() {
            let value = self.resolve_expression(argument)?;
            super::enumeration::check_payload_type(&expected, &value.type_symbol.type_kind)
                .map_err(|e| e.at(argument.span))?;
            fields.push((i, value.hir));
        }
        Ok(Lowered {
            type_symbol: TypeSymbol::new(ty.clone()),
            hir: HIRExpression::Struct {
                fields,
                struct_type: ty,
            },
        })
    }

    pub(super) fn resolve_numeric_index(
        &mut self,
        parent: Spanned<&'a Expression>,
        index: Spanned<&'a Expression>,
    ) -> ResolveResult<Lowered<HIRExpression>> {
        let parent = self.resolve_expression(&parent)?;
        let (size, ty) = match &parent.type_symbol.type_kind {
            PrimitiveType::Matrix {
                columns,
                rows,
                type_kind,
            } => (
                *columns,
                PrimitiveType::Vector {
                    dimension: *rows,
                    type_kind: type_kind.clone(),
                },
            ),
            PrimitiveType::Vector {
                dimension,
                type_kind,
            } => (*dimension, *type_kind.clone()),
            _ => {
                return Err(ResolveError::Unsupported {
                    feature: "index expressions on this type",
                });
            }
        };
        let resolved_index = self.resolve_expression(&index)?;
        if !resolved_index.type_symbol.type_kind.is_integer() {
            return Err(ResolveError::InvalidMatrix {
                message: "matrix/vector index must be an integer".into(),
            }
            .at(index.span));
        }
        check_constant_index(index, size)?;
        let mut type_symbol = parent.type_symbol;
        type_symbol.type_kind = ty.clone();
        Ok(Lowered {
            type_symbol,
            hir: HIRExpression::Index {
                parent: Box::new(parent.hir),
                index: Box::new(resolved_index.hir),
                element_type: ty,
            },
        })
    }
}

fn check_constant_index(index: Spanned<&Expression>, size: u32) -> ResolveResult<()> {
    let Expression::Literal(literal) = index.node else {
        return Ok(());
    };
    let Literal::Integer { value, .. } = &literal.node else {
        return Ok(());
    };
    match value.replace('_', "").parse::<u64>() {
        Ok(i) if i < u64::from(size) => Ok(()),
        _ => Err(ResolveError::InvalidMatrix {
            message: format!("index {value} is out of bounds for length {size}"),
        }
        .at(index.span)),
    }
}
