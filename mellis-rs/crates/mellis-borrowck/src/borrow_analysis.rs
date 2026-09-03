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
            Instruction::MakeClosure { env_ptr, captures, .. } => {
                if let Operand::Value(v) = env_ptr { state.live.insert(*v); }
                for capture in captures { state.live.insert(capture.source); }
            }
            Instruction::MakeTraitObject { data_ptr, .. } => {
                if let Operand::Value(v) = data_ptr { state.live.insert(*v); }
            }
            Instruction::CallVirt { obj, args, .. } => {
                if let Operand::Value(v) = obj { state.live.insert(*v); }
                for arg in args {
                    if let Operand::Value(v) = arg { state.live.insert(*v); }
                }
            }
            Instruction::Store { ptr, value } => {
                if let Operand::Value(v) = ptr { state.live.insert(*v); }
                if let Operand::Value(v) = value { state.live.insert(*v); }
            }
            Instruction::Borrow { base, .. } => {
                if let Operand::Value(v) = base { state.live.insert(*v); }
            }
            Instruction::CallDirect { args, .. } => {
                for arg in args {
                    if let Operand::Value(v) = arg { state.live.insert(*v); }
                }
            }
            Instruction::CallIndirect { callee, args } | Instruction::CallClosure { closure: callee, args } => {
                if let Operand::Value(v) = callee { state.live.insert(*v); }
                for arg in args {
                    if let Operand::Value(v) = arg { state.live.insert(*v); }
                }
            }
            Instruction::Add { left, right } | Instruction::Sub { left, right } | Instruction::Mul { left, right } | Instruction::Div { left, right } | Instruction::Rem { left, right } | Instruction::Eq { left, right } | Instruction::LessThan { left, right } | Instruction::LessOrEq { left, right } | Instruction::GreaterThan { left, right } | Instruction::GreaterOrEq { left, right } => {
                if let Operand::Value(v) = left { state.live.insert(*v); }
                if let Operand::Value(v) = right { state.live.insert(*v); }
            }
            Instruction::Drop { value, .. } => {
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
    pub direct_provenance: HashMap<ValueId, HashSet<Loan>>,
    pub carried_provenance: HashMap<ValueId, HashSet<Loan>>,
    pub escaped_loans: HashSet<Loan>,
    pub aliases: HashMap<ValueId, Operand>,
    pub closure_captures: HashMap<ValueId, Vec<mellis_semantic::CaptureMode>>,
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
            if let Some(term) = &block.terminator {
                analyzer.transfer_terminator(term, &mut current_state);
            }
        }
        
        analyzer.diagnostics
    }

    fn resolve_alias<'b>(&self, op: &'b Operand, state: &'b BorrowStateData) -> &'b Operand {
        let mut current = op;
        while let Operand::Value(v) = current {
            if let Some(alias) = state.aliases.get(v) {
                current = alias;
            } else {
                break;
            }
        }
        current
    }

    fn active_loans(&self, val_id: ValueId, state: &BorrowStateData) -> HashSet<Loan> {
        let mut active = state.escaped_loans.clone();
        let mut queue = Vec::new();
        
        if let Some(live_set) = self.live_before.get(&val_id) {
            for &v in live_set {
                queue.push(v);
            }
        }
        
        let mut visited = std::collections::HashSet::new();

        while let Some(v) = queue.pop() {
            if visited.insert(v) {
                if let Some(loans) = state.direct_provenance.get(&v) {
                    for loan in loans {
                        active.insert(loan.clone());
                        let resolved = self.resolve_alias(&loan.place, state);
                        if let Operand::Value(place_v) = resolved {
                            queue.push(*place_v);
                        }
                    }
                }
                if let Some(loans) = state.carried_provenance.get(&v) {
                    for loan in loans {
                        active.insert(loan.clone());
                        let resolved = self.resolve_alias(&loan.place, state);
                        if let Operand::Value(place_v) = resolved {
                            queue.push(*place_v);
                        }
                    }
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
                    let mut diag = Diagnostic::error(format!(
                        "Cannot access '{}' because it is borrowed as &rw",
                        place_name
                    ));
                    diag.span = self.func.values[val_id.0 as usize].span.clone();
                    self.diagnostics.push(diag);
                } else if is_write {
                    let mut diag = Diagnostic::error(format!(
                        "Cannot write to '{}' because it is borrowed as &",
                        place_name
                    ));
                    diag.span = self.func.values[val_id.0 as usize].span.clone();
                    self.diagnostics.push(diag);
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
                        let mut diag = Diagnostic::error(format!(
                            "Cannot borrow '{}' as {} because it is already borrowed as &rw",
                            place_name,
                            if is_rw { "&rw" } else { "&" }
                        ));
                        diag.span = self.func.values[val_id.0 as usize].span.clone();
                        self.diagnostics.push(diag);
                    } else if is_rw {
                        let mut diag = Diagnostic::error(format!(
                            "Cannot borrow '{}' as &rw because it is already borrowed as &",
                            place_name
                        ));
                        diag.span = self.func.values[val_id.0 as usize].span.clone();
                        self.diagnostics.push(diag);
                    }
                }
            }
        }

        let new_loan = Loan {
            id: val_id,
            place: place.clone(),
            is_rw,
        };
        
        state.direct_provenance.entry(val_id).or_default().insert(new_loan);
    }
}

fn print_operand_name(op: &Operand) -> String {
    match op {
        Operand::Value(val) => format!("%v{}", val.0),
        Operand::Global(id) => format!("@g{}", id.name.clone()),
        Operand::Block(id) => format!("block_{}", id.0),
        Operand::Number(n) => n.clone(),
        Operand::Boolean(b) => b.to_string(),
        Operand::StringRef(s) => format!("\"{}\"", s),
        Operand::Char(c) => format!("'{}'", c),
    }
}

impl<'a> DataflowAnalysis<BorrowStateData> for BorrowAnalyzer<'a> {
    fn init_entry_state(&mut self, _func: &Function, _state: &mut BorrowStateData) {}

    fn transfer_instruction(&mut self, val_id: ValueId, inst: &Instruction, state: &mut BorrowStateData) {
        match inst {
            Instruction::MakeClosure { env_ptr, captures, .. } => {
                self.check_access(env_ptr, false, val_id, state);
                state.closure_captures.insert(val_id, captures.iter().map(|capture| capture.mode).collect());
                for capture in captures {
                    if matches!(capture.mode, mellis_semantic::CaptureMode::SharedBorrow | mellis_semantic::CaptureMode::MutableBorrow) {
                        let source = Operand::Value(capture.source);
                        self.issue_loan(&source, capture.mode == mellis_semantic::CaptureMode::MutableBorrow, val_id, state);
                    }
                }
            }
            Instruction::MakeTraitObject { data_ptr, .. } => {
                self.check_access(data_ptr, false, val_id, state);
            }
            Instruction::CallVirt { obj, args, .. } => {
                self.check_access(obj, false, val_id, state);
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                }
            }
            Instruction::Store { ptr, value } => {
                self.check_access(value, false, val_id, state);
                self.check_access(ptr, true, val_id, state);
                
                // If ptr is a Value, it inherits provenance from value (for alias tracking in borrowck)
                // Actually, if we store a reference into a local, that local now holds the provenance.
                // In NLL, if we do `*ptr = value`, we are overwriting `ptr`. But wait, `Instruction::Store` in MVIR is usually `Store { ptr: %v1, value: %v2 }`.
                // If %v1 is an Alloca, %v1 now contains the provenance of %v2.
                // To keep it simple, if ptr is Operand::Value, we add provenance of value to ptr.
                if let Operand::Value(ptr_val) = ptr {
                    let mut resolved_ptr = *ptr_val;
                    let mut current_op = Operand::Value(*ptr_val);
                    while let Operand::Value(v) = current_op {
                        if let Some(alias) = state.aliases.get(&v) {
                            current_op = alias.clone();
                            if let Operand::Value(alias_v) = current_op {
                                resolved_ptr = alias_v;
                            }
                        } else {
                            break;
                        }
                    }
                    
                    if let Operand::Value(val_v) = value {
                        if let Some(prov) = state.direct_provenance.get(val_v).cloned() {
                            state.direct_provenance.entry(resolved_ptr).or_default().extend(prov);
                        }
                    }
                }
            }
            Instruction::Load { ptr } => {
                self.check_access(ptr, false, val_id, state);
                // Load from a pointer inherits its provenance
                if let Operand::Value(ptr_val) = ptr {
                    if let Some(prov) = state.direct_provenance.get(ptr_val).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                }
            }
            Instruction::CallDirect { args, callee, .. } => {
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                }
                
                let mut applied_summary = None;
                if let Some(map) = self.callee_summaries {
                    if let Some(sum) = map.get(callee) {
                        applied_summary = Some(sum.clone());
                    }
                }
                if callee.name == "register_callback" {
                }
                
                if let Some(sum) = applied_summary {
                    for (i, arg) in args.iter().enumerate() {
                        if i < sum.args.len() {
                            let arg_effect = &sum.args[i];
                            
                            // Check specific access (write access if required)
                            if arg_effect.access == crate::effect::AccessKind::Write || arg_effect.access == crate::effect::AccessKind::ReadWrite {
                                self.check_access(arg, true, val_id, state);
                            }
                            
                            // Handle escapes
                            if arg_effect.escape == crate::effect::EscapeKind::MayEscape {
                                if let Operand::Value(arg_v) = arg {
                                    if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                        state.escaped_loans.extend(prov);
                                    }
                                    if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
                                        state.escaped_loans.extend(prov);
                                    }
                                }
                            }
                        }
                    }
                    
                    // Return provenance
                    match &sum.ret {
                        ReturnEffect::BorrowsFrom(indices) => {
                            for &idx in indices {
                                if idx < args.len() {
                                    if let Operand::Value(arg_v) = &args[idx] {
                                        if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                                        }
                                    }
                                }
                            }
                        }
                        ReturnEffect::BorrowsCarried(indices) => {
                            for &idx in indices {
                                if idx < args.len() {
                                    if let Operand::Value(arg_v) = &args[idx] {
                                        if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
                                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                                        }
                                        if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Opaque call conservative fallback using CE6 ABI rules
                    let mut callee_sym_id = None;
                    if let Some(sym_id) = callee.symbol_id {
                        callee_sym_id = Some(sym_id);
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
                                    SemanticType::Struct(_, _, _) | SemanticType::Enum(_, _, _) | SemanticType::Tuple(_) | SemanticType::Array(_, _) | SemanticType::Slice(_) => {
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

                        if escape_kind == crate::effect::EscapeKind::MayEscape || escape_kind == crate::effect::EscapeKind::Unknown {
                            if let Operand::Value(arg_v) = arg {
                                if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                    state.escaped_loans.extend(prov);
                                } else {
                                }
                                if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
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
            Instruction::CallIndirect { args, callee } | Instruction::CallClosure { args, closure: callee } => {
                self.check_access(callee, false, val_id, state);
                for arg in args {
                    self.check_access(arg, false, val_id, state);
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
                                cb_worst_access = crate::effect::AccessKind::Unknown;
                                cb_worst_escape = crate::effect::EscapeKind::Unknown;
                            }
                        }
                    } else if let Operand::Value(arg_v) = arg {
                        if let Some(ctx) = self.ctx {
                            let val_data = self.func.value(*arg_v);
                            if let mellis_semantic::ty::SemanticType::Function { .. } = ctx.types.get(val_data.ty) {
                                cb_worst_access = crate::effect::AccessKind::Unknown;
                                cb_worst_escape = crate::effect::EscapeKind::Unknown;
                            }
                        }
                    }
                }

                for arg in args.iter() {
                    let mut escape_kind = crate::effect::EscapeKind::Unknown;
                    let mut access_kind = crate::effect::AccessKind::ReadWrite;

                    if let Operand::Value(arg_v) = arg {
                        if let Some(ctx) = self.ctx {
                            let val_data = self.func.value(*arg_v);
                            use mellis_semantic::ty::SemanticType;
                            match ctx.types.get(val_data.ty) {
                                SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => {
                                    escape_kind = crate::effect::EscapeKind::NoEscape;
                                    access_kind = crate::effect::AccessKind::None;
                                }
                                SemanticType::Struct(_, _, _) | SemanticType::Enum(_, _, _) | SemanticType::Tuple(_) | SemanticType::Array(_, _) | SemanticType::Slice(_) => {
                                    escape_kind = crate::effect::EscapeKind::NoEscape;
                                    access_kind = crate::effect::AccessKind::None;
                                }
                                SemanticType::Reference(_, is_mut, _) => {
                                    escape_kind = crate::effect::EscapeKind::NoEscape;
                                    access_kind = if *is_mut == mellis_semantic::ty::Mutability::Mutable { 
                                        crate::effect::AccessKind::ReadWrite 
                                    } else { 
                                        crate::effect::AccessKind::Read 
                                    };
                                }
                                SemanticType::Pointer(is_mut, _) => {
                                    escape_kind = crate::effect::EscapeKind::MayEscape;
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
                            if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                state.escaped_loans.extend(prov);
                            }
                            if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
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
            Instruction::Borrow { is_rw, base } => {
                // Issue a loan
                self.issue_loan(base, *is_rw, val_id, state);
                
                // If base has provenance, the borrow carries it
                if let Operand::Value(base_v) = base {
                    let mut current = *base_v;
                    let mut resolved_base = *base_v;
                    while let Some(alias) = state.aliases.get(&current) {
                        if let Operand::Value(alias_v) = alias {
                            resolved_base = *alias_v;
                            current = *alias_v;
                        } else { break; }
                    }
                    if let Some(prov) = state.direct_provenance.get(&resolved_base).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(&resolved_base).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    state.aliases.insert(val_id, Operand::Value(*base_v));
                }
            }
            Instruction::Extract { .. } | Instruction::Tag { .. } => {}
            Instruction::FieldPtr { base, .. } => {
                if let Operand::Value(b) = base {
                    state.aliases.insert(val_id, Operand::Value(*b));
                }
            }
            Instruction::Add { left, right, .. }
            | Instruction::Sub { left, right, .. }
            | Instruction::Mul { left, right, .. }
            | Instruction::Div { left, right, .. }
            | Instruction::Rem { left, right, .. }
            | Instruction::Eq { left, right, .. }
            | Instruction::LessThan { left, right, .. }
            | Instruction::LessOrEq { left, right, .. }
            | Instruction::GreaterThan { left, right, .. }
            | Instruction::GreaterOrEq { left, right, .. } => {
                self.check_access(left, false, val_id, state);
                self.check_access(right, false, val_id, state);
            }
            Instruction::Drop { value, .. } => {
                self.check_access(value, true, val_id, state);
            }
            Instruction::Alloca | _ => {}
        }
    }

    fn transfer_terminator(&mut self, term: &Terminator, state: &mut BorrowStateData) {
        if !self.emit_diagnostics {
            return;
        }
        if let Terminator::Ret { value: Some(Operand::Value(value)) } = term {
            if let Some(modes) = state.closure_captures.get(value) {
                if modes.iter().any(|mode| matches!(mode, mellis_semantic::CaptureMode::SharedBorrow | mellis_semantic::CaptureMode::MutableBorrow)) {
                    let mut diag = Diagnostic::error("Cannot return a closure that captures a local borrow");
                    diag.span = self.func.value(*value).span.clone();
                    if !self.diagnostics.iter().any(|existing| existing.message == diag.message) {
                        self.diagnostics.push(diag);
                    }
                }
            }
        }
    }

    fn merge(&mut self, dest: &mut BorrowStateData, src: &BorrowStateData) -> bool {
        let mut changed = false;
        for (v, loans) in &src.direct_provenance {
            let dest_loans = dest.direct_provenance.entry(*v).or_default();
            for loan in loans {
                if dest_loans.insert(loan.clone()) {
                    changed = true;
                }
            }
        }
        for (v, loans) in &src.carried_provenance {
            let dest_loans = dest.carried_provenance.entry(*v).or_default();
            for loan in loans {
                if dest_loans.insert(loan.clone()) {
                    changed = true;
                }
            }
        }
        for (v, alias) in &src.aliases {
            if dest.aliases.insert(*v, alias.clone()) != Some(alias.clone()) {
                changed = true;
            }
        }
        for loan in &src.escaped_loans {
            if dest.escaped_loans.insert(loan.clone()) {
                changed = true;
            }
        }
        for (closure, modes) in &src.closure_captures {
            if dest.closure_captures.insert(*closure, modes.clone()).is_none() {
                changed = true;
            }
        }
        changed
    }
}
