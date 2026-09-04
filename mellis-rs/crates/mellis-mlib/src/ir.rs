use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibCaptureInfo {
    pub symbol: u32,
    pub source: u32,
    pub env_field: u32,
    pub mode: u8,
    pub ty: u32,
    pub env_ty: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MlibModule {
    pub functions: Vec<MlibFunction>,
    pub strings: Vec<String>,
    pub types: Vec<MlibTypeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibTypeEntry {
    pub name: String,
    pub namespace_id: u32,
    pub size: u64,
    pub alignment: u64,
    pub visibility: u8,
    pub module_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibFunction {
    pub name: String,
    #[serde(default)]
    pub arg_count: u32,
    #[serde(default)]
    pub is_async: bool,
    pub values: Vec<MlibValue>,
    pub blocks: Vec<MlibBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibValue {
    pub id: u32,
    pub inst: MlibInstruction,
    // Add type information later if needed
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibBlock {
    pub id: u32,
    pub label: String,
    pub insts: Vec<u32>, // References to MlibValue IDs
    pub terminator: Option<MlibTerminator>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MlibInstruction {
    Alloca,
    HeapAlloc,
    Assign(MlibOperand),
    Store { ptr: u32, value: MlibOperand },
    Load { ptr: MlibOperand },
    CallDirect { callee: String, args: Vec<MlibOperand> },
    CallIndirect { callee: MlibOperand, args: Vec<MlibOperand> },
    CallClosure { closure: MlibOperand, args: Vec<MlibOperand> },
    MakeClosure { func: String, env_ptr: MlibOperand, captures: Vec<MlibCaptureInfo> },
    CallVirt { obj: MlibOperand, method_idx: u32, args: Vec<MlibOperand> },
    MakeTraitObject { data_ptr: MlibOperand, vtable: String, trait_sym: u32 },
    Add { left: MlibOperand, right: MlibOperand },
    Sub { left: MlibOperand, right: MlibOperand },
    Mul { left: MlibOperand, right: MlibOperand },
    Div { left: MlibOperand, right: MlibOperand },
    Rem { left: MlibOperand, right: MlibOperand },
    Eq {
        left: MlibOperand,
        right: MlibOperand,
    },
    NotEq {
        left: MlibOperand,
        right: MlibOperand,
    },
    LessThan {
        left: MlibOperand,
        right: MlibOperand,
    },
    LessOrEq {
        left: MlibOperand,
        right: MlibOperand,
    },
    GreaterThan {
        left: MlibOperand,
        right: MlibOperand,
    },
    GreaterOrEq {
        left: MlibOperand,
        right: MlibOperand,
    },
    BitAnd {
        left: MlibOperand,
        right: MlibOperand,
    },
    BitOr {
        left: MlibOperand,
        right: MlibOperand,
    },
    BitXor {
        left: MlibOperand,
        right: MlibOperand,
    },
    Shl {
        left: MlibOperand,
        right: MlibOperand,
    },
    Shr {
        left: MlibOperand,
        right: MlibOperand,
    },
    Borrow { is_rw: bool, base: MlibOperand },
    Variant { enum_ty: u32, variant_idx: u32, args: Vec<MlibOperand> },
    Tag { value: MlibOperand },
    Extract { value: MlibOperand, variant_idx: u32, field_idx: u32 },
    FieldPtr { base: MlibOperand, field_idx: u32 },
    Drop { value: MlibOperand },
    BoxNew { value: MlibOperand },
    BoxFree { value: MlibOperand },
    MarkInit { value: MlibOperand },
    SizeOf { ty: u32 },
    AlignOf { ty: u32 },
    Null { ty: u32 },
    Nop,
    Cast { value: MlibOperand, ty: u32 },
    PtrOffset { base: MlibOperand, offset: MlibOperand },
    ListNew,
    ListPush { list: MlibOperand, value: MlibOperand },
    ListGet { list: MlibOperand, index: MlibOperand, is_mut: bool },
    Await { future: MlibOperand },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MlibTerminator {
    Br { target: u32 },
    CondBr { condition: MlibOperand, true_target: u32, false_target: u32 },
    Ret { value: Option<MlibOperand> },
    Unreachable,
    MissingReturn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MlibOperand {
    Value(u32),
    Number(String),
    Boolean(bool),
    Block(u32),
    Global(String),
    StringRef(String),
    Char(String),
}
