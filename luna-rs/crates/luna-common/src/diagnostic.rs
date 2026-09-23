use crate::ids::Span;
use crate::source::SourceManager;
use std::fmt;

/// Stable, machine-readable compiler diagnostic identity.
///
/// Numerical discriminants are frozen by `docs/diagnostics/diagnostics-v1.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum DiagnosticCode {
    ExpectedToken = 1,
    UnexpectedToken = 2,
    InvalidSyntax = 3,
    UnclosedDelimiter = 4,
    InvalidAnnotation = 5,
    UnresolvedSymbol = 1001,
    DuplicateDefinition = 1002,
    PrivateSymbolAccess = 1003,
    UnresolvedModuleProvider = 1004,
    CyclicModuleDependency = 1005,
    InvalidVisibility = 1006,
    WildcardImportProhibited = 1007,
    TypeMismatch = 2001,
    CannotDereference = 2002,
    CannotIndex = 2003,
    CopyDropConflict = 2004,
    ConflictingTraitImpl = 2005,
    OrphanImpl = 2006,
    UnresolvedTraitImpl = 2007,
    AssociatedTypeCycle = 2008,
    InfiniteSizeRecursiveType = 2009,
    NonObjectSafeTrait = 2010,
    InvalidLvalue = 2011,
    InvalidUnaryOp = 2012,
    InvalidBinaryOp = 2013,
    DuplicateField = 2014,
    DuplicateVariant = 2015,
    LifetimeConstraintViolation = 2016,
    MissingField = 2017,
    NoAssociatedType = 2018,
    AmbiguousAssociatedType = 2019,
    CannotCallNonFunction = 2020,
    TraitBoundNotSatisfied = 2021,
    NonExhaustivePattern = 2022,
    CannotMutateImmutable = 2023,
    InvalidMainSignature = 2024,
    UnsafeOperationOutsideUnsafe = 2025,
    InvalidCast = 2026,
    AwaitOutsideAsync = 2027,
    InvalidTryOperator = 2028,
    LoopControlOutsideLoop = 2029,
    NonFfiSafeType = 2030,
    UseAfterMove = 3001,
    PartialMoveUnderDrop = 3002,
    BorrowConflict = 3003,
    MissingReturnValue = 3004,
    LocalBorrowEscape = 3005,
    AsyncBorrowAcrossAwait = 4001,
    ComptimeStepLimitExceeded = 4002,
    ComptimeUnserializableEscape = 4003,
    ComptimePanic = 4004,
    ComptimeEvaluationFailed = 4005,
    MonomorphizationBarrier = 5001,
    InfiniteMonomorphizationRecursion = 5002,
    BackendInvariantViolation = 6001,
    ObjectEmissionFailure = 6002,
    LinkerFailure = 6003,
}

impl DiagnosticCode {
    pub const fn number(self) -> u16 {
        self as u16
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "E{:04}", self.number())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub code: Option<DiagnosticCode>,
    pub span: Option<Span>,
    pub message: String,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            code: None,
            span: None,
            message: message.into(),
        }
    }

    pub fn with_code(mut self, code: DiagnosticCode) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn render(&self, source_manager: &SourceManager) -> String {
        let mut out = String::new();

        let level_str = match self.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Note => "note",
        };

        if let (Some(span), Some(file)) = (
            self.span,
            self.span.and_then(|s| source_manager.get_file(s.file_id)),
        ) {
            let (line, col) = file.get_line_col(span.start);
            if let Some(code) = self.code {
                out.push_str(&format!("{}[{}]: {}\n", level_str, code, self.message));
            } else {
                out.push_str(&format!("{}: {}\n", level_str, self.message));
            }
            out.push_str(&format!("  --> {}:{}:{}\n", file.name, line, col));

            if let Some(line_str) = file.get_line_str(line) {
                out.push_str("   |\n");
                out.push_str(&format!("{:<3}| {}\n", line, line_str));

                let indent = " ".repeat((col - 1) as usize);
                let length = std::cmp::max(1, span.end.saturating_sub(span.start)) as usize;
                let carets = "^".repeat(length);
                out.push_str(&format!("   | {}{}\n", indent, carets));
            }
        } else {
            if let Some(code) = self.code {
                out.push_str(&format!("{}[{}]: {}\n", level_str, code, self.message));
            } else {
                out.push_str(&format!("{}: {}\n", level_str, self.message));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, DiagnosticCode};
    use crate::source::SourceManager;

    #[test]
    fn structured_code_renders_without_becoming_message_identity() {
        let diagnostic = Diagnostic::error("Use of moved value `value`")
            .with_code(DiagnosticCode::UseAfterMove);

        assert_eq!(diagnostic.code, Some(DiagnosticCode::UseAfterMove));
        assert_eq!(diagnostic.message, "Use of moved value `value`");
        assert_eq!(
            diagnostic.render(&SourceManager::new()),
            "error[E3001]: Use of moved value `value`\n"
        );
    }
}
