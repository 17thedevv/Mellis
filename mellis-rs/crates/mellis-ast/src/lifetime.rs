//! Lifetime expression AST nodes for Mellis.
//!
//! This module defines the AST representation for lifetime expressions,
//! following the Implementation Contract specification.
//!
//! ## Syntax
//! ```mellis
//! life_from(a)              // origin from a
//! life_from(a | b | c)      // origin from any of them
//! where outlives(a, b)      // constraint: 'b ≤ 'a
//! ```

use serde::{Serialize, Deserialize};
use mellis_common::ids::Span;

/// A lifetime expression specifying the provenance of a reference.
///
/// This represents the `life_from(x)` syntax in function signatures.
///
/// # Examples
/// ```mellis
/// fn foo(x: &i32) -> &i32 life_from(x)
/// fn bar(a: &i32, b: &i32) -> &i32 life_from(a | b)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LifetimeExpr {
    /// `life_from(a)` — provenance from a single identifier
    Provenance(Span),
    /// `life_from(a | b | c)` — provenance from any of several identifiers
    ProvenanceSet(Vec<Span>),
}

impl LifetimeExpr {
    /// Returns the identifiers that this lifetime expression references.
    pub fn identifiers(&self) -> Vec<&Span> {
        match self {
            LifetimeExpr::Provenance(ident) => vec![ident],
            LifetimeExpr::ProvenanceSet(idents) => idents.iter().collect(),
        }
    }
}

/// A lifetime constraint clause.
///
/// This represents the `where outlives(a, b)` syntax, which generates
/// the constraint `'b ≤ 'a` (b outlives a, i.e., region(b) ⊆ region(a)).
///
/// # Examples
/// ```mellis
/// fn example(a: &i32, b: &i32) -> &i32
///     life_from(a)
///     where outlives(a, b)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LifetimeConstraint {
    /// The first identifier: `outlives(a, b)` means 'b ≤ 'a
    pub first: Span,
    /// The second identifier: `outlives(a, b)` means 'b ≤ 'a
    pub second: Span,
}

impl LifetimeConstraint {
    /// Creates a new outlives constraint: `outlives(first, second)` → `'second ≤ 'first`
    pub fn outlives(first: Span, second: Span) -> Self {
        Self { first, second }
    }
}

/// A function's lifetime signature combining provenance and constraints.
///
/// This is attached to function declarations that use explicit lifetime
/// annotations via `life_from` and `where outlives` clauses.
///
/// # Examples
/// ```mellis
/// fn foo(x: &i32) -> &i32 life_from(x)
/// fn bar(a: &i32, b: &i32) -> &i32
///     life_from(a | b)
///     where outlives(a, b)
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FnLifetimeSignature {
    /// The provenance expression (life_from)
    pub provenance: Option<LifetimeExpr>,
    /// Additional lifetime constraints (where outlives clauses)
    pub constraints: Vec<LifetimeConstraint>,
}

impl FnLifetimeSignature {
    /// Creates a new empty lifetime signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a signature with a single provenance.
    pub fn with_provenance(provenance: LifetimeExpr) -> Self {
        Self {
            provenance: Some(provenance),
            constraints: Vec::new(),
        }
    }

    /// Adds an outlives constraint.
    pub fn with_constraint(mut self, constraint: LifetimeConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        use mellis_common::ids::{FileId, Span};
        Span::new(FileId(0), 0, 0)
    }

    #[test]
    fn test_provenance_identifiers() {
        let single = LifetimeExpr::Provenance(dummy_span());
        assert_eq!(single.identifiers().len(), 1);

        let multi = LifetimeExpr::ProvenanceSet(vec![dummy_span(), dummy_span()]);
        assert_eq!(multi.identifiers().len(), 2);
    }

    #[test]
    fn test_constraint_creation() {
        let c = LifetimeConstraint::outlives(dummy_span(), dummy_span());
        assert_eq!(c.first.file_id.0, 0);
    }

    #[test]
    fn test_signature_builder() {
        let sig = FnLifetimeSignature::new();
        assert!(sig.provenance.is_none());
        assert!(sig.constraints.is_empty());

        let sig = FnLifetimeSignature::with_provenance(LifetimeExpr::Provenance(dummy_span()));
        assert!(sig.provenance.is_some());
        assert!(sig.constraints.is_empty());

        let sig = sig.with_constraint(LifetimeConstraint::outlives(dummy_span(), dummy_span()));
        assert_eq!(sig.constraints.len(), 1);
    }
}
