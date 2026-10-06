use alloc::{vec, vec::Vec};
use musubu_hir::HIRMatchPattern as Pattern;
use musubu_primitive::PrimitiveType;

pub(super) fn useful(previous: &[Pattern], candidate: &Pattern, ty: &PrimitiveType) -> bool {
    let rows = previous.iter().map(|p| vec![p.clone()]).collect::<Vec<_>>();
    useful_row(&rows, &[candidate.clone()], &[ty.clone()])
}

// Specialize each enum constructor into its payload columns. A candidate is
// useful when at least one combination remains uncovered by earlier rows.
fn useful_row(matrix: &[Vec<Pattern>], row: &[Pattern], types: &[PrimitiveType]) -> bool {
    // An earlier row of bindings/wildcards already covers every remaining value.
    if matrix
        .iter()
        .any(|row| row.iter().all(|p| !matches!(p, Pattern::Variant { .. })))
    {
        return false;
    }
    if row.is_empty() {
        return matrix.is_empty();
    }
    if let PrimitiveType::NamedEnum { variants, .. } = &types[0] {
        for (index, variant) in variants.iter().enumerate() {
            let Some(candidate) = specialize(row, index, variant.fields.len()) else {
                continue;
            };
            let specialized = matrix
                .iter()
                .filter_map(|row| specialize(row, index, variant.fields.len()))
                .collect::<Vec<_>>();
            let mut next_types = variant
                .fields
                .iter()
                .map(|(_, ty)| ty.clone())
                .collect::<Vec<_>>();
            next_types.extend_from_slice(&types[1..]);
            if useful_row(&specialized, &candidate, &next_types) {
                return true;
            }
        }
        false
    } else {
        let next = matrix
            .iter()
            .map(|row| row[1..].to_vec())
            .collect::<Vec<_>>();
        useful_row(&next, &row[1..], &types[1..])
    }
}

fn specialize(row: &[Pattern], index: usize, count: usize) -> Option<Vec<Pattern>> {
    let mut payload = vec![Pattern::Wildcard; count];
    if let Pattern::Variant {
        index: found,
        fields,
    } = &row[0]
    {
        if *found != index {
            return None;
        }
        for (i, pattern) in fields {
            payload[*i] = pattern.clone();
        }
    }
    payload.extend_from_slice(&row[1..]);
    Some(payload)
}
