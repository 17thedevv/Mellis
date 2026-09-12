use crate::dataflow::DataflowAnalysis;
use crate::place::Place;
use luna_common::Diagnostic;
use luna_mvir::{Function, Instruction, Operand, Terminator};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveState {
    Uninitialized,
    Live,
    Moved,
    Dropped,
    ConditionallyMoved,
    PartialMoved,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MoveStateData {
    pub is_unvisited: bool,
    pub places: HashMap<Place, MoveState>,
}

impl Default for MoveStateData {
    fn default() -> Self {
        Self { is_unvisited: true, places: HashMap::new() }
    }
}

impl MoveStateData {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct MoveAnalyzer<'a> {
    pub diagnostics: Vec<Diagnostic>,
    pub emit_diagnostics: bool,
    pub dead_drops: std::collections::HashSet<luna_mvir::ValueId>,
    pub func: &'a Function,
    pub semantic_ctx: Option<&'a luna_semantic::SemanticContext>,
    pub summaries: Option<&'a HashMap<luna_mvir::GlobalId, crate::effect::CallEffectSummary>>,
    pub values_to_places: HashMap<luna_mvir::ValueId, Place>,
    pub load_origins: HashMap<luna_mvir::ValueId, luna_mvir::ValueId>,
}

impl<'a> MoveAnalyzer<'a> {
    pub fn new(func: &'a Function, semantic_ctx: Option<&'a luna_semantic::SemanticContext>, summaries: Option<&'a HashMap<luna_mvir::GlobalId, crate::effect::CallEffectSummary>>) -> Self {
        Self {
            diagnostics: Vec::new(),
            emit_diagnostics: false,
            dead_drops: std::collections::HashSet::new(),
            func,
            semantic_ctx,
            summaries,
            values_to_places: HashMap::new(),
            load_origins: HashMap::new(),
        }
    }

    fn get_place_state(&self, place: &Place, state: &MoveStateData) -> MoveState {
        let mut current_ancestor = None;
        let mut max_len = -1;
        for (k, v) in &state.places {
            if k.is_ancestor_of(place) {
                if k.projections.len() as isize > max_len {
                    max_len = k.projections.len() as isize;
                    current_ancestor = Some(v.clone());
                }
            }
        }
        
        let effective_state = current_ancestor.unwrap_or(MoveState::Live);
        
        if effective_state == MoveState::Live {
            for (k, v) in &state.places {
                if k.is_descendant_of(place) {
                    if matches!(v, MoveState::Moved | MoveState::Dropped | MoveState::Uninitialized | MoveState::ConditionallyMoved) {
                        return MoveState::PartialMoved;
                    }
                }
            }
        }
        
        effective_state
    }

    fn check_place(&mut self, place: &Place, state: &MoveStateData, val_id: luna_mvir::ValueId) {
        if !self.emit_diagnostics { return; }
        let loc_state = self.get_place_state(place, state);
        let name = format!("%v{}", place.local.0);
        let mut formatted_name = name;
        for proj in &place.projections {
            match proj {
                crate::place::Projection::Field(idx) => {
                    formatted_name = format!("{}.{}", formatted_name, idx);
                }
                crate::place::Projection::Deref => {
                    formatted_name = format!("(*{})", formatted_name);
                }
                crate::place::Projection::Index => {
                    formatted_name = format!("{}[_]", formatted_name);
                }
            }
        }
        
        let span = self.func.values[val_id.0 as usize].span.clone();
        if loc_state == MoveState::Moved {
            let msg = format!("Use of moved value '{}'", formatted_name);
            if !self.diagnostics.iter().any(|d| d.message == msg) {
                let mut diag = Diagnostic::error(msg);
                diag.span = span.clone();
                self.diagnostics.push(diag);
            }
        } else if loc_state == MoveState::ConditionallyMoved {
            let msg = format!("Use of conditionally moved value '{}'", formatted_name);
            if !self.diagnostics.iter().any(|d| d.message == msg) {
                let mut diag = Diagnostic::error(msg);
                diag.span = span.clone();
                self.diagnostics.push(diag);
            }
        } else if loc_state == MoveState::Dropped {
            let msg = format!("Use of dropped value '{}'", formatted_name);
            if !self.diagnostics.iter().any(|d| d.message == msg) {
                let mut diag = Diagnostic::error(msg);
                diag.span = span.clone();
                self.diagnostics.push(diag);
            }
        } else if loc_state == MoveState::Uninitialized {
            let msg = format!("Use of uninitialized value '{}'", formatted_name);
            if !self.diagnostics.iter().any(|d| d.message == msg) {
                let mut diag = Diagnostic::error(msg);
                diag.span = span.clone();
                self.diagnostics.push(diag);
            }
        } else if loc_state == MoveState::PartialMoved {
            let msg = format!("Use of partially moved value '{}'", formatted_name);
            if !self.diagnostics.iter().any(|d| d.message == msg) {
                let mut diag = Diagnostic::error(msg);
                diag.span = span.clone();
                self.diagnostics.push(diag);
            }
        }
    }

    fn check_operand(&mut self, op: &Operand, state: &MoveStateData, val_id: luna_mvir::ValueId) {
        if !self.emit_diagnostics { return; }
        if let Operand::Value(val) = op {
            let place = match self.values_to_places.get(val).cloned() {
                Some(p) => p,
                None => return, // Temporary, assumed live
            };
            self.check_place(&place, state, val_id);
        }
    }

    fn mark_moved(&mut self, op: &Operand, state: &mut MoveStateData) {
        if let Operand::Value(val) = op {
            if let Some(ctx) = self.semantic_ctx {
                let inst_ty = self.func.value(*val).ty;
                let sem_ty = ctx.types.get(inst_ty);
                match sem_ty {
                    luna_semantic::SemanticType::Primitive(b) => {
                        use luna_semantic::ty::BuiltinType;
                        if matches!(b, BuiltinType::I8 | BuiltinType::I16 | BuiltinType::I32 | BuiltinType::I64 | BuiltinType::I128 | BuiltinType::Isize | BuiltinType::U8 | BuiltinType::U16 | BuiltinType::U32 | BuiltinType::U64 | BuiltinType::U128 | BuiltinType::Usize | BuiltinType::F32 | BuiltinType::F64 | BuiltinType::Bool | BuiltinType::Char) {
                            return; // Primitive types are trivially copyable
                        }
                    }
                    luna_semantic::SemanticType::Void => {
                        return;
                    }
                    luna_semantic::SemanticType::Pointer(_, _) | luna_semantic::SemanticType::Reference(..) => {
                        return; // Pointers and references are copyable handles
                    }
                    _ => {}
                }
            }

            if let Some(place) = self.values_to_places.get(val).cloned() {
                if self.emit_diagnostics && !place.projections.is_empty() {
                    if let Some(ctx) = self.semantic_ctx {
                        for len in 0..place.projections.len() {
                            let mut ancestor = place.clone();
                            ancestor.projections.truncate(len);
                            let mut ancestor_ty = None;
                            for (v_id, p) in &self.values_to_places {
                                if p == &ancestor {
                                    ancestor_ty = Some(self.func.value(*v_id).ty);
                                    break;
                                }
                            }
                            if let Some(ty_id) = ancestor_ty {
                                let sem_ty = ctx.types.get(ty_id).clone();
                                let has_drop = match sem_ty {
                                    luna_semantic::SemanticType::Struct(sym_id, _, _) | luna_semantic::SemanticType::Enum(sym_id, _, _) => {
                                        ctx.tables.drop_impls.contains_key(&sym_id)
                                    },
                                    _ => false,
                                };
                                if has_drop {
                                    let msg = "Cannot move out of a subplace of a type that implements Drop [E_PARTIAL_MOVE_UNDER_DROP]".to_string();
                                    if !self.diagnostics.iter().any(|d| d.message == msg) {
                                        let mut diag = Diagnostic::error(msg);
                                        diag.span = self.func.values[val.0 as usize].span.clone();
                                        self.diagnostics.push(diag);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                
                state.places.retain(|k, _| !k.is_descendant_of(&place));
                state.places.insert(place, MoveState::Moved);
            }
        }
    }

    fn mark_dropped(&mut self, op: &Operand, state: &mut MoveStateData) {
        if let Operand::Value(val) = op {
            if let Some(place) = self.values_to_places.get(val).cloned() {
                let current_state = self.get_place_state(&place, state);
                if current_state != MoveState::Moved && current_state != MoveState::Uninitialized {
                    state.places.retain(|k, _| !k.is_descendant_of(&place));
                    state.places.insert(place, MoveState::Dropped);
                }
            }
        }
    }
    
    fn mark_live(&mut self, op: &Operand, state: &mut MoveStateData) {
        if let Operand::Value(val) = op {
            if let Some(place) = self.values_to_places.get(val).cloned() {
                state.places.retain(|k, _| !k.is_descendant_of(&place));
                state.places.insert(place.clone(), MoveState::Live);

                if !place.projections.is_empty() {
                    let mut parent = place.clone();
                    parent.projections.pop();
                    if let Some(parent_state) = state.places.get(&parent) {
                        if *parent_state == MoveState::Uninitialized {
                            state.places.insert(parent, MoveState::Live);
                        }
                    }
                }
            }
        }
    }
}

impl<'a> DataflowAnalysis<MoveStateData> for MoveAnalyzer<'a> {
    fn transfer_instruction(
        &mut self,
        val_id: luna_mvir::ValueId,
        inst: &Instruction,
        state: &mut MoveStateData,
    ) {
        match inst {
            Instruction::Alloca | Instruction::HeapAlloc => {
                let p = Place::new(val_id);
                self.values_to_places.insert(val_id, p.clone());
                if (val_id.0 as usize) >= self.func.arg_count {
                    state.places.insert(p, MoveState::Uninitialized);
                } else {
                    state.places.insert(p, MoveState::Live);
                }
            }
            Instruction::Assign(op) => {
                self.check_operand(op, state, val_id);
                self.mark_moved(op, state);
            }
            Instruction::Store { ptr, value } => {
                self.check_operand(value, state, val_id);
                self.mark_moved(value, state);
                self.mark_live(ptr, state);
            }
            Instruction::Load { ptr, .. } => {
                self.check_operand(ptr, state, val_id);
                if let Operand::Value(ptr_val) = ptr {
                    if let Some(ptr_place) = self.values_to_places.get(ptr_val).cloned() {
                        self.values_to_places.insert(val_id, ptr_place);
                    }
                }
            }
            Instruction::CallDirect { args, callee, .. } => {
                self.check_operand(&Operand::Global(callee.clone()), state, val_id);
                let mut effects = None;
                if let Some(sums) = self.summaries {
                    effects = sums.get(callee);
                }

                for arg in args {
                    self.check_operand(arg, state, val_id);
                    self.mark_moved(arg, state);
                }
            }
            Instruction::CallIndirect { args, callee, .. } => {
                self.check_operand(callee, state, val_id);
                for arg in args {
                    self.check_operand(arg, state, val_id);
                    self.mark_moved(arg, state);
                }
            }
            Instruction::Await { future } => {
                self.check_operand(future, state, val_id);
                self.mark_moved(future, state);
            }
            Instruction::CallClosure { closure, args, .. } => {
                self.check_operand(closure, state, val_id);
                for arg in args {
                    self.check_operand(arg, state, val_id);
                    self.mark_moved(arg, state);
                }
            }
            Instruction::MakeClosure { captures, .. } => {
                for cap in captures {
                    self.check_operand(&Operand::Value(cap.source), state, val_id);
                    if cap.mode == luna_semantic::CaptureMode::Move {
                        self.mark_moved(&Operand::Value(cap.source), state);
                    }
                }
            }
            Instruction::MakeTraitObject { data_ptr, .. } => {
                self.check_operand(data_ptr, state, val_id);
            }
            Instruction::MakeSlice { data_ptr, len } => {
                self.check_operand(data_ptr, state, val_id);
                self.check_operand(len, state, val_id);
            }
            Instruction::DropVirt { obj } => {
                self.check_operand(obj, state, val_id);
            }
            Instruction::CallIntrinsic { args, .. } => {
                for arg in args {
                    self.check_operand(arg, state, val_id);
                }
            }
            Instruction::CallVirt { obj, args, .. } => {
                self.check_operand(obj, state, val_id);
                for arg in args {
                    self.check_operand(arg, state, val_id);
                    self.mark_moved(arg, state);
                }
            }

            Instruction::Borrow { base, .. } => {
                self.check_operand(base, state, val_id);
            }
            Instruction::Add { left, right, .. }
            | Instruction::Sub { left, right, .. }
            | Instruction::Mul { left, right, .. }
            | Instruction::Div { left, right, .. }
            | Instruction::Rem { left, right, .. }
            | Instruction::Eq { left, right, .. }
            | Instruction::NotEq { left, right, .. }
            | Instruction::LessThan { left, right, .. }
            | Instruction::LessOrEq { left, right, .. }
            | Instruction::GreaterThan { left, right, .. }
            | Instruction::GreaterOrEq { left, right, .. }
            | Instruction::BitAnd { left, right, .. }
            | Instruction::BitOr { left, right, .. }
            | Instruction::BitXor { left, right, .. }
            | Instruction::Shl { left, right, .. }
            | Instruction::Shr { left, right, .. } => {
                self.check_operand(left, state, val_id);
                self.check_operand(right, state, val_id);
            }
            Instruction::BoxNew { value } => {
                self.mark_moved(value, state);
            }
            Instruction::BoxFree { value } => {
                self.mark_dropped(value, state);
            }
            Instruction::MarkInit { value } => {
                self.mark_live(value, state);
            }
            Instruction::SizeOf { .. } |
            Instruction::AlignOf { .. } |
            Instruction::Null { .. } |
            Instruction::Cast { .. } |
            Instruction::PtrOffset { .. } => {}
            Instruction::FieldPtr { base, field_idx, .. } => {
                if let Operand::Value(base_val) = base {
                    if let Some(base_place) = self.values_to_places.get(base_val).cloned() {
                        self.values_to_places.insert(val_id, base_place.projected(crate::place::Projection::Field(*field_idx as usize)));
                    }
                }
            }
            Instruction::BoundsCheck { index, len } => {
                self.check_operand(index, state, val_id);
                self.check_operand(len, state, val_id);
            }
            Instruction::Variant { args, .. } => {
                for arg in args {
                    self.check_operand(arg, state, val_id);
                    self.mark_moved(arg, state);
                }
            }
            Instruction::Tag { value } => {
                self.check_operand(value, state, val_id);
            }
            Instruction::Extract { value, field_idx, .. } => {
                if let Operand::Value(base_val) = value {
                    if let Some(base_place) = self.values_to_places.get(base_val).cloned() {
                        let field_place = base_place.projected(crate::place::Projection::Field(*field_idx as usize));
                        self.values_to_places.insert(val_id, field_place.clone());
                        
                        // Check the field place instead of the base place to allow partial moves
                        if self.emit_diagnostics {
                            let loc_state = self.get_place_state(&field_place, state);
                            if matches!(loc_state, MoveState::Moved | MoveState::Dropped | MoveState::ConditionallyMoved | MoveState::Uninitialized) {
                                self.check_place(&field_place, state, val_id);
                            }
                        }
                        
                        // Check E_PARTIAL_MOVE_UNDER_DROP: if any ancestor of field_place
                        // has a user Drop impl, reject the extraction as a partial move.
                        // This is checked directly here rather than via mark_moved because
                        // Extract must NOT mark the place as moved — the downstream consumer
                        // (Store, Variant, etc.) does that.
                        if self.emit_diagnostics && !field_place.projections.is_empty() {
                            if let Some(ctx) = self.semantic_ctx {
                                for len in 0..field_place.projections.len() {
                                    let mut ancestor = field_place.clone();
                                    ancestor.projections.truncate(len);
                                    let mut ancestor_ty = None;
                                    for (v_id, p) in &self.values_to_places {
                                        if p == &ancestor {
                                            ancestor_ty = Some(self.func.value(*v_id).ty);
                                            break;
                                        }
                                    }
                                    if let Some(ty_id) = ancestor_ty {
                                        let sem_ty = ctx.types.get(ty_id).clone();
                                        let has_drop = match sem_ty {
                                            luna_semantic::SemanticType::Struct(sym_id, _, _) | luna_semantic::SemanticType::Enum(sym_id, _, _) => {
                                                ctx.tables.drop_impls.contains_key(&sym_id)
                                            },
                                            _ => false,
                                        };
                                        if has_drop {
                                            let msg = "Cannot move out of a subplace of a type that implements Drop [E_PARTIAL_MOVE_UNDER_DROP]".to_string();
                                            if !self.diagnostics.iter().any(|d| d.message == msg) {
                                                let mut diag = Diagnostic::error(msg);
                                                diag.span = self.func.values[val_id.0 as usize].span.clone();
                                                self.diagnostics.push(diag);
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Instruction::Drop { value, .. } => {
                if self.emit_diagnostics {
                    if let Operand::Value(val) = value {
                        if let Some(place) = self.values_to_places.get(val).cloned() {
                            let loc_state = self.get_place_state(&place, state);
                            if loc_state == MoveState::ConditionallyMoved {
                                let msg = format!("Cannot drop conditionally moved value");
                                if !self.diagnostics.iter().any(|d| d.message == msg) {
                                    let mut diag = Diagnostic::error(msg);
                                    diag.span = self.func.values[val.0 as usize].span.clone();
                                    self.diagnostics.push(diag);
                                }
                            }
                            if loc_state == MoveState::Moved || loc_state == MoveState::Dropped || loc_state == MoveState::Uninitialized {
                                self.dead_drops.insert(val_id);
                            }
                        }
                    }
                }
                self.mark_dropped(value, state);
            }
            Instruction::Nop => {}
        }
    }

    fn transfer_terminator(&mut self, term: &Terminator, state: &mut MoveStateData) {
        match term {
            Terminator::Ret { value } => {
                if let Some(val) = value {
                    self.check_operand(val, state, luna_mvir::ValueId(0));
                    self.mark_moved(val, state);
                }
            }
            Terminator::CondBr { condition, .. } => {
                self.check_operand(condition, state, luna_mvir::ValueId(0));
            }
            _ => {}
        }
    }


    fn merge(&mut self, dest: &mut MoveStateData, src: &MoveStateData) -> bool {
        if dest.is_unvisited {
            *dest = src.clone();
            dest.is_unvisited = false;
            return true;
        }
        
        let mut changed = false;
        
        let mut all_keys = HashSet::new();
        for k in dest.places.keys() { all_keys.insert(k.clone()); }
        for k in src.places.keys() { all_keys.insert(k.clone()); }
        
        for k in all_keys {
            let dest_state = dest.places.get(&k).cloned();
            let src_state = src.places.get(&k).cloned();
            
            // If neither has it directly, they might inherit it from ancestors, but we only merge explicit entries
            if dest_state == src_state { continue; }
            
            // If one has an explicit state and the other doesn't, the other inherits from its ancestors.
            // But we need to be careful: if we merge, we should merge the *effective* states of the place in both CFG paths!
            let effective_dest = self.get_place_state(&k, dest);
            let effective_src = self.get_place_state(&k, src);
            
            let new_state = match (effective_dest.clone(), effective_src.clone()) {
                (MoveState::Uninitialized, MoveState::Uninitialized) => MoveState::Uninitialized,
                (MoveState::Dropped, MoveState::Dropped) => MoveState::Dropped,
                (MoveState::Moved, MoveState::Moved) => MoveState::Moved,
                (MoveState::ConditionallyMoved, _) | (_, MoveState::ConditionallyMoved) => MoveState::ConditionallyMoved,
                (MoveState::Moved, _) | (_, MoveState::Moved) => MoveState::ConditionallyMoved,
                (MoveState::Dropped, _) | (_, MoveState::Dropped) => MoveState::ConditionallyMoved,
                (MoveState::Uninitialized, _) | (_, MoveState::Uninitialized) => MoveState::ConditionallyMoved,
                (MoveState::PartialMoved, MoveState::PartialMoved) => MoveState::PartialMoved,
                (MoveState::PartialMoved, _) | (_, MoveState::PartialMoved) => MoveState::PartialMoved, // actually a partial move mixed with Live is basically a partial move, but maybe ConditionallyMoved? Let's use PartialMoved
                _ => MoveState::Live,
            };
            
            // Only update dest if it differs from the current explicit state?
            // Actually, if we just insert the merged state, we might be inserting redundant entries.
            // But that's safe.
            if dest_state.as_ref() != Some(&new_state) {
                if new_state == MoveState::ConditionallyMoved && k.local.0 == 5 {
                }
                dest.places.insert(k, new_state);
                changed = true;
            }
        }
        
        changed
    }

    fn init_entry_state(&mut self, func: &Function, state: &mut MoveStateData) {
        state.is_unvisited = false;
        let mut alloca_count = 0;
        for (idx, val_data) in func.values.iter().enumerate() {
            if matches!(val_data.inst, Instruction::Alloca) {
                let init_state = if alloca_count < func.arg_count {
                    MoveState::Live
                } else {
                    MoveState::Uninitialized
                };
                
                let p = Place::new(luna_mvir::ValueId(idx as u32));
                state.places.insert(p, init_state);
                    
                alloca_count += 1;
            }
        }
    }
}
