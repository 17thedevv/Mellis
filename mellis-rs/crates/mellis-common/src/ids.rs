#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,
    pub end: u32,
}
