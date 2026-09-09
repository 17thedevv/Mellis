use std::collections::HashMap;
use mellis_common::ids::{SymbolId, Span};
use mellis_semantic::{
    ComptimeValue, ComptimeError, IntWidth, FloatWidth,
    SemanticContext, SemanticTypeId, SemanticType,
    effect::Effect,
};
use crate::mvir::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Address {
    Stack { frame_idx: usize, slot_idx: usize, field_idx: Option<u32>, offset: isize },
    Heap { alloc_id: usize, field_idx: Option<u32>, offset: isize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceState {
    Uninitialized,
    Initialized,
    Moved,
}

#[derive(Debug, Clone)]
pub struct MemorySlot {
    pub value: RuntimeValue,
    pub state: PlaceState,
    pub ty: SemanticTypeId,
}

#[derive(Debug, Clone)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int { val: i128, width: IntWidth },
    Float { val: f64, width: FloatWidth },
    Pointer(Address),
    NullPointer,
    Compound(Vec<RuntimeValue>),
    Variant { enum_ty: SemanticTypeId, tag: u32, payload: Vec<RuntimeValue> },
    Closure { func: GlobalId, env_ptr: Address },
    TraitObject {
        data: Box<RuntimeValue>,
        vtable: GlobalId,
        trait_sym: SymbolId,
        concrete_sym: SymbolId,
    },
    Slice {
        data: Box<RuntimeValue>,
        len: usize,
    },
}

impl RuntimeValue {
    pub fn as_bool(&self) -> Result<bool, ComptimeError> {
        match self {
            RuntimeValue::Bool(b) => Ok(*b),
            _ => Err(ComptimeError::TypeMismatch("expected boolean value".to_string())),
        }
    }

    pub fn as_i128(&self) -> Result<i128, ComptimeError> {
        match self {
            RuntimeValue::Int { val, .. } => Ok(*val),
            _ => Err(ComptimeError::TypeMismatch("expected integer value".to_string())),
        }
    }

    pub fn to_comptime_value(&self, ctx: &SemanticContext) -> Result<ComptimeValue, ComptimeError> {
        match self {
            RuntimeValue::Unit => Ok(ComptimeValue::Unit),
            RuntimeValue::Bool(b) => Ok(ComptimeValue::Bool(*b)),
            RuntimeValue::Int { val, width } => Ok(ComptimeValue::Int { val: *val, width: *width }),
            RuntimeValue::Float { val, width } => Ok(ComptimeValue::Float { val: *val, width: *width }),
            RuntimeValue::Pointer(Address::Stack { .. }) => {
                Err(ComptimeError::PointerEscape("cannot return pointer to temporary compile-time stack memory".to_string()))
            }
            RuntimeValue::Pointer(Address::Heap { .. }) => {
                Err(ComptimeError::PointerEscape("cannot return raw heap pointer across compile-time boundary".to_string()))
            }
            RuntimeValue::NullPointer => Ok(ComptimeValue::Unit),
            RuntimeValue::TraitObject { .. } => {
                // Invariant: RuntimeReachable(V) ∩ VMObjects = ∅
                // Trait objects created in compile-time evaluation cannot escape into runtime constants
                // because their data points to VM memory and vtable is an internal compiler representation.
                Err(ComptimeError::PointerEscape("cannot return trait object across compile-time boundary: fat pointer references compiler-owned memory".to_string()))
            }
            RuntimeValue::Slice { data, .. } => {
                // Invariant: RuntimeReachable(V) ∩ VMObjects = ∅
                // Slices referencing VM stack or heap cannot escape into runtime
                if let RuntimeValue::Pointer(Address::Stack { .. }) = **data {
                    return Err(ComptimeError::PointerEscape("cannot return slice referencing temporary compile-time stack memory".to_string()));
                }
                if let RuntimeValue::Pointer(Address::Heap { .. }) = **data {
                    return Err(ComptimeError::PointerEscape("cannot return slice referencing compile-time heap memory across boundary".to_string()));
                }
                data.to_comptime_value(ctx)?;
                Err(ComptimeError::PointerEscape("cannot return slice fat pointer across compile-time boundary".to_string()))
            }
            RuntimeValue::Compound(elements) => {
                let mut converted = Vec::new();
                for elem in elements {
                    converted.push(elem.to_comptime_value(ctx)?);
                }
                Ok(ComptimeValue::Tuple(converted))
            }
            RuntimeValue::Variant { tag, payload, .. } => {
                let mut conv_payload = Vec::new();
                for p in payload {
                    conv_payload.push(p.to_comptime_value(ctx)?);
                }
                Ok(ComptimeValue::Enum {
                    symbol: None,
                    type_name: "Enum".to_string(),
                    variant_name: format!("Variant_{}", tag),
                    variant_index: *tag,
                    payload: conv_payload,
                })
            }
            RuntimeValue::Closure { .. } => {
                Err(ComptimeError::ResourceEscape("cannot export closure as compile-time constant value: runtime-observable VM resource cannot escape".to_string()))
            }
        }
    }

    pub fn from_comptime_value(val: &ComptimeValue) -> Self {
        match val {
            ComptimeValue::Unit => RuntimeValue::Unit,
            ComptimeValue::Bool(b) => RuntimeValue::Bool(*b),
            ComptimeValue::Int { val, width } => RuntimeValue::Int { val: *val, width: *width },
            ComptimeValue::Float { val, width } => RuntimeValue::Float { val: *val, width: *width },
            ComptimeValue::Char(c) => RuntimeValue::Int { val: *c as i128, width: IntWidth::U32 },
            ComptimeValue::Tuple(elems) => {
                RuntimeValue::Compound(elems.iter().map(Self::from_comptime_value).collect())
            }
            ComptimeValue::Array { elements, .. } => {
                RuntimeValue::Compound(elements.iter().map(Self::from_comptime_value).collect())
            }
            ComptimeValue::Struct { fields, .. } => {
                RuntimeValue::Compound(fields.iter().map(|(_, v)| Self::from_comptime_value(v)).collect())
            }
            ComptimeValue::Enum { variant_index, payload, .. } => {
                RuntimeValue::Variant {
                    enum_ty: SemanticTypeId(0),
                    tag: *variant_index,
                    payload: payload.iter().map(Self::from_comptime_value).collect(),
                }
            }
            _ => RuntimeValue::Unit,
        }
    }
}

#[derive(Debug, Default)]
pub struct HeapArena {
    pub allocations: HashMap<usize, Vec<MemorySlot>>,
    pub freed: HashMap<usize, bool>,
    pub next_alloc_id: usize,
}

pub struct StackFrame {
    pub func_name: String,
    pub slots: Vec<MemorySlot>,
    pub values: Vec<Option<RuntimeValue>>,
}

pub struct MvirInterpreter<'a> {
    pub module: &'a Module,
    pub ctx: &'a SemanticContext,
    pub steps: usize,
    pub max_steps: usize,
    pub max_depth: usize,
    pub heap: HeapArena,
    pub call_stack: Vec<StackFrame>,
}

impl<'a> MvirInterpreter<'a> {
    pub fn new(module: &'a Module, ctx: &'a SemanticContext) -> Self {
        Self {
            module,
            ctx,
            steps: 0,
            max_steps: 1_000_000,
            max_depth: 512,
            heap: HeapArena::default(),
            call_stack: Vec::new(),
        }
    }

    pub fn is_copy_type(&self, ty_id: SemanticTypeId) -> bool {
        let sem_ty = self.ctx.types.get(ty_id);
        match sem_ty {
            SemanticType::Primitive(mellis_semantic::ty::BuiltinType::String) => false,
            SemanticType::Primitive(_) => true,
            SemanticType::Reference(_, mutability, _) => {
                matches!(mutability, mellis_semantic::ty::Mutability::Immutable)
            }
            SemanticType::Pointer(..) => true,
            SemanticType::Tuple(elems) => elems.iter().all(|&e| self.is_copy_type(e)),
            SemanticType::Array(elem, _) => self.is_copy_type(*elem),
            SemanticType::GenericParam(sym_id) => {
                if let Some(bounds) = self.ctx.tables.trait_bounds.get(sym_id) {
                    bounds.iter().any(|b| {
                        let trait_sym = self.ctx.symbol_table.get_symbol(b.trait_id);
                        trait_sym.name == "Copy"
                    })
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn get_slot_mut(&mut self, addr: Address) -> Result<&mut MemorySlot, ComptimeError> {
        match addr {
            Address::Stack { frame_idx, slot_idx, field_idx, offset } => {
                if offset != 0 {
                    return Err(ComptimeError::Custom("E_OUT_OF_BOUNDS_DEREF: pointer offset out of bounds".to_string()));
                }
                let frame = self.call_stack.get_mut(frame_idx).ok_or_else(|| {
                    ComptimeError::Custom("invalid stack frame reference".to_string())
                })?;
                let slot = frame.slots.get_mut(slot_idx).ok_or_else(|| {
                    ComptimeError::Custom("invalid stack slot reference".to_string())
                })?;
                if let Some(f_idx) = field_idx {
                    if let RuntimeValue::Compound(fields) = &mut slot.value {
                        if let Some(_field_val) = fields.get_mut(f_idx as usize) {
                            return Ok(slot);
                        }
                    }
                }
                Ok(slot)
            }
            Address::Heap { alloc_id, field_idx: _, offset } => {
                if self.heap.freed.contains_key(&alloc_id) {
                    return Err(ComptimeError::UseAfterFree);
                }
                if offset != 0 {
                    return Err(ComptimeError::Custom("E_OUT_OF_BOUNDS_DEREF: pointer offset out of bounds".to_string()));
                }
                let slots = self.heap.allocations.get_mut(&alloc_id).ok_or_else(|| {
                    ComptimeError::UseAfterFree
                })?;
                slots.get_mut(0).ok_or_else(|| {
                    ComptimeError::Custom("empty heap allocation".to_string())
                })
            }
        }
    }

    fn read_memory(&mut self, addr: Address, ty: SemanticTypeId) -> Result<RuntimeValue, ComptimeError> {
        let (val, state, slot_ty) = {
            let slot = self.get_slot_mut(addr)?;
            (slot.value.clone(), slot.state, slot.ty)
        };
        let actual_ty = if ty != SemanticTypeId(0) { ty } else { slot_ty };
        let is_copy = match &val {
            RuntimeValue::Int { .. } | RuntimeValue::Float { .. } | RuntimeValue::Bool(_) | RuntimeValue::Unit => true,
            _ => self.is_copy_type(actual_ty),
        };
        match state {
            PlaceState::Uninitialized => {
                Err(ComptimeError::UseOfUninitializedOrMoved("reading uninitialized memory".to_string()))
            }
            PlaceState::Moved => {
                Err(ComptimeError::UseOfUninitializedOrMoved("use of moved value".to_string()))
            }
            PlaceState::Initialized => {
                match addr {
                    Address::Stack { field_idx: Some(f_idx), .. } | Address::Heap { field_idx: Some(f_idx), .. } => {
                        if let RuntimeValue::Compound(fields) = &val {
                            let f_val = fields.get(f_idx as usize).cloned().unwrap_or(RuntimeValue::Unit);
                            Ok(f_val)
                        } else if let RuntimeValue::TraitObject { data, .. } = &val {
                            if f_idx == 0 {
                                Ok(*data.clone())
                            } else {
                                Ok(RuntimeValue::Unit)
                            }
                        } else if let RuntimeValue::Slice { data, len } = &val {
                            if f_idx == 0 {
                                Ok(*data.clone())
                            } else if f_idx == 1 {
                                Ok(RuntimeValue::Int { val: *len as i128, width: IntWidth::USize })
                            } else {
                                Ok(RuntimeValue::Unit)
                            }
                        } else {
                            Ok(val)
                        }
                    }
                    _ => {
                        if !is_copy {
                            let slot = self.get_slot_mut(addr)?;
                            slot.state = PlaceState::Moved;
                        }
                        Ok(val)
                    }
                }
            }
        }
    }

    fn write_memory(&mut self, addr: Address, value: RuntimeValue) -> Result<(), ComptimeError> {
        let slot = self.get_slot_mut(addr)?;
        match addr {
            Address::Stack { field_idx: Some(f_idx), .. } | Address::Heap { field_idx: Some(f_idx), .. } => {
                if let RuntimeValue::Compound(fields) = &mut slot.value {
                    if (f_idx as usize) < fields.len() {
                        fields[f_idx as usize] = value;
                    } else {
                        fields.resize(f_idx as usize + 1, RuntimeValue::Unit);
                        fields[f_idx as usize] = value;
                    }
                } else if let RuntimeValue::TraitObject { data, .. } = &mut slot.value {
                    if f_idx == 0 {
                        *data = Box::new(value);
                    }
                } else if let RuntimeValue::Slice { data, len } = &mut slot.value {
                    if f_idx == 0 {
                        *data = Box::new(value);
                    } else if f_idx == 1 {
                        if let Ok(l) = value.as_i128() {
                            *len = l as usize;
                        }
                    }
                } else {
                    let mut fields = Vec::new();
                    fields.resize(f_idx as usize + 1, RuntimeValue::Unit);
                    fields[f_idx as usize] = value;
                    slot.value = RuntimeValue::Compound(fields);
                }
                slot.state = PlaceState::Initialized;
                return Ok(());
            }
            _ => {}
        }
        slot.value = value;
        slot.state = PlaceState::Initialized;
        Ok(())
    }

    fn eval_operand(&self, op: &Operand) -> Result<RuntimeValue, ComptimeError> {
        match op {
            Operand::Value(val_id) => {
                let frame = self.call_stack.last().ok_or_else(|| {
                    ComptimeError::Custom("no active stack frame".to_string())
                })?;
                frame.values.get(val_id.0 as usize)
                    .and_then(|v| v.clone())
                    .ok_or_else(|| ComptimeError::Custom(format!("SSA value {:?} not evaluated", val_id)))
            }
            Operand::Boolean(b) => Ok(RuntimeValue::Bool(*b)),
            Operand::Number(s) => {
                if s == "null" {
                    return Ok(RuntimeValue::NullPointer);
                }
                if let Ok(i) = s.parse::<i128>() {
                    Ok(RuntimeValue::Int { val: i, width: IntWidth::I32 })
                } else if let Ok(f) = s.parse::<f64>() {
                    Ok(RuntimeValue::Float { val: f, width: FloatWidth::F64 })
                } else {
                    Ok(RuntimeValue::Int { val: 0, width: IntWidth::I32 })
                }
            }
            Operand::Global(gid) => {
                if let Some(sym_id) = gid.symbol_id {
                    if let Some(ct_val) = self.ctx.const_values.get(&sym_id) {
                        return Ok(RuntimeValue::from_comptime_value(ct_val));
                    }
                }
                Err(ComptimeError::SymbolNotFound(gid.name.clone()))
            }
            Operand::Block(_) => Err(ComptimeError::UnsupportedOperation("block operand cannot be evaluated to value".to_string())),
            Operand::StringRef(_) => Err(ComptimeError::UnsupportedOperation("string literal evaluation not fully supported in comptime".to_string())),
            Operand::Char(_) => Err(ComptimeError::UnsupportedOperation("char literal evaluation not fully supported in comptime".to_string())),
        }
    }

    fn eval_binary_op(&self, left: i128, right: i128, width: IntWidth, op: &str) -> Result<RuntimeValue, ComptimeError> {
        let res = match op {
            "+" => {
                left.checked_add(right).ok_or(ComptimeError::IntegerOverflow)?
            }
            "-" => {
                left.checked_sub(right).ok_or(ComptimeError::IntegerOverflow)?
            }
            "*" => {
                left.checked_mul(right).ok_or(ComptimeError::IntegerOverflow)?
            }
            "/" => {
                if right == 0 {
                    return Err(ComptimeError::DivisionByZero);
                }
                left.checked_div(right).ok_or(ComptimeError::IntegerOverflow)?
            }
            "%" => {
                if right == 0 {
                    return Err(ComptimeError::DivisionByZero);
                }
                left.checked_rem(right).ok_or(ComptimeError::IntegerOverflow)?
            }
            _ => return Err(ComptimeError::UnsupportedOperation(format!("binary op {}", op))),
        };

        let bit_w = width.bit_width();
        if width.is_signed() {
            let min = -(1i128 << (bit_w - 1));
            let max = (1i128 << (bit_w - 1)) - 1;
            if res < min || res > max {
                return Err(ComptimeError::IntegerOverflow);
            }
        } else {
            let max = if bit_w == 128 { u128::MAX as i128 } else { (1i128 << bit_w) - 1 };
            if res < 0 || res > max {
                return Err(ComptimeError::IntegerOverflow);
            }
        }

        Ok(RuntimeValue::Int { val: res, width })
    }

    pub fn eval_function(&mut self, func: &Function, args: Vec<RuntimeValue>) -> Result<RuntimeValue, ComptimeError> {
        if self.call_stack.len() >= self.max_depth {
            return Err(ComptimeError::RecursionLimitExceeded(self.max_depth));
        }

        let frame_idx = self.call_stack.len();
        self.call_stack.push(StackFrame {
            func_name: func.name.name.clone(),
            slots: Vec::new(),
            values: vec![None; func.values.len()],
        });

        let mut current_block_idx = 0;

        'block_loop: loop {
            if current_block_idx >= func.blocks.len() {
                break;
            }
            let block = &func.blocks[current_block_idx];

            for &val_id in &block.insts {
                self.steps += 1;
                if self.steps > self.max_steps {
                    return Err(ComptimeError::StepLimitExceeded(self.max_steps));
                }

                let val_data = func.value(val_id);
                let inst_ty = val_data.ty;

                let res_val = match &val_data.inst {
                    Instruction::Alloca => {
                        let frame = self.call_stack.get_mut(frame_idx).unwrap();
                        let slot_idx = frame.slots.len();
                        let (init_val, init_state) = if slot_idx < func.arg_count && slot_idx < args.len() {
                            (args[slot_idx].clone(), PlaceState::Initialized)
                        } else {
                            (RuntimeValue::Unit, PlaceState::Uninitialized)
                        };
                        frame.slots.push(MemorySlot {
                            value: init_val,
                            state: init_state,
                            ty: inst_ty,
                        });
                        RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx: None, offset: 0 })
                    }
                    Instruction::HeapAlloc => {
                        let alloc_id = self.heap.next_alloc_id;
                        self.heap.next_alloc_id += 1;
                        self.heap.allocations.insert(alloc_id, vec![MemorySlot {
                            value: RuntimeValue::Unit,
                            state: PlaceState::Initialized,
                            ty: inst_ty,
                        }]);
                        RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: None, offset: 0 })
                    }
                    Instruction::BoxNew { value } => {
                        let val = self.eval_operand(value)?;
                        let alloc_id = self.heap.next_alloc_id;
                        self.heap.next_alloc_id += 1;
                        self.heap.allocations.insert(alloc_id, vec![MemorySlot {
                            value: val,
                            state: PlaceState::Initialized,
                            ty: inst_ty,
                        }]);
                        RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: None, offset: 0 })
                    }
                    Instruction::BoxFree { value } => {
                        let ptr = self.eval_operand(value)?;
                        if let RuntimeValue::Pointer(Address::Heap { alloc_id, .. }) = ptr {
                            if self.heap.freed.contains_key(&alloc_id) {
                                return Err(ComptimeError::UseAfterFree);
                            }
                            self.heap.allocations.remove(&alloc_id);
                            self.heap.freed.insert(alloc_id, true);
                        }
                        RuntimeValue::Unit
                    }
                    Instruction::Assign(op) => {
                        self.eval_operand(op)?
                    }
                    Instruction::Store { ptr, value } => {
                        let ptr_val = self.eval_operand(ptr)?;
                        let val = self.eval_operand(value)?;
                        match ptr_val {
                            RuntimeValue::Pointer(addr) => {
                                self.write_memory(addr, val)?;
                                RuntimeValue::Unit
                            }
                            RuntimeValue::NullPointer | RuntimeValue::Int { val: 0, .. } => {
                                return Err(ComptimeError::NullPointerDereference);
                            }
                            _ => RuntimeValue::Unit,
                        }
                    }
                    Instruction::Load { ptr } => {
                        let ptr_val = self.eval_operand(ptr)?;
                        match ptr_val {
                            RuntimeValue::Pointer(addr) => self.read_memory(addr, inst_ty)?,
                            RuntimeValue::NullPointer | RuntimeValue::Int { val: 0, .. } => {
                                return Err(ComptimeError::NullPointerDereference);
                            }
                            _ => RuntimeValue::Unit,
                        }
                    }
                    Instruction::Borrow { base, .. } => {
                        let base_val = self.eval_operand(base)?;
                        base_val
                    }
                    Instruction::FieldPtr { base, field_idx } => {
                        let base_val = self.eval_operand(base)?;
                        match base_val {
                            RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx: _, offset }) => {
                                RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx: Some(*field_idx), offset })
                            }
                            RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: _, offset }) => {
                                RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: Some(*field_idx), offset })
                            }
                            _ => base_val,
                        }
                    }
                    Instruction::Add { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, width }, RuntimeValue::Int { val: v2, .. }) => {
                                self.eval_binary_op(v1, v2, width, "+")?
                            }
                            (RuntimeValue::Float { val: v1, width }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Float { val: v1 + v2, width }
                            }
                            (RuntimeValue::Pointer(addr), RuntimeValue::Int { val: offset, .. }) => {
                                match addr {
                                    Address::Stack { frame_idx, slot_idx, field_idx, offset: curr_off } => {
                                        RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx, offset: curr_off + offset as isize })
                                    }
                                    Address::Heap { alloc_id, field_idx, offset: curr_off } => {
                                        RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx, offset: curr_off + offset as isize })
                                    }
                                }
                            }
                            (RuntimeValue::NullPointer, RuntimeValue::Int { .. }) => {
                                return Err(ComptimeError::NullPointerDereference);
                            }
                            _ => return Err(ComptimeError::TypeMismatch("add operand mismatch".to_string())),
                        }
                    }
                    Instruction::Sub { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, width }, RuntimeValue::Int { val: v2, .. }) => {
                                self.eval_binary_op(v1, v2, width, "-")?
                            }
                            (RuntimeValue::Float { val: v1, width }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Float { val: v1 - v2, width }
                            }
                            (RuntimeValue::Pointer(addr), RuntimeValue::Int { val: offset, .. }) => {
                                match addr {
                                    Address::Stack { frame_idx, slot_idx, field_idx, offset: curr_off } => {
                                        RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx, offset: curr_off - offset as isize })
                                    }
                                    Address::Heap { alloc_id, field_idx, offset: curr_off } => {
                                        RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx, offset: curr_off - offset as isize })
                                    }
                                }
                            }
                            (RuntimeValue::Pointer(addr1), RuntimeValue::Pointer(addr2)) => {
                                match (addr1, addr2) {
                                    (Address::Stack { frame_idx: f1, slot_idx: s1, offset: o1, .. }, Address::Stack { frame_idx: f2, slot_idx: s2, offset: o2, .. }) => {
                                        if f1 == f2 && s1 == s2 {
                                            RuntimeValue::Int { val: (o1 - o2) as i128, width: IntWidth::ISize }
                                        } else {
                                            return Err(ComptimeError::Custom("cannot subtract pointers to different stack allocations".to_string()));
                                        }
                                    }
                                    (Address::Heap { alloc_id: a1, offset: o1, .. }, Address::Heap { alloc_id: a2, offset: o2, .. }) => {
                                        if a1 == a2 {
                                            RuntimeValue::Int { val: (o1 - o2) as i128, width: IntWidth::ISize }
                                        } else {
                                            return Err(ComptimeError::Custom("cannot subtract pointers to different heap allocations".to_string()));
                                        }
                                    }
                                    _ => return Err(ComptimeError::Custom("cannot subtract pointers to different memory regions".to_string())),
                                }
                            }
                            (RuntimeValue::NullPointer, _) => {
                                return Err(ComptimeError::NullPointerDereference);
                            }
                            _ => return Err(ComptimeError::TypeMismatch("sub operand mismatch".to_string())),
                        }
                    }
                    Instruction::Mul { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (&l, &r) {
                            (RuntimeValue::Int { val: v1, width }, RuntimeValue::Int { val: v2, .. }) => {
                                self.eval_binary_op(*v1, *v2, *width, "*")?
                            }
                            (RuntimeValue::Float { val: v1, width }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Float { val: v1 * v2, width: *width }
                            }
                            _ => return Err(ComptimeError::TypeMismatch(format!("mul operand mismatch: left={:?}, right={:?}", l, r))),
                        }
                    }
                    Instruction::Div { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, width }, RuntimeValue::Int { val: v2, .. }) => {
                                self.eval_binary_op(v1, v2, width, "/")?
                            }
                            (RuntimeValue::Float { val: v1, width }, RuntimeValue::Float { val: v2, .. }) => {
                                if v2 == 0.0 {
                                    return Err(ComptimeError::DivisionByZero);
                                }
                                RuntimeValue::Float { val: v1 / v2, width }
                            }
                            _ => return Err(ComptimeError::TypeMismatch("div operand mismatch".to_string())),
                        }
                    }
                    Instruction::Rem { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, width }, RuntimeValue::Int { val: v2, .. }) => {
                                self.eval_binary_op(v1, v2, width, "%")?
                            }
                            _ => return Err(ComptimeError::TypeMismatch("rem operand mismatch".to_string())),
                        }
                    }
                    Instruction::Eq { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, .. }, RuntimeValue::Int { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 == v2)
                            }
                            (RuntimeValue::Bool(b1), RuntimeValue::Bool(b2)) => RuntimeValue::Bool(b1 == b2),
                            _ => RuntimeValue::Bool(false),
                        }
                    }
                    Instruction::NotEq { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, .. }, RuntimeValue::Int { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 != v2)
                            }
                            (RuntimeValue::Bool(b1), RuntimeValue::Bool(b2)) => RuntimeValue::Bool(b1 != b2),
                            _ => RuntimeValue::Bool(true),
                        }
                    }
                    Instruction::LessThan { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, .. }, RuntimeValue::Int { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 < v2)
                            }
                            (RuntimeValue::Float { val: v1, .. }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 < v2)
                            }
                            _ => return Err(ComptimeError::TypeMismatch("comparison operand mismatch".to_string())),
                        }
                    }
                    Instruction::LessOrEq { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, .. }, RuntimeValue::Int { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 <= v2)
                            }
                            (RuntimeValue::Float { val: v1, .. }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 <= v2)
                            }
                            _ => return Err(ComptimeError::TypeMismatch("comparison operand mismatch".to_string())),
                        }
                    }
                    Instruction::GreaterThan { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, .. }, RuntimeValue::Int { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 > v2)
                            }
                            (RuntimeValue::Float { val: v1, .. }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 > v2)
                            }
                            _ => return Err(ComptimeError::TypeMismatch("comparison operand mismatch".to_string())),
                        }
                    }
                    Instruction::GreaterOrEq { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, .. }, RuntimeValue::Int { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 >= v2)
                            }
                            (RuntimeValue::Float { val: v1, .. }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Bool(v1 >= v2)
                            }
                            _ => return Err(ComptimeError::TypeMismatch("comparison operand mismatch".to_string())),
                        }
                    }
                    Instruction::BoundsCheck { index, len } => {
                        let idx = self.eval_operand(index)?.as_i128()?;
                        let l = self.eval_operand(len)?.as_i128()?;
                        if idx < 0 || idx >= l {
                            return Err(ComptimeError::Custom(format!("index {} out of bounds for length {}", idx, l)));
                        }
                        RuntimeValue::Unit
                    }
                    Instruction::Await { .. } => {
                        return Err(ComptimeError::ComptimeAwaitForbidden);
                    }
                    Instruction::SizeOf { ty } => {
                        let size = self.calculate_size_of(*ty);
                        RuntimeValue::Int { val: size as i128, width: IntWidth::USize }
                    }
                    Instruction::AlignOf { ty } => {
                        let align = self.calculate_align_of(*ty);
                        RuntimeValue::Int { val: align as i128, width: IntWidth::USize }
                    }
                    Instruction::Variant { enum_ty, variant_idx, args } => {
                        let mut arg_vals = Vec::new();
                        for a in args {
                            arg_vals.push(self.eval_operand(a)?);
                        }
                        RuntimeValue::Variant {
                            enum_ty: *enum_ty,
                            tag: *variant_idx,
                            payload: arg_vals,
                        }
                    }
                    Instruction::Tag { value } => {
                        let v = self.eval_operand(value)?;
                        if let RuntimeValue::Variant { tag, .. } = v {
                            RuntimeValue::Int { val: tag as i128, width: IntWidth::U32 }
                        } else {
                            RuntimeValue::Int { val: 0, width: IntWidth::U32 }
                        }
                    }
                    Instruction::Extract { value, field_idx, .. } => {
                        let v = self.eval_operand(value)?;
                        match v {
                            RuntimeValue::Compound(elems) => {
                                elems.get(*field_idx as usize).cloned().unwrap_or(RuntimeValue::Unit)
                            }
                            RuntimeValue::Variant { payload, .. } => {
                                payload.get(*field_idx as usize).cloned().unwrap_or(RuntimeValue::Unit)
                            }
                            RuntimeValue::TraitObject { data, .. } => {
                                if *field_idx == 0 {
                                    *data
                                } else {
                                    RuntimeValue::Unit
                                }
                            }
                            RuntimeValue::Slice { data, len } => {
                                if *field_idx == 0 {
                                    *data
                                } else if *field_idx == 1 {
                                    RuntimeValue::Int { val: len as i128, width: IntWidth::USize }
                                } else {
                                    RuntimeValue::Unit
                                }
                            }
                            _ => RuntimeValue::Unit,
                        }
                    }

                    Instruction::CallDirect { callee, args } => {
                        if let Some(sym_id) = callee.symbol_id {
                            if let Some(effects) = self.ctx.tables.function_effects.get(&sym_id) {
                                if !effects.is_pure() {
                                    return Err(ComptimeError::ForbiddenSideEffect(
                                        format!("calling function '{}' with effect is forbidden in comptime", callee.name)
                                    ));
                                }
                            }
                        }

                        let target_func = self.module.functions.iter().find(|f| f.name.name == callee.name);
                        if let Some(f) = target_func {
                            if f.is_extern {
                                return Err(ComptimeError::ForbiddenSideEffect(
                                    format!("calling extern function '{}' is forbidden in comptime", callee.name)
                                ));
                            }
                            let mut arg_vals = Vec::new();
                            for a in args {
                                arg_vals.push(self.eval_operand(a)?);
                            }
                            let target_f = f.clone();
                            self.eval_function(&target_f, arg_vals)?
                        } else {
                            return Err(ComptimeError::SymbolNotFound(callee.name.clone()));
                        }
                    }
                    Instruction::Drop { value, callee, .. } => {
                        let val = self.eval_operand(value)?;
                        let mut should_drop = true;
                        if let RuntimeValue::Pointer(addr) = val {
                            if let Ok(slot) = self.get_slot_mut(addr) {
                                if slot.state == PlaceState::Moved || slot.state == PlaceState::Uninitialized {
                                    should_drop = false;
                                } else {
                                    slot.state = PlaceState::Moved;
                                }
                            }
                        }
                        if should_drop {
                            if let Some(c_id) = callee {
                                if let Some(f) = self.module.functions.iter().find(|f| f.name.name == c_id.name).cloned() {
                                    self.eval_function(&f, vec![val])?;
                                }
                            } else if let RuntimeValue::Pointer(Address::Heap { alloc_id, .. }) = val {
                                self.heap.allocations.remove(&alloc_id);
                                self.heap.freed.insert(alloc_id, true);
                            }
                        }
                        RuntimeValue::Unit
                    }
                    Instruction::MakeClosure { func, env_ptr, .. } => {
                        let env_val = self.eval_operand(env_ptr)?;
                        if let RuntimeValue::Pointer(addr) = env_val {
                            RuntimeValue::Closure { func: func.clone(), env_ptr: addr }
                        } else {
                            RuntimeValue::Unit
                        }
                    }
                    Instruction::CallClosure { closure, args } => {
                        let clos_val = self.eval_operand(closure)?;
                        if let RuntimeValue::Closure { func, env_ptr } = clos_val {
                            let mut call_args = vec![RuntimeValue::Pointer(env_ptr)];
                            for a in args {
                                call_args.push(self.eval_operand(a)?);
                            }
                            if let Some(f) = self.module.functions.iter().find(|f| f.name.name == func.name).cloned() {
                                self.eval_function(&f, call_args)?
                            } else {
                                return Err(ComptimeError::SymbolNotFound(func.name));
                            }
                        } else {
                            return Err(ComptimeError::TypeMismatch("expected closure value".to_string()));
                        }
                    }
                    Instruction::Cast { value, target_ty } => {
                        let val = self.eval_operand(value)?;
                        let sem_target = self.ctx.types.get(*target_ty);
                        match sem_target {
                            SemanticType::Pointer(..) => {
                                match val {
                                    RuntimeValue::Pointer(_) => val,
                                    RuntimeValue::NullPointer => RuntimeValue::NullPointer,
                                    RuntimeValue::Int { val: 0, .. } => RuntimeValue::NullPointer,
                                    _ => val,
                                }
                            }
                            SemanticType::Primitive(b) if b.is_integer() => {
                                match val {
                                    RuntimeValue::Int { val: v, .. } => RuntimeValue::Int { val: v, width: IntWidth::from_builtin(*b) },
                                    RuntimeValue::Pointer(_) => val,
                                    RuntimeValue::NullPointer => RuntimeValue::Int { val: 0, width: IntWidth::from_builtin(*b) },
                                    _ => val,
                                }
                            }
                            _ => val,
                        }
                    }
                    Instruction::Null { .. } => RuntimeValue::NullPointer,
                    Instruction::MakeTraitObject { data_ptr, vtable, trait_sym, concrete_sym } => {
                        let data_val = self.eval_operand(data_ptr)?;
                        RuntimeValue::TraitObject {
                            data: Box::new(data_val),
                            vtable: vtable.clone(),
                            trait_sym: *trait_sym,
                            concrete_sym: *concrete_sym,
                        }
                    }
                    Instruction::MakeSlice { data_ptr, len } => {
                        let data_val = self.eval_operand(data_ptr)?;
                        let len_val = self.eval_operand(len)?;
                        let l = len_val.as_i128().map_err(|_| ComptimeError::TypeMismatch("expected integer length for slice".to_string()))? as usize;
                        RuntimeValue::Slice {
                            data: Box::new(data_val),
                            len: l,
                        }
                    }
                    Instruction::CallVirt { obj, method_idx, args } => {
                        let obj_val = self.eval_operand(obj)?;
                        let obj_val = match obj_val {
                            RuntimeValue::Pointer(addr) => {
                                if let Ok(slot) = self.get_slot_mut(addr) {
                                    slot.value.clone()
                                } else {
                                    obj_val
                                }
                            }
                            other => other,
                        };
                        match obj_val {
                            RuntimeValue::TraitObject { data, trait_sym, concrete_sym, .. } => {
                                if let Some(methods) = self.ctx.tables.trait_methods.get(&trait_sym) {
                                    if let Some(&m_sym) = methods.get(*method_idx as usize) {
                                        let concrete_name = &self.ctx.symbol_table.get_symbol(concrete_sym).name;
                                        let m_name = &self.ctx.symbol_table.get_symbol(m_sym).name;
                                        let mangled_name = format!("{}_{}", concrete_name, m_name);
                                        let target_func = self.module.functions.iter().find(|f| {
                                            f.name.name == mangled_name || f.name.name == *m_name || f.name.name.ends_with(m_name)
                                        }).cloned();
                                        if let Some(f) = target_func {
                                            let mut call_args = vec![*data];
                                            for a in args {
                                                call_args.push(self.eval_operand(a)?);
                                            }
                                            self.eval_function(&f, call_args)?
                                        } else {
                                            return Err(ComptimeError::UnsupportedOperation(format!(
                                                "E_COMPTIME_VIRTUAL_CALL: concrete method `{}` of trait `{}` on `{}` not found in comptime module",
                                                m_name, self.ctx.symbol_table.get_symbol(trait_sym).name, concrete_name
                                            )));
                                        }
                                    } else {
                                        return Err(ComptimeError::UnsupportedOperation(format!(
                                            "E_COMPTIME_VIRTUAL_CALL: method index {} out of range for trait `{}`",
                                            method_idx, self.ctx.symbol_table.get_symbol(trait_sym).name
                                        )));
                                    }
                                } else {
                                    return Err(ComptimeError::UnsupportedOperation(format!(
                                        "E_COMPTIME_VIRTUAL_CALL: no methods found for trait `{}`",
                                        self.ctx.symbol_table.get_symbol(trait_sym).name
                                    )));
                                }
                            }
                            _ => {
                                return Err(ComptimeError::UnsupportedOperation(
                                    "E_COMPTIME_VIRTUAL_CALL_UNSUPPORTED: virtual call on unsupported or null trait object in comptime".to_string()
                                ));
                            }
                        }
                    }
                    Instruction::DropVirt { obj } => {
                        let obj_val = self.eval_operand(obj)?;
                        let obj_val = match obj_val {
                            RuntimeValue::Pointer(addr) => {
                                if let Ok(slot) = self.get_slot_mut(addr) {
                                    slot.value.clone()
                                } else {
                                    obj_val
                                }
                            }
                            other => other,
                        };
                        match obj_val {
                            RuntimeValue::TraitObject { data, concrete_sym, .. } => {
                                if let Some(&drop_sym) = self.ctx.tables.drop_impls.get(&concrete_sym) {
                                    let drop_name = &self.ctx.symbol_table.get_symbol(drop_sym).name;
                                    let concrete_name = &self.ctx.symbol_table.get_symbol(concrete_sym).name;
                                    let mangled_name = format!("{}_{}", concrete_name, drop_name);
                                    let drop_func = self.module.functions.iter().find(|f| {
                                        f.name.name == mangled_name || f.name.name == *drop_name || f.name.name.ends_with(drop_name)
                                    }).cloned();
                                    if let Some(f) = drop_func {
                                        self.eval_function(&f, vec![*data])?;
                                    }
                                } else if let RuntimeValue::Pointer(Address::Heap { alloc_id, .. }) = *data {
                                    self.heap.allocations.remove(&alloc_id);
                                    self.heap.freed.insert(alloc_id, true);
                                }
                                RuntimeValue::Unit
                            }
                            RuntimeValue::NullPointer => RuntimeValue::Unit,
                            _ => {
                                return Err(ComptimeError::UnsupportedOperation(
                                    "E_COMPTIME_DROP_VIRT_UNSUPPORTED: invalid or unsupported object for virtual drop in comptime".to_string()
                                ));
                            }
                        }
                    }
                    _ => RuntimeValue::Unit,
                };

                let frame = self.call_stack.get_mut(frame_idx).unwrap();
                frame.values[val_id.0 as usize] = Some(res_val);
            }

            match &block.terminator {
                Some(Terminator::Ret { value }) => {
                    let ret_val = if let Some(op) = value {
                        self.eval_operand(op)?
                    } else {
                        RuntimeValue::Unit
                    };
                    self.call_stack.pop();
                    return Ok(ret_val);
                }
                Some(Terminator::Br { target }) => {
                    if let Some(idx) = func.blocks.iter().position(|b| b.label.name == target.name) {
                        current_block_idx = idx;
                        continue 'block_loop;
                    }
                    break;
                }
                Some(Terminator::CondBr { condition, true_target, false_target }) => {
                    let cond = self.eval_operand(condition)?.as_bool()?;
                    let target = if cond { true_target } else { false_target };
                    if let Some(idx) = func.blocks.iter().position(|b| b.label.name == target.name) {
                        current_block_idx = idx;
                        continue 'block_loop;
                    }
                    break;
                }
                Some(Terminator::Unreachable) => {
                    return Err(ComptimeError::Custom("reached unreachable terminator in comptime".to_string()));
                }
                _ => break,
            }
        }

        self.call_stack.pop();
        Ok(RuntimeValue::Unit)
    }

    fn calculate_size_of(&self, ty_id: SemanticTypeId) -> usize {
        let sem_ty = self.ctx.types.get(ty_id);
        match sem_ty {
            SemanticType::Primitive(b) => match b {
                mellis_semantic::ty::BuiltinType::Bool => 1,
                mellis_semantic::ty::BuiltinType::Char => 4,
                mellis_semantic::ty::BuiltinType::I8 | mellis_semantic::ty::BuiltinType::U8 => 1,
                mellis_semantic::ty::BuiltinType::I16 | mellis_semantic::ty::BuiltinType::U16 => 2,
                mellis_semantic::ty::BuiltinType::I32 | mellis_semantic::ty::BuiltinType::U32 | mellis_semantic::ty::BuiltinType::F32 => 4,
                mellis_semantic::ty::BuiltinType::I64 | mellis_semantic::ty::BuiltinType::U64 | mellis_semantic::ty::BuiltinType::F64 => 8,
                mellis_semantic::ty::BuiltinType::I128 | mellis_semantic::ty::BuiltinType::U128 => 16,
                mellis_semantic::ty::BuiltinType::Isize | mellis_semantic::ty::BuiltinType::Usize => 8, // Assuming 64-bit for now
                mellis_semantic::ty::BuiltinType::String => 16,
            },
            SemanticType::Void => 0,
            SemanticType::Pointer(..) | SemanticType::Reference(..) => 8,
            SemanticType::Array(elem, len) => self.calculate_size_of(*elem) * (*len as usize),
            SemanticType::Tuple(elems) => elems.iter().map(|&e| self.calculate_size_of(e)).sum(),
            SemanticType::Struct(sym_id, _, _) => {
                if let Some(fields) = self.ctx.tables.struct_fields.get(sym_id) {
                    fields.iter().map(|f_sym| {
                        let f_ty = self.ctx.tables.symbol_types.get(f_sym).copied().unwrap_or(SemanticTypeId(0));
                        self.calculate_size_of(f_ty)
                    }).sum()
                } else {
                    8
                }
            }
            _ => 8,
        }
    }

    fn calculate_align_of(&self, ty_id: SemanticTypeId) -> usize {
        let sem_ty = self.ctx.types.get(ty_id);
        match sem_ty {
            SemanticType::Primitive(b) => match b {
                mellis_semantic::ty::BuiltinType::Bool => 1,
                mellis_semantic::ty::BuiltinType::Char => 4,
                mellis_semantic::ty::BuiltinType::I8 | mellis_semantic::ty::BuiltinType::U8 => 1,
                mellis_semantic::ty::BuiltinType::I16 | mellis_semantic::ty::BuiltinType::U16 => 2,
                mellis_semantic::ty::BuiltinType::I32 | mellis_semantic::ty::BuiltinType::U32 | mellis_semantic::ty::BuiltinType::F32 => 4,
                mellis_semantic::ty::BuiltinType::I64 | mellis_semantic::ty::BuiltinType::U64 | mellis_semantic::ty::BuiltinType::F64 => 8,
                mellis_semantic::ty::BuiltinType::I128 | mellis_semantic::ty::BuiltinType::U128 => 8,
                mellis_semantic::ty::BuiltinType::Isize | mellis_semantic::ty::BuiltinType::Usize => 8,
                mellis_semantic::ty::BuiltinType::String => 8,
            },
            SemanticType::Void => 1,
            SemanticType::Pointer(..) | SemanticType::Reference(..) => 8,
            SemanticType::Array(elem, _) => self.calculate_align_of(*elem),
            SemanticType::Tuple(elems) => elems.iter().map(|&e| self.calculate_align_of(e)).max().unwrap_or(1),
            SemanticType::Struct(sym_id, _, _) => {
                if let Some(fields) = self.ctx.tables.struct_fields.get(sym_id) {
                    fields.iter().map(|f_sym| {
                        let f_ty = self.ctx.tables.symbol_types.get(f_sym).copied().unwrap_or(SemanticTypeId(0));
                        self.calculate_align_of(f_ty)
                    }).max().unwrap_or(1)
                } else {
                    8
                }
            }
            _ => 8,
        }
    }


}

pub struct MvirComptimeEngine {
    pub max_steps: usize,
    pub max_depth: usize,
}

impl Default for MvirComptimeEngine {
    fn default() -> Self {
        Self {
            max_steps: 1_000_000,
            max_depth: 512,
        }
    }
}

fn has_unresolved_projection(ctx: &SemanticContext, ty_id: SemanticTypeId) -> bool {
    let ty = ctx.types.get(ty_id);
    match ty {
        SemanticType::Projection { .. } | SemanticType::GenericParam(_) => true,
        SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
            has_unresolved_projection(ctx, *inner)
        }
        SemanticType::Array(inner, _) | SemanticType::Slice(inner) => {
            has_unresolved_projection(ctx, *inner)
        }
        SemanticType::Tuple(elems) => {
            elems.iter().any(|&e| has_unresolved_projection(ctx, e))
        }
        SemanticType::Function { params, return_type } => {
            has_unresolved_projection(ctx, *return_type) || params.iter().any(|&p| has_unresolved_projection(ctx, p))
        }
        _ => false,
    }
}

impl mellis_semantic::ComptimeEngine for MvirComptimeEngine {
    fn eval_expr(&self, arena: &mellis_ast::AstArena, ctx: &SemanticContext, source_manager: &mellis_common::source::SourceManager, expr_id: mellis_ast::ExprId) -> Result<ComptimeValue, ComptimeError> {
        let expr = &arena.exprs[expr_id.0 as usize];
        if let mellis_ast::Expr::Comptime { body } = expr {
            return self.eval_stmt(arena, ctx, source_manager, *body);
        }
        let ret_ty = ctx.tables.expr_types.get(&expr_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
        if has_unresolved_projection(ctx, ret_ty) {
            return Err(ComptimeError::TypeMismatch("E_UNRESOLVED_PROJECTION: cannot evaluate comptime expression with unresolved associated type projection".to_string()));
        }
        let mut generator = crate::generator::MvirGenerator::new(arena, ctx, source_manager);
        generator.generate_all_known_functions();
        let func = generator.generate_expr_as_function(&expr_id, ret_ty);
        if func.values.iter().any(|v| has_unresolved_projection(ctx, v.ty)) {
            return Err(ComptimeError::TypeMismatch("E_UNRESOLVED_PROJECTION: comptime function contains unresolved associated type projection".to_string()));
        }
        let mut interp = MvirInterpreter::new(generator.current_module(), ctx);
        interp.max_steps = self.max_steps;
        interp.max_depth = self.max_depth;
        let val = interp.eval_function(&func, Vec::new())?;
        if !interp.heap.allocations.is_empty() {
            return Err(ComptimeError::ResourceLeak(format!(
                "comptime evaluation leaked heap memory: {} unfreed allocation(s) at compile-time boundary",
                interp.heap.allocations.len()
            )));
        }
        val.to_comptime_value(ctx)
    }

    fn eval_stmt(&self, arena: &mellis_ast::AstArena, ctx: &SemanticContext, source_manager: &mellis_common::source::SourceManager, stmt_id: mellis_ast::StmtId) -> Result<ComptimeValue, ComptimeError> {
        let mut generator = crate::generator::MvirGenerator::new(arena, ctx, source_manager);
        generator.generate_all_known_functions();
        let func = generator.generate_stmt_as_function(&stmt_id, mellis_semantic::SemanticTypeId(0));
        if func.values.iter().any(|v| has_unresolved_projection(ctx, v.ty)) {
            return Err(ComptimeError::TypeMismatch("E_UNRESOLVED_PROJECTION: comptime function contains unresolved associated type projection".to_string()));
        }
        let mut interp = MvirInterpreter::new(generator.current_module(), ctx);
        interp.max_steps = self.max_steps;
        interp.max_depth = self.max_depth;
        let val = interp.eval_function(&func, Vec::new())?;
        if !interp.heap.allocations.is_empty() {
            return Err(ComptimeError::ResourceLeak(format!(
                "comptime evaluation leaked heap memory: {} unfreed allocation(s) at compile-time boundary",
                interp.heap.allocations.len()
            )));
        }
        val.to_comptime_value(ctx)
    }
}


