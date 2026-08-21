use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibModule {
    pub functions: Vec<MlibFunction>,
    // In the future: structs, traits, globals
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlibFunction {
    pub name: String,
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
    Assign(MlibOperand),
    Store { ptr: u32, value: MlibOperand },
    Load { ptr: MlibOperand },
    Call { callee: MlibOperand, args: Vec<MlibOperand> },
    Add { left: MlibOperand, right: MlibOperand },
    Sub { left: MlibOperand, right: MlibOperand },
    Mul { left: MlibOperand, right: MlibOperand },
    Eq { left: MlibOperand, right: MlibOperand },
    Borrow { is_rw: bool, base: MlibOperand },
    Variant { enum_ty: u32, variant_idx: u32, args: Vec<MlibOperand> },
    Tag { value: MlibOperand },
    Extract { value: MlibOperand, variant_idx: u32, field_idx: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MlibTerminator {
    Br { target: u32 },
    CondBr { condition: MlibOperand, true_target: u32, false_target: u32 },
    Ret { value: Option<MlibOperand> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MlibOperand {
    Value(u32),
    Number(String),
    Boolean(bool),
    Block(u32),
    Global(String),
}
