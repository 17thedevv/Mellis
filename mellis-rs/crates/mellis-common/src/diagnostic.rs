use crate::ids::Span;
use crate::source::SourceManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub span: Option<Span>,
    pub message: String,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            span: None,
            message: message.into(),
        }
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
            out.push_str(&format!("{}: {}\n", level_str, self.message));
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
            out.push_str(&format!("{}: {}\n", level_str, self.message));
        }

        out
    }
}
