use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct SyntaxContext(pub u32);

impl SyntaxContext {
    pub const ROOT: Self = Self(0);

    pub fn is_root(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,
    pub end: u32,
    #[serde(default)]
    pub ctxt: SyntaxContext,
}

impl Span {
    pub fn new(file_id: FileId, start: u32, end: u32) -> Self {
        Self {
            file_id,
            start,
            end,
            ctxt: SyntaxContext::ROOT,
        }
    }

    pub fn with_ctxt(mut self, ctxt: SyntaxContext) -> Self {
        self.ctxt = ctxt;
        self
    }
}
