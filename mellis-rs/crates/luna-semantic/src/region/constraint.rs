//! Constraints and relation metadata for Luna Region IR.

use luna_common::Span;
use serde::{Deserialize, Serialize};
use super::id::{ConstraintId, RegionId};

/// Canonical constraint kind in the Region IR graph.
///
/// # Normalization Boundary
/// The canonical solver graph only requires `Outlives` relations.
/// Surface relations such as `Equality` are normalized into bidirectional
/// `Outlives` relations (`a ⪰ b` and `b ⪰ a`).
/// High-level constructs like `Binding`, `Projection`, and `life_from` require AST and
/// type context and are normalized into canonical relations during REGION-01B
/// constraint generation before reaching the solver graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintKind {
    /// `sup ⪰ sub` (sup outlives sub / sub requires at most what sup provides).
    Outlives {
        sup: RegionId,
        sub: RegionId,
    },
}

/// The semantic rule that generated the lifetime constraint.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintRule {
    /// Explicit contract on function or type (`requires life(a) >= life(b)`).
    ExplicitContract,

    /// Explicit `life_from(...)` contract at return or boundary.
    LifeFrom,

    /// Automatic Luna Lifetime Elision (LLE).
    Elision,

    /// Reference borrowing operation (`&x` or `&rw x`).
    Borrow,

    /// Closure environment capturing variables.
    ClosureCapture,

    /// Structural composite field relationship.
    Structural,

    /// Internal compiler-synthesized constraint.
    Internal,
}

/// The transport mechanism through which this constraint arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintTransport {
    /// Intra-crate source code.
    Source,

    /// Precompiled artifact (`.llib` / `.mlib`) interface contract.
    Artifact,

    /// Compiler internal synthetic generation.
    Synthesized,
}

/// The trust / interface boundary where the constraint applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintBoundary {
    /// Standard language safe boundary.
    Normal,

    /// Foreign Function Interface (FFI) boundary.
    Ffi,
}

/// Complete provenance and semantic metadata of a lifetime constraint.
///
/// Preserves Rule, Transport, and Boundary independently so that diagnostic
/// explanation (REGION-01C) can distinguish e.g.:
/// - rule: LifeFrom
/// - transport: Artifact
/// - boundary: Ffi
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConstraintOrigin {
    /// The semantic rule that produced this constraint.
    pub rule: ConstraintRule,
    /// The transport mechanism (Source, Artifact, Synthesized).
    pub transport: ConstraintTransport,
    /// The boundary kind (Normal, Ffi).
    pub boundary: ConstraintBoundary,
}

impl ConstraintOrigin {
    /// Constructs a new constraint origin with the given rule, transport, and boundary.
    pub const fn new(
        rule: ConstraintRule,
        transport: ConstraintTransport,
        boundary: ConstraintBoundary,
    ) -> Self {
        Self {
            rule,
            transport,
            boundary,
        }
    }

    /// Explicit contract origin.
    pub const fn explicit_contract(
        transport: ConstraintTransport,
        boundary: ConstraintBoundary,
    ) -> Self {
        Self {
            rule: ConstraintRule::ExplicitContract,
            transport,
            boundary,
        }
    }

    /// `life_from` contract origin.
    pub const fn life_from(
        transport: ConstraintTransport,
        boundary: ConstraintBoundary,
    ) -> Self {
        Self {
            rule: ConstraintRule::LifeFrom,
            transport,
            boundary,
        }
    }

    /// Automatic elision origin.
    pub const fn elision() -> Self {
        Self {
            rule: ConstraintRule::Elision,
            transport: ConstraintTransport::Source,
            boundary: ConstraintBoundary::Normal,
        }
    }

    /// Reference borrow origin.
    pub const fn borrow() -> Self {
        Self {
            rule: ConstraintRule::Borrow,
            transport: ConstraintTransport::Source,
            boundary: ConstraintBoundary::Normal,
        }
    }

    /// Closure environment capture origin.
    pub const fn closure_capture() -> Self {
        Self {
            rule: ConstraintRule::ClosureCapture,
            transport: ConstraintTransport::Source,
            boundary: ConstraintBoundary::Normal,
        }
    }

    /// Composite structural origin.
    pub const fn structural() -> Self {
        Self {
            rule: ConstraintRule::Structural,
            transport: ConstraintTransport::Source,
            boundary: ConstraintBoundary::Normal,
        }
    }

    /// Compiler internal origin.
    pub const fn internal() -> Self {
        Self {
            rule: ConstraintRule::Internal,
            transport: ConstraintTransport::Synthesized,
            boundary: ConstraintBoundary::Normal,
        }
    }
}



/// A fully qualified constraint stored in the `RegionGraph`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionConstraint {
    /// Unique identifier for this constraint within the graph.
    pub id: ConstraintId,
    /// The canonical constraint relation.
    pub kind: ConstraintKind,
    /// Semantic origin for error reporting and diagnostics.
    pub origin: ConstraintOrigin,
    /// Optional source code location for precision diagnostics.
    pub span: Option<Span>,
}

impl RegionConstraint {
    /// Creates a new region constraint.
    pub fn new(
        id: ConstraintId,
        kind: ConstraintKind,
        origin: ConstraintOrigin,
        span: Option<Span>,
    ) -> Self {
        Self {
            id,
            kind,
            origin,
            span,
        }
    }
}
