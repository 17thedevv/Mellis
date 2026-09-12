use crate::PatId;
use luna_common::Span;
use luna_lexer::Token;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct StructPatternField {
    pub name: Span,
    pub pattern: Option<PatId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Pattern {
    Wildcard,
    Literal(Token),
    Identifier {
        segments: Vec<Span>,
    },
    Enum {
        path: Vec<Span>,
        fields: Vec<PatId>,
    },
    Tuple {
        elements: Vec<PatId>,
        has_rest: bool,
    },
    Struct {
        path: Vec<Span>,
        fields: Vec<StructPatternField>,
        has_rest: bool,
    },
}
