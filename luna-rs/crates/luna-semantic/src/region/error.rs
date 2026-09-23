//! Generation errors for Luna Region Constraint Generation (REGION-01B).

use super::graph::RegionGraphError;
use super::subject::LifetimeSubject;

/// Errors that can occur during constraint generation from contracts or body structures.
///
/// # Boundary Invariant
/// `RegionGenError` strictly captures structural generation and resolution failures.
/// It does NOT represent solver contradictions, unsatisfiability, or lifetime expiry,
/// which belong strictly to REGION-01C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionGenError {
    /// A subject referenced in a contract has no bound RegionId.
    UnboundLifetimeSubject(LifetimeSubject),

    /// Expected a return region binding for a contract targeting Return.
    MissingReturnRegion,

    /// A subject was used in an invalid context.
    InvalidContractSubject(LifetimeSubject),

    /// A subject was bound more than once in the same binding context.
    DuplicateLifetimeBinding(LifetimeSubject),

    /// A serialized or canonical contract was malformed.
    MalformedCanonicalContract(String),

    /// Underlying graph structural error.
    Graph(RegionGraphError),
}

impl From<RegionGraphError> for RegionGenError {
    fn from(err: RegionGraphError) -> Self {
        Self::Graph(err)
    }
}

impl std::fmt::Display for RegionGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnboundLifetimeSubject(subject) => {
                write!(f, "Unbound lifetime subject: {:?}", subject)
            }
            Self::MissingReturnRegion => {
                write!(f, "Missing return region in contract binding")
            }
            Self::InvalidContractSubject(subject) => {
                write!(f, "Invalid contract subject: {:?}", subject)
            }
            Self::DuplicateLifetimeBinding(subject) => {
                write!(f, "Duplicate lifetime binding for subject: {:?}", subject)
            }
            Self::MalformedCanonicalContract(msg) => {
                write!(f, "Malformed canonical contract: {}", msg)
            }
            Self::Graph(err) => write!(f, "Region graph error: {}", err),
        }
    }
}

impl std::error::Error for RegionGenError {}
