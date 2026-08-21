use crate::PatId;
use mellis_common::Span;
use mellis_lexer::Token;

#[derive(Debug, Clone)]
pub struct StructPatternField {
    pub name: Span,
    pub pattern: Option<PatId>,
}

#[derive(Debug, Clone)]
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
