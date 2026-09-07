pub mod diagnostic;
pub mod ids;
pub mod interner;
pub mod source;

pub use diagnostic::{Diagnostic, DiagnosticLevel};
pub use ids::{FileId, Span, SymbolId, SyntaxContext};
pub use interner::StringInterner;
pub use source::{SourceFile, SourceManager};

pub struct CompilerSession {
    pub source_manager: SourceManager,
    pub interner: StringInterner,
    pub diagnostics: Vec<Diagnostic>,
    pub allow_internal_lang_items: bool,
}

impl CompilerSession {
    pub fn new() -> Self {
        Self {
            source_manager: SourceManager::new(),
            interner: StringInterner::new(),
            diagnostics: Vec::new(),
            allow_internal_lang_items: false,
        }
    }
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}
