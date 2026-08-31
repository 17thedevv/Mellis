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
    Stack { frame_idx: usize, slot_idx: usize, field_idx: Option<u32> },
    Heap { alloc_id: usize, field_idx: Option<u32> },
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
                Err(ComptimeError::UnsupportedOperation("cannot export closure as compile-time constant value".to_string()))
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
            _ => false,
        }
    }

    fn get_slot_mut(&mut self, addr: Address) -> Result<&mut MemorySlot, ComptimeError> {
        match addr {
            Address::Stack { frame_idx, slot_idx, field_idx } => {
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
            Address::Heap { alloc_id, field_idx: _ } => {
                if self.heap.freed.contains_key(&alloc_id) {
                    return Err(ComptimeError::Custom("use-after-free in compile-time heap memory".to_string()));
                }
                let slots = self.heap.allocations.get_mut(&alloc_id).ok_or_else(|| {
                    ComptimeError::Custom("invalid heap allocation reference".to_string())
                })?;
                slots.get_mut(0).ok_or_else(|| {
                    ComptimeError::Custom("empty heap allocation".to_string())
                })
            }
        }
    }

    fn read_memory(&mut self, addr: Address, ty: SemanticTypeId) -> Result<RuntimeValue, ComptimeError> {
        let is_copy = self.is_copy_type(ty);
        let slot = self.get_slot_mut(addr)?;
        match slot.state {
            PlaceState::Uninitialized => {
                Err(ComptimeError::UseOfUninitializedOrMoved("reading uninitialized memory".to_string()))
            }
            PlaceState::Moved => {
                Err(ComptimeError::UseOfUninitializedOrMoved("use of moved value".to_string()))
            }
            PlaceState::Initialized => {
                match addr {
                    Address::Stack { field_idx: Some(f_idx), .. } | Address::Heap { field_idx: Some(f_idx), .. } => {
                        if let RuntimeValue::Compound(fields) = &slot.value {
                            let f_val = fields.get(f_idx as usize).cloned().unwrap_or(RuntimeValue::Unit);
                            Ok(f_val)
                        } else {
                            Ok(slot.value.clone())
                        }
                    }
                    _ => {
                        let val = slot.value.clone();
                        if !is_copy {
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
                    while fields.len() <= f_idx as usize {
                        fields.push(RuntimeValue::Unit);
                    }
                    fields[f_idx as usize] = value;
                    slot.state = PlaceState::Initialized;
                    return Ok(());
                }
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
                        RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx: None })
                    }
                    Instruction::HeapAlloc => {
                        let alloc_id = self.heap.next_alloc_id;
                        self.heap.next_alloc_id += 1;
                        self.heap.allocations.insert(alloc_id, vec![MemorySlot {
                            value: RuntimeValue::Unit,
                            state: PlaceState::Initialized,
                            ty: inst_ty,
                        }]);
                        RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: None })
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
                        RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: None })
                    }
                    Instruction::BoxFree { value } => {
                        let ptr = self.eval_operand(value)?;
                        if let RuntimeValue::Pointer(Address::Heap { alloc_id, .. }) = ptr {
                            if self.heap.freed.contains_key(&alloc_id) {
                                return Err(ComptimeError::Custom("double free in compile-time memory".to_string()));
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
                        if let RuntimeValue::Pointer(addr) = ptr_val {
                            self.write_memory(addr, val)?;
                        }
                        RuntimeValue::Unit
                    }
                    Instruction::Load { ptr } => {
                        let ptr_val = self.eval_operand(ptr)?;
                        if let RuntimeValue::Pointer(addr) = ptr_val {
                            self.read_memory(addr, inst_ty)?
                        } else {
                            RuntimeValue::Unit
                        }
                    }
                    Instruction::Borrow { base, .. } => {
                        let base_val = self.eval_operand(base)?;
                        base_val
                    }
                    Instruction::FieldPtr { base, field_idx } => {
                        let base_val = self.eval_operand(base)?;
                        match base_val {
                            RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx: _ }) => {
                                RuntimeValue::Pointer(Address::Stack { frame_idx, slot_idx, field_idx: Some(*field_idx) })
                            }
                            RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: _ }) => {
                                RuntimeValue::Pointer(Address::Heap { alloc_id, field_idx: Some(*field_idx) })
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
                            _ => return Err(ComptimeError::TypeMismatch("sub operand mismatch".to_string())),
                        }
                    }
                    Instruction::Mul { left, right } => {
                        let l = self.eval_operand(left)?;
                        let r = self.eval_operand(right)?;
                        match (l, r) {
                            (RuntimeValue::Int { val: v1, width }, RuntimeValue::Int { val: v2, .. }) => {
                                self.eval_binary_op(v1, v2, width, "*")?
                            }
                            (RuntimeValue::Float { val: v1, width }, RuntimeValue::Float { val: v2, .. }) => {
                                RuntimeValue::Float { val: v1 * v2, width }
                            }
                            _ => return Err(ComptimeError::TypeMismatch("mul operand mismatch".to_string())),
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
                        if let Some(c_id) = callee {
                            if let Some(f) = self.module.functions.iter().find(|f| f.name.name == c_id.name).cloned() {
                                self.eval_function(&f, vec![val])?;
                            }
                        } else if let RuntimeValue::Pointer(Address::Heap { alloc_id, .. }) = val {
                            self.heap.allocations.remove(&alloc_id);
                            self.heap.freed.insert(alloc_id, true);
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
            SemanticType::Struct(sym_id, _) => {
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
            SemanticType::Struct(sym_id, _) => {
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

pub struct MvirComptimeEngine;

impl mellis_semantic::ComptimeEngine for MvirComptimeEngine {
    fn eval_expr(&self, arena: &mellis_ast::AstArena, ctx: &SemanticContext, source: &str, expr_id: mellis_ast::ExprId) -> Result<ComptimeValue, ComptimeError> {
        let expr = &arena.exprs[expr_id.0 as usize];
        if let mellis_ast::Expr::Comptime { body } = expr {
            return self.eval_stmt(arena, ctx, source, *body);
        }
        let ret_ty = ctx.tables.expr_types.get(&expr_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
        let mut generator = crate::generator::MvirGenerator::new(arena, ctx, source);
        generator.generate_all_known_functions();
        let func = generator.generate_expr_as_function(&expr_id, ret_ty);
        let mut interp = MvirInterpreter::new(generator.current_module(), ctx);
        let val = interp.eval_function(&func, Vec::new())?;
        val.to_comptime_value(ctx)
    }

    fn eval_stmt(&self, arena: &mellis_ast::AstArena, ctx: &SemanticContext, source: &str, stmt_id: mellis_ast::StmtId) -> Result<ComptimeValue, ComptimeError> {
        let mut generator = crate::generator::MvirGenerator::new(arena, ctx, source);
        generator.generate_all_known_functions();
        let func = generator.generate_stmt_as_function(&stmt_id, mellis_semantic::SemanticTypeId(0));
        let mut interp = MvirInterpreter::new(generator.current_module(), ctx);
        let val = interp.eval_function(&func, Vec::new())?;
        val.to_comptime_value(ctx)
    }
}

