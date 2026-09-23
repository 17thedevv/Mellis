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
use luna_common::ids::Span;

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

/// A lifetime target in an explicit outlives constraint or projection.
///
/// Syntax examples:
/// - `life(a)` -> `LifetimeTargetAst::Named { name: "a", span }`
/// - `life(self)` -> `LifetimeTargetAst::SelfVal(span)`
/// - `life(return)` -> `LifetimeTargetAst::Return(span)`
/// - `life(holder.field)` -> `LifetimeTargetAst::Projection { base, field: "field", span }`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LifetimeTargetAst {
    Named { name: String, span: Span },
    SelfVal(Span),
    Return(Span),
    Projection {
        base: Box<LifetimeTargetAst>,
        field: String,
        span: Span,
    },
}

impl LifetimeTargetAst {
    pub fn span(&self) -> Span {
        match self {
            LifetimeTargetAst::Named { span, .. } => *span,
            LifetimeTargetAst::SelfVal(span) => *span,
            LifetimeTargetAst::Return(span) => *span,
            LifetimeTargetAst::Projection { span, .. } => *span,
        }
    }
}

impl std::fmt::Display for LifetimeTargetAst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifetimeTargetAst::Named { name, .. } => write!(f, "{}", name),
            LifetimeTargetAst::SelfVal(_) => write!(f, "self"),
            LifetimeTargetAst::Return(_) => write!(f, "return"),
            LifetimeTargetAst::Projection { base, field, .. } => write!(f, "{}.{}", base, field),
        }
    }
}

/// A lifetime constraint clause.
///
/// Canonical syntax:
/// ```mellis
/// requires life(longer) >= life(shorter)
/// requires life(shorter) <= life(longer)
/// ```
/// Both forms normalize to `longer` outliving `shorter` (i.e. region(shorter) ⊆ region(longer)).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LifetimeConstraintAst {
    pub longer: LifetimeTargetAst,
    pub shorter: LifetimeTargetAst,
    pub span: Span,
}

impl LifetimeConstraintAst {
    pub fn new(longer: LifetimeTargetAst, shorter: LifetimeTargetAst, span: Span) -> Self {
        Self { longer, shorter, span }
    }
}

pub type LifetimeConstraint = LifetimeConstraintAst;

/// A function's lifetime signature combining provenance and constraints.
///
/// This is attached to function declarations that use explicit lifetime
/// annotations via `life_from` and `requires` clauses.
///
/// # Examples
/// ```mellis
/// fn foo(x: &i32) -> &i32 life_from(x)
/// fn bar(a: &i32, b: &i32) -> &i32
///     life_from(a | b)
///     requires life(a) >= life(b)
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FnLifetimeSignature {
    /// The provenance expression (life_from)
    pub provenance: Option<LifetimeExpr>,
    /// Additional lifetime constraints (requires clauses)
    pub constraints: Vec<LifetimeConstraintAst>,
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
    pub fn with_constraint(mut self, constraint: LifetimeConstraintAst) -> Self {
        self.constraints.push(constraint);
        self
    }
}

/// A struct's lifetime contract containing postfix outlives constraints.
///
/// Canonical syntax:
/// ```mellis
/// struct Holder {
///     value: &T,
/// } requires life(value) >= life(self);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StructLifetimeContractAst {
    pub constraints: Vec<LifetimeConstraintAst>,
}

impl StructLifetimeContractAst {
    pub fn new(constraints: Vec<LifetimeConstraintAst>) -> Self {
        Self { constraints }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        use luna_common::ids::{FileId, Span};
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
        let c = LifetimeConstraintAst::new(
            LifetimeTargetAst::Named { name: "a".into(), span: dummy_span() },
            LifetimeTargetAst::Named { name: "b".into(), span: dummy_span() },
            dummy_span(),
        );
        assert_eq!(c.span.file_id.0, 0);
        assert_eq!(c.longer.to_string(), "a");
        assert_eq!(c.shorter.to_string(), "b");
    }

    #[test]
    fn test_signature_builder() {
        let sig = FnLifetimeSignature::new();
        assert!(sig.provenance.is_none());
        assert!(sig.constraints.is_empty());

        let sig = FnLifetimeSignature::with_provenance(LifetimeExpr::Provenance(dummy_span()));
        assert!(sig.provenance.is_some());
        assert!(sig.constraints.is_empty());

        let sig = sig.with_constraint(LifetimeConstraintAst::new(
            LifetimeTargetAst::Named { name: "a".into(), span: dummy_span() },
            LifetimeTargetAst::Named { name: "b".into(), span: dummy_span() },
            dummy_span(),
        ));
        assert_eq!(sig.constraints.len(), 1);
    }
}
