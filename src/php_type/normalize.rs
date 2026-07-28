//! Union/intersection simplification and normalization.

use super::*;

impl PhpType {
    // -----------------------------------------------------------------------
    // Union / intersection simplification
    // -----------------------------------------------------------------------

    /// Return a simplified copy of this type.
    ///
    /// Applies the following normalisations recursively:
    ///
    /// - Deduplicates union and intersection members.
    /// - `true | false` → `bool` (in either order, including with
    ///   extra members).
    /// - Unions containing `mixed` collapse to `mixed`.
    /// - Unions containing both `T` and `null` where `T` is a single
    ///   type collapse to `?T`.
    /// - Scalar refinement absorption: `positive-int | int` → `int`,
    ///   `non-empty-string | string` → `string`, etc.
    /// - Single-member unions/intersections are unwrapped.
    /// - `?T` where `T` is `never` simplifies to `null`.
    /// - Nested unions are flattened (`(A|B)|C` → `A|B|C`).
    /// - Nested intersections are flattened (`(A&B)&C` → `A&B&C`).
    pub fn simplified(&self) -> PhpType {
        match self.kind() {
            TypeKind::Union(members) => {
                // Recursively simplify members first.
                let mut simplified: Vec<PhpType> = Vec::with_capacity(members.len());
                for m in members {
                    let s = m.simplified();
                    // Flatten nested unions.
                    if let TypeKind::Union(inner) = s.kind() {
                        simplified.extend(inner.iter().cloned());
                    } else {
                        simplified.push(s);
                    }
                }

                // If any member is `mixed`, the whole union is `mixed`.
                if simplified.iter().any(|m| m.is_mixed()) {
                    return PhpType::mixed();
                }

                // Deduplicate (by Display form for simplicity).
                dedup_types(&mut simplified);

                // `true | false` → `bool`.
                simplify_bool_union(&mut simplified);

                // Scalar refinement absorption.
                absorb_scalar_refinements(&mut simplified);

                // Unwrap single-member union.
                if simplified.len() == 1 {
                    return simplified.into_iter().next().unwrap();
                }
                if simplified.is_empty() {
                    return PhpType::never();
                }

                PhpType::union(simplified)
            }
            TypeKind::Intersection(members) => {
                let mut simplified: Vec<PhpType> = Vec::with_capacity(members.len());
                for m in members {
                    let s = m.simplified();
                    // Flatten nested intersections.
                    if let TypeKind::Intersection(inner) = s.kind() {
                        simplified.extend(inner.iter().cloned());
                    } else {
                        simplified.push(s);
                    }
                }

                dedup_types(&mut simplified);

                // If any member is `never`, the intersection is `never`.
                if simplified.iter().any(|m| m.is_never()) {
                    return PhpType::never();
                }

                if simplified.len() == 1 {
                    return simplified.into_iter().next().unwrap();
                }
                if simplified.is_empty() {
                    return PhpType::mixed();
                }

                PhpType::intersection(simplified)
            }
            TypeKind::Nullable(inner) => {
                let s = inner.simplified();
                if s.is_never() || s.is_null() {
                    PhpType::null()
                } else if s.is_mixed() {
                    PhpType::mixed()
                } else {
                    PhpType::nullable(s)
                }
            }
            TypeKind::Generic(g) => {
                let simplified_args: Vec<PhpType> = g.args.iter().map(|a| a.simplified()).collect();
                PhpType::generic_atom(g.name, simplified_args)
            }
            TypeKind::Array(inner) => PhpType::array_of(inner.simplified()),
            TypeKind::ClassString(inner) => {
                PhpType::class_string(inner.as_ref().map(|i| i.simplified()))
            }
            TypeKind::InterfaceString(inner) => {
                PhpType::interface_string(inner.as_ref().map(|i| i.simplified()))
            }
            TypeKind::KeyOf(inner) => PhpType::key_of(inner.simplified()),
            TypeKind::ValueOf(inner) => PhpType::value_of(inner.simplified()),
            // Leaf types are already simplified.
            _ => self.clone(),
        }
    }

    // -----------------------------------------------------------------------
    // Intersection distribution over unions
    // -----------------------------------------------------------------------

    /// Distribute intersections over unions.
    ///
    /// Transforms `(A|B) & C` into `(A&C) | (B&C)`, producing a
    /// union of intersections (disjunctive normal form for types).
    ///
    /// This is useful for type narrowing: when an intersection type
    /// contains union members, distributing lets each branch be
    /// checked independently.
    ///
    /// If the type is not an intersection containing unions, returns
    /// a clone unchanged. The result is also simplified.
    pub fn distribute_intersection(&self) -> PhpType {
        match self.kind() {
            TypeKind::Intersection(members) => {
                // Check if any member is a union.
                let has_union = members
                    .iter()
                    .any(|m| matches!(m.kind(), TypeKind::Union(_)));
                if !has_union {
                    return self.clone();
                }

                // Collect each member as a list of alternatives.
                // Non-union members are singleton lists.
                let alternatives: Vec<Vec<PhpType>> = members
                    .iter()
                    .map(|m| match m.kind() {
                        TypeKind::Union(u) => u.to_vec(),
                        _ => vec![m.clone()],
                    })
                    .collect();

                // Compute the cartesian product to produce union members.
                let mut product: Vec<Vec<PhpType>> = vec![vec![]];
                for alt_set in &alternatives {
                    let mut new_product = Vec::with_capacity(product.len() * alt_set.len());
                    for existing in &product {
                        for alt in alt_set {
                            let mut combo = existing.clone();
                            combo.push(alt.clone());
                            new_product.push(combo);
                        }
                    }
                    product = new_product;
                }

                // Each product element becomes an intersection.
                let union_members: Vec<PhpType> = product
                    .into_iter()
                    .map(|combo| {
                        if combo.len() == 1 {
                            combo.into_iter().next().unwrap()
                        } else {
                            PhpType::intersection(combo)
                        }
                    })
                    .collect();

                if union_members.len() == 1 {
                    union_members.into_iter().next().unwrap().simplified()
                } else {
                    PhpType::union(union_members).simplified()
                }
            }
            _ => self.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Simplification helpers (private)
// ---------------------------------------------------------------------------

/// Deduplicate types in a vector by their `Display` form.
pub(crate) fn dedup_types(types: &mut Vec<PhpType>) {
    let mut seen = std::collections::HashSet::new();
    types.retain(|t| {
        let key = t.to_string().to_ascii_lowercase();
        seen.insert(key)
    });
}

/// If a union contains both `true` and `false`, replace them with `bool`.
pub(crate) fn simplify_bool_union(types: &mut Vec<PhpType>) {
    let has_true = types
        .iter()
        .any(|t| matches!(t.kind(), TypeKind::Named(s) if s.eq_ignore_ascii_case("true")));
    let has_false = types
        .iter()
        .any(|t| matches!(t.kind(), TypeKind::Named(s) if s.eq_ignore_ascii_case("false")));

    if has_true && has_false {
        types.retain(|t| {
            !matches!(t.kind(), TypeKind::Named(s)
                if matches!(s.to_ascii_lowercase().as_str(), "true" | "false"))
        });
        types.push(PhpType::bool());
    }
}

/// Absorb scalar refinements into their parent types.
///
/// When a union contains both a refinement and its parent (e.g.
/// `positive-int | int`), the refinement is redundant and removed.
pub(crate) fn absorb_scalar_refinements(types: &mut Vec<PhpType>) {
    // Collect the named types present.
    let named_set: std::collections::HashSet<String> = types
        .iter()
        .filter_map(|t| {
            if let TypeKind::Named(s) = t.kind() {
                Some(s.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect();

    if named_set.is_empty() {
        return;
    }

    types.retain(|t| {
        if let TypeKind::Named(s) = t.kind() {
            let lower = s.to_ascii_lowercase();
            // Check if any OTHER type in the set is a proper supertype.
            for sup in &named_set {
                if sup != &lower && is_named_subtype(&lower, sup) {
                    return false; // Remove: absorbed by the supertype.
                }
            }
        }
        true
    });
}
