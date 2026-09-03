use mellis_common::ids::SymbolId;
use mellis_semantic::{SemanticTypeId, semantic_tables::IntrinsicKind};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalId {
    pub name: String,
    pub symbol_id: Option<SymbolId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlobalId {
    pub name: String,
    pub symbol_id: Option<SymbolId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LabelId {
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Operand {
    Value(ValueId),
    Global(GlobalId),
    Block(BlockId),
    Number(String),
    Boolean(bool),
    StringRef(String),
    Char(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureInfo {
    pub symbol: mellis_common::ids::SymbolId,
    pub source: ValueId,
    pub env_field: u32,
    pub mode: mellis_semantic::CaptureMode,
    pub ty: SemanticTypeId,
    pub env_ty: SemanticTypeId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    Alloca, // ty is kept in ValueData
    HeapAlloc, // ty is kept in ValueData, dynamically allocates memory
    Assign(Operand), // For constant folding or aliases
    Store {
        ptr: Operand,
        value: Operand,
    },
    Load {
        ptr: Operand,
    },
    Add {
        left: Operand,
        right: Operand,
    },
    Sub {
        left: Operand,
        right: Operand,
    },
    Mul {
        left: Operand,
        right: Operand,
    },
    Div {
        left: Operand,
        right: Operand,
    },
    Rem {
        left: Operand,
        right: Operand,
    },
    Eq {
        left: Operand,
        right: Operand,
    },
    NotEq {
        left: Operand,
        right: Operand,
    },
    LessThan {
        left: Operand,
        right: Operand,
    },
    LessOrEq {
        left: Operand,
        right: Operand,
    },
    GreaterThan {
        left: Operand,
        right: Operand,
    },
    GreaterOrEq {
        left: Operand,
        right: Operand,
    },
    BitAnd {
        left: Operand,
        right: Operand,
    },
    BitOr {
        left: Operand,
        right: Operand,
    },
    BitXor {
        left: Operand,
        right: Operand,
    },
    Shl {
        left: Operand,
        right: Operand,
    },
    Shr {
        left: Operand,
        right: Operand,
    },
    CallDirect {
        callee: GlobalId,
        args: Vec<Operand>,
    },
    CallIndirect {
        callee: Operand,
        args: Vec<Operand>,
    },
    CallClosure {
        closure: Operand,
        args: Vec<Operand>,
    },
    CallIntrinsic {
        kind: IntrinsicKind,
        args: Vec<Operand>,
    },
    MakeClosure {
        func: GlobalId,
        env_ptr: Operand,
        captures: Vec<CaptureInfo>,
    },
    CallVirt {
        obj: Operand,
        method_idx: u32,
        args: Vec<Operand>,
    },
    MakeTraitObject {
        data_ptr: Operand,
        vtable: GlobalId,
        trait_sym: SymbolId,
    },
    BoundsCheck {
        index: Operand,
        len: Operand,
    },
    Borrow {
        is_rw: bool,
        base: Operand,
    },
    Variant {
        enum_ty: SemanticTypeId,
        variant_idx: u32,
        args: Vec<Operand>,
    },
    Tag {
        value: Operand,
    },
    Extract {
        value: Operand,
        variant_idx: u32,
        field_idx: u32,
    },
    FieldPtr {
        base: Operand,
        field_idx: u32,
    },
    BoxNew {
        value: Operand,
    },
    BoxFree {
        value: Operand,
    },
    MarkInit {
        value: Operand,
    },
    Drop {
        value: Operand,
        ty: mellis_semantic::SemanticTypeId,
        callee: Option<GlobalId>,
    },
    PtrOffset {
        ptr: Operand,
        offset: Operand,
    },
    Cast {
        value: Operand,
        target_ty: SemanticTypeId,
    },
    SizeOf {
        ty: SemanticTypeId,
    },
    AlignOf {
        ty: SemanticTypeId,
    },
    Null {
        ty: SemanticTypeId,
    },
    Await {
        future: Operand,
    },
}

#[derive(Clone, Debug)]
pub struct ValueData {
    pub inst: Instruction,
    pub ty: SemanticTypeId,
    pub span: Option<mellis_common::ids::Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminator {
    Ret {
        value: Option<Operand>,
    },
    Br {
        target: LabelId,
    },
    CondBr {
        condition: Operand,
        true_target: LabelId,
        false_target: LabelId,
    },
    Unreachable,
    MissingReturn,
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub label: LabelId,
    pub insts: Vec<ValueId>,
    pub terminator: Option<Terminator>,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: GlobalId,
    pub is_extern: bool,
    pub is_async: bool,
    pub arg_count: usize,
    pub link_name: Option<String>,
    pub param_types: Vec<SemanticTypeId>,
    pub ret_ty: SemanticTypeId,
    pub blocks: Vec<BasicBlock>,
    pub values: Vec<ValueData>,
}

impl Function {
    pub fn value(&self, id: ValueId) -> &ValueData {
        &self.values[id.0 as usize]
    }
    pub fn block(&self, id: BlockId) -> &BasicBlock {
        &self.blocks[id.0 as usize]
    }
}

#[derive(Clone, Debug)]
pub struct Module {
    pub functions: Vec<Function>,
}

impl Module {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
        }
    }
}
