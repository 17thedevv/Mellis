use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessKind {
    None,
    Read,
    Write,
    ReadWrite,
    Unknown,
}

impl PartialOrd for AccessKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self == other {
            return Some(std::cmp::Ordering::Equal);
        }
        match (self, other) {
            (AccessKind::None, _) => Some(std::cmp::Ordering::Less),
            (_, AccessKind::Unknown) => Some(std::cmp::Ordering::Less),
            (AccessKind::Unknown, _) => Some(std::cmp::Ordering::Greater),
            (_, AccessKind::None) => Some(std::cmp::Ordering::Greater),
            (AccessKind::Read, AccessKind::ReadWrite) => Some(std::cmp::Ordering::Less),
            (AccessKind::Write, AccessKind::ReadWrite) => Some(std::cmp::Ordering::Less),
            (AccessKind::ReadWrite, AccessKind::Read) => Some(std::cmp::Ordering::Greater),
            (AccessKind::ReadWrite, AccessKind::Write) => Some(std::cmp::Ordering::Greater),
            _ => None,
        }
    }
}

impl AccessKind {
    pub fn merge(&self, other: &Self) -> Self {
        match (self, other) {
            (AccessKind::Unknown, _) | (_, AccessKind::Unknown) => AccessKind::Unknown,
            (AccessKind::ReadWrite, _) | (_, AccessKind::ReadWrite) => AccessKind::ReadWrite,
            (AccessKind::Read, AccessKind::Write) | (AccessKind::Write, AccessKind::Read) => {
                AccessKind::ReadWrite
            }
            (AccessKind::Read, _) | (_, AccessKind::Read) => AccessKind::Read,
            (AccessKind::Write, _) | (_, AccessKind::Write) => AccessKind::Write,
            (AccessKind::None, AccessKind::None) => AccessKind::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnershipKind {
    Copy,
    BorrowShared,
    BorrowMut,
    Consume,
    Unknown,
}

impl OwnershipKind {
    pub fn merge(&self, other: &Self) -> Self {
        std::cmp::max(self, other).clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EscapeKind {
    NoEscape,
    CallOnly,
    MayEscape,
    Unknown,
}

impl EscapeKind {
    pub fn merge(&self, other: &Self) -> Self {
        std::cmp::max(self, other).clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnEffect {
    Independent,
    BorrowsFrom(Vec<usize>), // Arg indices
    BorrowsCarried(Vec<usize>),
    Unknown,
}

impl PartialOrd for ReturnEffect {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self == other {
            return Some(std::cmp::Ordering::Equal);
        }
        match (self, other) {
            (ReturnEffect::Independent, _) => Some(std::cmp::Ordering::Less),
            (_, ReturnEffect::Independent) => Some(std::cmp::Ordering::Greater),
            (ReturnEffect::Unknown, _) => Some(std::cmp::Ordering::Greater),
            (_, ReturnEffect::Unknown) => Some(std::cmp::Ordering::Less),
            // Partial ordering between BorrowsFrom and BorrowsCarried is complex,
            // we will treat them as incomparable if they mix, but we can compare same types
            (ReturnEffect::BorrowsFrom(a), ReturnEffect::BorrowsFrom(b)) |
            (ReturnEffect::BorrowsCarried(a), ReturnEffect::BorrowsCarried(b)) => {
                let a_is_subset = a.iter().all(|x| b.contains(x));
                let b_is_subset = b.iter().all(|x| a.contains(x));
                if a_is_subset && b_is_subset {
                    Some(std::cmp::Ordering::Equal)
                } else if a_is_subset {
                    Some(std::cmp::Ordering::Less)
                } else if b_is_subset {
                    Some(std::cmp::Ordering::Greater)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl ReturnEffect {
    pub fn merge(&self, other: &Self) -> Self {
        match (self, other) {
            (ReturnEffect::Unknown, _) | (_, ReturnEffect::Unknown) => ReturnEffect::Unknown,
            (ReturnEffect::BorrowsFrom(a), ReturnEffect::BorrowsFrom(b)) => {
                let mut merged = a.clone();
                for &idx in b {
                    if !merged.contains(&idx) {
                        merged.push(idx);
                    }
                }
                merged.sort();
                ReturnEffect::BorrowsFrom(merged)
            }
            (ReturnEffect::BorrowsCarried(a), ReturnEffect::BorrowsCarried(b)) => {
                let mut merged = a.clone();
                for &idx in b {
                    if !merged.contains(&idx) {
                        merged.push(idx);
                    }
                }
                merged.sort();
                ReturnEffect::BorrowsCarried(merged)
            }
            // If they mix, we could upgrade to a combined effect, but for now we fallback to Unknown
            // or just take BorrowsFrom which is more conservative (since it borrows the object itself)
            (ReturnEffect::BorrowsFrom(_), ReturnEffect::BorrowsCarried(_)) |
            (ReturnEffect::BorrowsCarried(_), ReturnEffect::BorrowsFrom(_)) => {
                // If a function returns something that borrows BOTH from the argument and what it carries,
                // borrowing from the argument is strictly more restrictive.
                if let ReturnEffect::BorrowsFrom(_) = self {
                    self.clone()
                } else {
                    other.clone()
                }
            }
            (ReturnEffect::BorrowsFrom(a), ReturnEffect::Independent) => {
                ReturnEffect::BorrowsFrom(a.clone())
            }
            (ReturnEffect::Independent, ReturnEffect::BorrowsFrom(b)) => {
                ReturnEffect::BorrowsFrom(b.clone())
            }
            (ReturnEffect::BorrowsCarried(a), ReturnEffect::Independent) => {
                ReturnEffect::BorrowsCarried(a.clone())
            }
            (ReturnEffect::Independent, ReturnEffect::BorrowsCarried(b)) => {
                ReturnEffect::BorrowsCarried(b.clone())
            }
            (ReturnEffect::Independent, ReturnEffect::Independent) => ReturnEffect::Independent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgEffect {
    pub access: AccessKind,
    pub ownership: OwnershipKind,
    pub escape: EscapeKind,
}

impl PartialOrd for ArgEffect {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let access_cmp = self.access.partial_cmp(&other.access);
        let ownership_cmp = self.ownership.partial_cmp(&other.ownership);
        let escape_cmp = self.escape.partial_cmp(&other.escape);

        let cmps = [access_cmp, ownership_cmp, escape_cmp];

        let mut has_less = false;
        let mut has_greater = false;

        for cmp in cmps {
            match cmp {
                Some(std::cmp::Ordering::Less) => has_less = true,
                Some(std::cmp::Ordering::Greater) => has_greater = true,
                None => return None,
                _ => {}
            }
        }

        if has_less && has_greater {
            None
        } else if has_less {
            Some(std::cmp::Ordering::Less)
        } else if has_greater {
            Some(std::cmp::Ordering::Greater)
        } else {
            Some(std::cmp::Ordering::Equal)
        }
    }
}

impl ArgEffect {
    pub fn default() -> Self {
        Self {
            access: AccessKind::None,
            ownership: OwnershipKind::Copy,
            escape: EscapeKind::CallOnly,
        }
    }

    pub fn worst_case() -> Self {
        Self {
            access: AccessKind::Unknown,
            ownership: OwnershipKind::Unknown,
            escape: EscapeKind::Unknown,
        }
    }

    pub fn merge(&self, other: &Self) -> Self {
        Self {
            access: self.access.merge(&other.access),
            ownership: self.ownership.merge(&other.ownership),
            escape: self.escape.merge(&other.escape),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEffectSummary {
    pub args: Vec<ArgEffect>,
    pub ret: ReturnEffect,
    pub is_opaque: bool,
}

impl PartialOrd for CallEffectSummary {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.args.len() != other.args.len() {
            return None;
        }

        let mut has_less = false;
        let mut has_greater = false;

        for (a, b) in self.args.iter().zip(other.args.iter()) {
            match a.partial_cmp(b) {
                Some(std::cmp::Ordering::Less) => has_less = true,
                Some(std::cmp::Ordering::Greater) => has_greater = true,
                None => return None,
                _ => {}
            }
        }

        match self.ret.partial_cmp(&other.ret) {
            Some(std::cmp::Ordering::Less) => has_less = true,
            Some(std::cmp::Ordering::Greater) => has_greater = true,
            None => return None,
            _ => {}
        }

        if has_less && has_greater {
            None
        } else if has_less {
            Some(std::cmp::Ordering::Less)
        } else if has_greater {
            Some(std::cmp::Ordering::Greater)
        } else {
            Some(std::cmp::Ordering::Equal)
        }
    }
}

impl Default for CallEffectSummary {
    fn default() -> Self {
        Self {
            args: Vec::new(),
            ret: ReturnEffect::Independent,
            is_opaque: false,
        }
    }
}

impl CallEffectSummary {
    pub fn default_for_args(num_args: usize) -> Self {
        Self {
            args: vec![ArgEffect::default(); num_args],
            ret: ReturnEffect::Independent,
            is_opaque: false,
        }
    }

    pub fn worst_case(num_args: usize) -> Self {
        Self {
            args: vec![ArgEffect::worst_case(); num_args],
            ret: ReturnEffect::Unknown,
            is_opaque: true,
        }
    }
}
