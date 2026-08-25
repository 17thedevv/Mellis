use crate::dataflow::{DataflowAnalysis, DataflowEngine};
use crate::effect::{CallEffectSummary, EscapeKind, ReturnEffect};
use mellis_common::Diagnostic;
use mellis_mvir::{Function, GlobalId, Instruction, Operand, Terminator, ValueId};
use std::collections::{HashMap, HashSet};

// --- Liveness Analysis ---

#[derive(Clone, Default, PartialEq, Eq)]
pub struct LivenessState {
    pub live: HashSet<ValueId>,
}

pub struct LivenessAnalyzer;

impl DataflowAnalysis<LivenessState> for LivenessAnalyzer {
    fn transfer_instruction(&mut self, val_id: ValueId, inst: &Instruction, state: &mut LivenessState) {
        // In backward dataflow, definition kills liveness
        state.live.remove(&val_id);

        // Uses make values live
        match inst {
            Instruction::Load { ptr } => {
                if let Operand::Value(v) = ptr { state.live.insert(*v); }
            }
            Instruction::Store { ptr, value } => {
                if let Operand::Value(v) = ptr { state.live.insert(*v); }
                if let Operand::Value(v) = value { state.live.insert(*v); }
            }
            Instruction::Borrow { base, .. } => {
                if let Operand::Value(v) = base { state.live.insert(*v); }
            }
            Instruction::Call { args, callee, .. } => {
                if let Operand::Value(v) = callee { state.live.insert(*v); }
                for arg in args {
                    if let Operand::Value(v) = arg { state.live.insert(*v); }
                }
            }
            Instruction::Add { left, right } | Instruction::Sub { left, right } | Instruction::Mul { left, right } | Instruction::Eq { left, right } => {
                if let Operand::Value(v) = left { state.live.insert(*v); }
                if let Operand::Value(v) = right { state.live.insert(*v); }
            }
            Instruction::Drop { value } => {
                if let Operand::Value(v) = value { state.live.insert(*v); }
            }
            Instruction::Alloca | _ => {}
        }
    }

    fn transfer_terminator(&mut self, term: &Terminator, state: &mut LivenessState) {
        match term {
            Terminator::Ret { value: Some(Operand::Value(v)) } => {
                state.live.insert(*v);
            }
            Terminator::CondBr { condition: Operand::Value(v), .. } => {
                state.live.insert(*v);
            }
            _ => {}
        }
    }

    fn merge(&mut self, dest: &mut LivenessState, src: &LivenessState) -> bool {
        let mut changed = false;
        for &val in &src.live {
            if dest.live.insert(val) {
                changed = true;
            }
        }
        changed
    }

    fn init_entry_state(&mut self, _func: &Function, _state: &mut LivenessState) {}
}

pub fn compute_liveness(func: &Function) -> (HashMap<String, LivenessState>, HashMap<ValueId, HashSet<ValueId>>) {
    let mut analyzer = LivenessAnalyzer;
    // run_backward gives us the OUT state of each block
    let block_out = DataflowEngine::run_backward(func, &mut analyzer);
    
    // Now compute liveness BEFORE each instruction
    let mut live_before = HashMap::new();
    
    for block in &func.blocks {
        let mut current_state = block_out.get(&block.label.name).cloned().unwrap_or_default();
        if let Some(term) = &block.terminator {
            analyzer.transfer_terminator(term, &mut current_state);
        }
        
        for &val_id in block.insts.iter().rev() {
            analyzer.transfer_instruction(val_id, &func.value(val_id).inst, &mut current_state);
            live_before.insert(val_id, current_state.live.clone());
        }
    }
    
    (block_out, live_before)
}

// --- Borrow Analysis (NLL) ---

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Loan {
    pub id: ValueId,
    pub place: Operand,
    pub is_rw: bool,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct BorrowStateData {
    pub provenance: HashMap<ValueId, HashSet<Loan>>,
    pub escaped_loans: HashSet<Loan>,
}

use mellis_semantic::SemanticContext;

pub struct BorrowAnalyzer<'a> {
    pub diagnostics: Vec<Diagnostic>,
    live_before: HashMap<ValueId, HashSet<ValueId>>,
    callee_summaries: Option<&'a HashMap<GlobalId, CallEffectSummary>>,
    ctx: Option<&'a SemanticContext>,
    func: &'a Function,
    pub emit_diagnostics: bool,
}

impl<'a> BorrowAnalyzer<'a> {
    pub fn new(live_before: HashMap<ValueId, HashSet<ValueId>>, callee_summaries: Option<&'a HashMap<GlobalId, CallEffectSummary>>, ctx: Option<&'a SemanticContext>, func: &'a Function) -> Self {
        Self {
            diagnostics: Vec::new(),
            live_before,
            callee_summaries,
            ctx,
            func,
            emit_diagnostics: false,
        }
    }

    pub fn analyze(func: &'a Function, summaries: Option<&'a HashMap<GlobalId, CallEffectSummary>>, ctx: Option<&'a SemanticContext>) -> Vec<Diagnostic> {
        let (_, live_before) = compute_liveness(func);
        let mut analyzer = Self::new(live_before, summaries, ctx, func);
        let block_states = DataflowEngine::run_forward(func, &mut analyzer);
        
        // Second pass to emit diagnostics with final computed state
        analyzer.emit_diagnostics = true;
        for block in &func.blocks {
            let mut current_state = block_states.get(&block.label.name).cloned().unwrap_or_default();
            for &val_id in &block.insts {
                let val_data = func.value(val_id);
                analyzer.transfer_instruction(val_id, &val_data.inst, &mut current_state);
            }
        }
        
        analyzer.diagnostics
    }

    fn active_loans(&self, val_id: ValueId, state: &BorrowStateData) -> HashSet<Loan> {
        let mut active = state.escaped_loans.clone();
        if let Some(live_set) = self.live_before.get(&val_id) {
            for v in live_set {
                if let Some(loans) = state.provenance.get(v) {
                    active.extend(loans.iter().cloned());
                }
            }
        }
        active
    }

    fn check_access(&mut self, place: &Operand, is_write: bool, val_id: ValueId, state: &BorrowStateData) {
        if !self.emit_diagnostics { return; }
        let place_name = print_operand_name(place);
        let active = self.active_loans(val_id, state);
        
        for loan in &active {
            if print_operand_name(&loan.place) == place_name {
                if loan.is_rw {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "Cannot access '{}' because it is borrowed as &rw",
                        place_name
                    )));
                } else if is_write {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "Cannot write to '{}' because it is borrowed as &",
                        place_name
                    )));
                }
            }
        }
    }

    fn issue_loan(&mut self, place: &Operand, is_rw: bool, val_id: ValueId, state: &mut BorrowStateData) {
        let place_name = print_operand_name(place);
        let active = self.active_loans(val_id, state);
        
        if self.emit_diagnostics {
            for loan in &active {
                if print_operand_name(&loan.place) == place_name {
                    if loan.is_rw {
                        self.diagnostics.push(Diagnostic::error(format!(
                            "Cannot borrow '{}' as {} because it is already borrowed as &rw",
                            place_name,
                            if is_rw { "&rw" } else { "&" }
                        )));
                    } else if is_rw {
                        self.diagnostics.push(Diagnostic::error(format!(
                            "Cannot borrow '{}' as &rw because it is already borrowed as &",
                            place_name
                        )));
                    }
                }
            }
        }

        let new_loan = Loan {
            id: val_id,
            place: place.clone(),
            is_rw,
        };
        
        state.provenance.entry(val_id).or_default().insert(new_loan);
    }
}

fn print_operand_name(op: &Operand) -> String {
    match op {
        Operand::Value(val) => format!("%v{}", val.0),
        Operand::Global(glb) => glb.name.clone(),
        Operand::Block(blk) => format!("%block_{}", blk.0),
        Operand::Number(n) => n.clone(),
        Operand::Boolean(b) => b.to_string(),
    }
}

impl<'a> DataflowAnalysis<BorrowStateData> for BorrowAnalyzer<'a> {
    fn init_entry_state(&mut self, _func: &Function, _state: &mut BorrowStateData) {}

    fn transfer_instruction(&mut self, val_id: ValueId, inst: &Instruction, state: &mut BorrowStateData) {
        match inst {
            Instruction::Store { ptr, value } => {
                self.check_access(value, false, val_id, state);
                self.check_access(ptr, true, val_id, state);
                
                // If ptr is a Value, it inherits provenance from value (for alias tracking in borrowck)
                // Actually, if we store a reference into a local, that local now holds the provenance.
                // In NLL, if we do `*ptr = value`, we are overwriting `ptr`. But wait, `Instruction::Store` in MVIR is usually `Store { ptr: %v1, value: %v2 }`.
                // If %v1 is an Alloca, %v1 now contains the provenance of %v2.
                // To keep it simple, if ptr is Operand::Value, we add provenance of value to ptr.
                if let Operand::Value(ptr_val) = ptr {
                    if let Operand::Value(val_v) = value {
                        if let Some(prov) = state.provenance.get(val_v).cloned() {
                            state.provenance.entry(*ptr_val).or_default().extend(prov);
                        }
                    }
                }
            }
            Instruction::Load { ptr } => {
                self.check_access(ptr, false, val_id, state);
                // Load from a pointer inherits its provenance
                if let Operand::Value(ptr_val) = ptr {
                    if let Some(prov) = state.provenance.get(ptr_val).cloned() {
                        state.provenance.entry(val_id).or_default().extend(prov);
                    }
                }
            }
            Instruction::Call { args, callee, .. } => {
                self.check_access(callee, false, val_id, state);
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                }
                
                let mut applied_summary = None;
                if let Operand::Global(gid) = callee {
                    if let Some(map) = self.callee_summaries {
                        if let Some(sum) = map.get(gid) {
                            applied_summary = Some(sum.clone());
                        }
                    }
                }
                
                if let Some(sum) = applied_summary {
                    for (i, arg) in args.iter().enumerate() {
                        if i < sum.args.len() {
                            let arg_effect = &sum.args[i];
                            // If callee escapes the arg, add its provenance to escaped_loans
                            if arg_effect.escape == EscapeKind::MayEscape {
                                if let Operand::Value(arg_v) = arg {
                                    if let Some(prov) = state.provenance.get(arg_v).cloned() {
                                        state.escaped_loans.extend(prov);
                                    }
                                }
                            }
                            
                            // Check access based on access kind
                            // If callee reads, check read. If callee writes, check write.
                            use crate::effect::AccessKind;
                            if arg_effect.access == AccessKind::Read || arg_effect.access == AccessKind::ReadWrite {
                                self.check_access(arg, false, val_id, state);
                            }
                            if arg_effect.access == AccessKind::Write || arg_effect.access == AccessKind::ReadWrite {
                                self.check_access(arg, true, val_id, state);
                            }
                        }
                    }
                    
                    // Return provenance
                    if let ReturnEffect::BorrowsFrom(indices) = &sum.ret {
                        for &idx in indices {
                            if idx < args.len() {
                                if let Operand::Value(arg_v) = &args[idx] {
                                    if let Some(prov) = state.provenance.get(arg_v).cloned() {
                                        state.provenance.entry(val_id).or_default().extend(prov);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Opaque call conservative fallback using CE6 ABI rules
                    let mut callee_sym_id = None;
                    if let Operand::Global(gid) = callee {
                        if let Some(sym_id) = gid.symbol_id {
                            callee_sym_id = Some(sym_id);
                        }
                    }

                    // Pre-compute callback effects to union into this call
                    let mut cb_worst_access = crate::effect::AccessKind::None;
                    let mut cb_worst_escape = crate::effect::EscapeKind::NoEscape;

                    for arg in args {
                        if let Operand::Global(cb_gid) = arg {
                            if let Some(summaries) = self.callee_summaries {
                                if let Some(cb_sum) = summaries.get(cb_gid) {
                                    for cb_arg in &cb_sum.args {
                                        cb_worst_access = cb_worst_access.merge(&cb_arg.access);
                                        cb_worst_escape = cb_worst_escape.merge(&cb_arg.escape);
                                    }
                                } else {
                                    // Static callback but no summary? Fallback to unknown.
                                    cb_worst_access = crate::effect::AccessKind::Unknown;
                                    cb_worst_escape = crate::effect::EscapeKind::Unknown;
                                }
                            }
                        } else if let Operand::Value(val_id) = arg {
                            if let Some(ctx) = self.ctx {
                                let val_data = self.func.value(*val_id);
                                if let mellis_semantic::ty::SemanticType::Function { .. } = ctx.types.get(val_data.ty) {
                                    // Dynamic callback fallback
                                    cb_worst_access = crate::effect::AccessKind::Unknown;
                                    cb_worst_escape = crate::effect::EscapeKind::Unknown;
                                }
                            }
                        }
                    }

                    for (arg_pos, arg) in args.iter().enumerate() {
                        let mut escape_kind = crate::effect::EscapeKind::Unknown;
                        let mut access_kind = crate::effect::AccessKind::ReadWrite;
                        let mut has_sync_noescape = false;

                        if let Some(sym_id) = callee_sym_id {
                            if let Some(ctx) = self.ctx {
                                if let Some(noescapes) = ctx.tables.ffi_sync_noescape.get(&sym_id) {
                                    if arg_pos < noescapes.len() {
                                        has_sync_noescape = noescapes[arg_pos];
                                    }
                                }
                            }
                        }

                        if let Operand::Value(val_id) = arg {
                            if let Some(ctx) = self.ctx {
                                let val_data = self.func.value(*val_id);
                                use mellis_semantic::ty::SemanticType;
                                match ctx.types.get(val_data.ty) {
                                    SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => {
                                        escape_kind = crate::effect::EscapeKind::NoEscape;
                                        access_kind = crate::effect::AccessKind::None;
                                    }
                                    SemanticType::Struct(_, _) | SemanticType::Enum(_, _) | SemanticType::Tuple(_) | SemanticType::Array(_, _) | SemanticType::Slice(_) => {
                                        // Passed by value (or opaque copy), no memory effect on the caller's aliasing
                                        escape_kind = crate::effect::EscapeKind::NoEscape;
                                        access_kind = crate::effect::AccessKind::None;
                                    }
                                    SemanticType::Reference(_, is_mut, _) => {
                                        // Safe Mellis reference contract
                                        escape_kind = crate::effect::EscapeKind::NoEscape;
                                        access_kind = if *is_mut == mellis_semantic::ty::Mutability::Mutable { 
                                            crate::effect::AccessKind::ReadWrite 
                                        } else { 
                                            crate::effect::AccessKind::Read 
                                        };
                                    }
                                    SemanticType::Pointer(is_mut, _) => {
                                        // Conservative C pointer contract
                                        escape_kind = if has_sync_noescape {
                                            crate::effect::EscapeKind::NoEscape
                                        } else {
                                            crate::effect::EscapeKind::MayEscape
                                        };
                                        access_kind = if *is_mut == mellis_semantic::ty::Mutability::Mutable { 
                                            crate::effect::AccessKind::ReadWrite 
                                        } else { 
                                            crate::effect::AccessKind::Read 
                                        };
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            escape_kind = crate::effect::EscapeKind::NoEscape;
                            access_kind = crate::effect::AccessKind::None;
                        }

                        // Union the callback's worst effect into this argument's access and escape
                        access_kind = access_kind.merge(&cb_worst_access);
                        if escape_kind != crate::effect::EscapeKind::Unknown {
                             escape_kind = escape_kind.merge(&cb_worst_escape);
                        }

                        if escape_kind == crate::effect::EscapeKind::MayEscape || escape_kind == crate::effect::EscapeKind::Unknown {
                            if let Operand::Value(arg_v) = arg {
                                if let Some(prov) = state.provenance.get(arg_v).cloned() {
                                    state.escaped_loans.extend(prov);
                                }
                            }
                        }

                        if access_kind == crate::effect::AccessKind::Read || access_kind == crate::effect::AccessKind::ReadWrite {
                            self.check_access(arg, false, val_id, state);
                        }
                        if access_kind == crate::effect::AccessKind::Write || access_kind == crate::effect::AccessKind::ReadWrite {
                            self.check_access(arg, true, val_id, state);
                        }
                    }
                }
            }
            Instruction::Borrow { is_rw, base, .. } => {
                self.issue_loan(base, *is_rw, val_id, state);
            }
            Instruction::Add { left, right, .. }
            | Instruction::Sub { left, right, .. }
            | Instruction::Mul { left, right, .. }
            | Instruction::Eq { left, right, .. } => {
                self.check_access(left, false, val_id, state);
                self.check_access(right, false, val_id, state);
            }
            Instruction::Drop { value } => {
                self.check_access(value, true, val_id, state);
            }
            Instruction::Alloca | _ => {}
        }
    }

    fn transfer_terminator(&mut self, term: &Terminator, state: &mut BorrowStateData) {
        // We don't have val_id for terminator, but we can assume it doesn't conflict directly unless it uses a value.
        // Wait, check_access needs a val_id to know active loans.
        // For terminators, active loans are those live BEFORE the terminator.
        // Wait, terminators don't have val_id!
        // So we can't use check_access easily.
        // But we don't strictly need to check access for Return/Br, they just read the value.
    }

    fn merge(&mut self, dest: &mut BorrowStateData, src: &BorrowStateData) -> bool {
        let mut changed = false;
        for (v, loans) in &src.provenance {
            let dest_loans = dest.provenance.entry(*v).or_default();
            for loan in loans {
                if dest_loans.insert(loan.clone()) {
                    changed = true;
                }
            }
        }
        for loan in &src.escaped_loans {
            if dest.escaped_loans.insert(loan.clone()) {
                changed = true;
            }
        }
        changed
    }
}
