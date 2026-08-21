use mellis_common::ids::SymbolId;
use mellis_semantic::SemanticTypeId;

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
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    Alloca, // ty is kept in ValueData
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
    Eq {
        left: Operand,
        right: Operand,
    },
    Call {
        callee: Operand,
        args: Vec<Operand>,
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
}

#[derive(Clone, Debug)]
pub struct ValueData {
    pub inst: Instruction,
    pub ty: SemanticTypeId,
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
    pub blocks: Vec<BasicBlock>,
    pub values: Vec<ValueData>,
    pub ret_ty: SemanticTypeId,
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
