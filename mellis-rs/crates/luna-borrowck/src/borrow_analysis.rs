use crate::dataflow::{DataflowAnalysis, DataflowEngine};
use crate::effect::{CallEffectSummary, EscapeKind, ReturnEffect};
use luna_common::Diagnostic;
use luna_mvir::{Function, GlobalId, Instruction, Operand, Terminator, ValueId, ValueOrigin};
use luna_semantic::{SemanticContext, SemanticType};
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
            Instruction::MakeSlice { data_ptr, len } => {
                if let Operand::Value(v) = data_ptr { state.live.insert(*v); }
                if let Operand::Value(v) = len { state.live.insert(*v); }
            }
            Instruction::DropVirt { obj } => {
                if let Operand::Value(v) = obj { state.live.insert(*v); }
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
            Instruction::Await { future } => {
                if let Operand::Value(v) = future { state.live.insert(*v); }
            }
            Instruction::Assign(op) | Instruction::Cast { value: op, .. } | Instruction::FieldPtr { base: op, .. } | Instruction::Extract { value: op, .. } | Instruction::Tag { value: op } => {
                if let Operand::Value(v) = op { state.live.insert(*v); }
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

pub fn compute_liveness(func: &Function) -> (HashMap<String, LivenessState>, HashMap<ValueId, HashSet<ValueId>>, HashMap<ValueId, HashSet<ValueId>>) {
    let mut analyzer = LivenessAnalyzer;
    // run_backward gives us the OUT state of each block
    let block_out = DataflowEngine::run_backward(func, &mut analyzer);
    
    // Now compute liveness BEFORE and AFTER each instruction
    let mut live_before = HashMap::new();
    let mut live_after = HashMap::new();
    
    for block in &func.blocks {
        let mut current_state = block_out.get(&block.label.name).cloned().unwrap_or_default();
        if let Some(term) = &block.terminator {
            analyzer.transfer_terminator(term, &mut current_state);
        }
        
        for &val_id in block.insts.iter().rev() {
            live_after.insert(val_id, current_state.live.clone());
            analyzer.transfer_instruction(val_id, &func.value(val_id).inst, &mut current_state);
            live_before.insert(val_id, current_state.live.clone());
        }
    }
    
    (block_out, live_before, live_after)
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
    pub closure_captures: HashMap<ValueId, Vec<luna_semantic::CaptureMode>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LifetimeRoot {
    Static,
    Parameter(u16),
    Local(ValueId),
}

pub struct BorrowAnalyzer<'a> {
    pub diagnostics: Vec<Diagnostic>,
    live_before: HashMap<ValueId, HashSet<ValueId>>,
    live_after: HashMap<ValueId, HashSet<ValueId>>,
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
            live_after: HashMap::new(),
            callee_summaries,
            ctx,
            func,
            emit_diagnostics: false,
        }
    }

    pub fn with_live_after(mut self, live_after: HashMap<ValueId, HashSet<ValueId>>) -> Self {
        self.live_after = live_after;
        self
    }

    pub fn analyze(func: &'a Function, summaries: Option<&'a HashMap<GlobalId, CallEffectSummary>>, ctx: Option<&'a SemanticContext>) -> Vec<Diagnostic> {
        let (_, live_before, live_after) = compute_liveness(func);
        let mut analyzer = Self::new(live_before, summaries, ctx, func).with_live_after(live_after);
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

    fn resolve_argument_roots(&self, arg: &Operand, state: &BorrowStateData) -> HashSet<LifetimeRoot> {
        let mut roots = HashSet::new();
        let resolved_op = self.resolve_alias(arg, state);

        match resolved_op {
            Operand::Global(_) => {
                roots.insert(LifetimeRoot::Static);
                return roots;
            }
            Operand::Value(val) => {
                let mut loans = HashSet::new();
                if let Some(prov) = state.direct_provenance.get(val) {
                    loans.extend(prov.clone());
                }
                if let Some(prov) = state.carried_provenance.get(val) {
                    loans.extend(prov.clone());
                }

                let mut curr = *val;
                while let Some(alias) = state.aliases.get(&curr) {
                    if let Operand::Value(av) = alias {
                        if let Some(prov) = state.direct_provenance.get(av) {
                            loans.extend(prov.clone());
                        }
                        if let Some(prov) = state.carried_provenance.get(av) {
                            loans.extend(prov.clone());
                        }
                        curr = *av;
                    } else {
                        break;
                    }
                }

                for loan in &loans {
                    let resolved_place = self.resolve_alias(&loan.place, state);
                    match resolved_place {
                        Operand::Global(_) => {
                            roots.insert(LifetimeRoot::Static);
                        }
                        Operand::Value(pv) => {
                            if (pv.0 as usize) < self.func.values.len() {
                                let val_data = &self.func.values[pv.0 as usize];
                                match val_data.origin {
                                    ValueOrigin::Global => {
                                        roots.insert(LifetimeRoot::Static);
                                    }
                                    ValueOrigin::Parameter(p_idx) => {
                                        roots.insert(LifetimeRoot::Parameter(p_idx as u16));
                                    }
                                    ValueOrigin::Local | ValueOrigin::Temporary => {
                                        roots.insert(LifetimeRoot::Local(*pv));
                                    }
                                }
                            } else {
                                roots.insert(LifetimeRoot::Local(*pv));
                            }
                        }
                        _ => {}
                    }
                }

                if roots.is_empty() {
                    if (val.0 as usize) < self.func.values.len() {
                        let val_data = &self.func.values[val.0 as usize];
                        match val_data.origin {
                            ValueOrigin::Global => {
                                roots.insert(LifetimeRoot::Static);
                            }
                            ValueOrigin::Parameter(p_idx) => {
                                roots.insert(LifetimeRoot::Parameter(p_idx as u16));
                            }
                            ValueOrigin::Local | ValueOrigin::Temporary => {
                                roots.insert(LifetimeRoot::Local(*val));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        roots
    }

    fn root_outlives(
        &self,
        longer: &LifetimeRoot,
        shorter: &LifetimeRoot,
        caller_contract: Option<&luna_semantic::CanonicalLifetimeContract>,
    ) -> bool {
        if longer == shorter {
            return true;
        }
        match (longer, shorter) {
            (LifetimeRoot::Static, _) => true,
            (_, LifetimeRoot::Static) => false,
            (LifetimeRoot::Parameter(_), LifetimeRoot::Local(_)) => true,
            (LifetimeRoot::Local(_), LifetimeRoot::Parameter(_)) => false,
            (LifetimeRoot::Parameter(p_long), LifetimeRoot::Parameter(p_short)) => {
                if let Some(contract) = caller_contract {
                    contract.outlives_holds(*p_long, *p_short)
                } else {
                    false
                }
            }
            (LifetimeRoot::Local(v_long), LifetimeRoot::Local(v_short)) => {
                v_long.0 <= v_short.0
            }
        }
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

    fn active_loans_after(&self, val_id: ValueId, state: &BorrowStateData) -> HashSet<Loan> {
        let mut active = state.escaped_loans.clone();
        let mut queue = Vec::new();
        
        if let Some(live_set) = self.live_after.get(&val_id) {
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

    fn is_external_loan(&self, loan: &Loan, state: &BorrowStateData) -> bool {
        let resolved = self.resolve_alias(&loan.place, state);
        match resolved {
            Operand::Global(_) => true,
            Operand::Value(v) => {
                if (v.0 as usize) >= self.func.values.len() {
                    return false;
                }
                let val_data = &self.func.values[v.0 as usize];
                match &val_data.origin {
                    ValueOrigin::Global => true,
                    ValueOrigin::Parameter(_) => {
                        // In Mellis, a parameter loan is external only if the parameter type is a Reference
                        // (i.e. caller storage). By-value parameters are stored in EnvStruct (local).
                        if let Some(ctx) = self.ctx {
                            matches!(ctx.types.get(val_data.ty), SemanticType::Reference(..))
                        } else {
                            false
                        }
                    }
                    ValueOrigin::Local | ValueOrigin::Temporary => false,
                }
            }
            _ => false,
        }
    }

    fn check_access(&mut self, place: &Operand, is_write: bool, val_id: ValueId, state: &BorrowStateData) {
        if !self.emit_diagnostics { return; }
        let resolved_place = self.resolve_alias(place, state);
        let place_name = print_operand_name(resolved_place);
        let active = self.active_loans(val_id, state);
        
        for loan in &active {
            if self.is_external_loan(loan, state) {
                continue;
            }
            // If the place being accessed is itself a reference holding this loan,
            // it is accessing through the borrow, not conflicting with it.
            if let Operand::Value(pv) = place {
                if let Some(prov) = state.direct_provenance.get(pv) {
                    if prov.contains(loan) {
                        continue;
                    }
                }
                if let Some(prov) = state.carried_provenance.get(pv) {
                    if prov.contains(loan) {
                        continue;
                    }
                }
            }
            let resolved_loan_place = self.resolve_alias(&loan.place, state);
            if print_operand_name(resolved_loan_place) == place_name {
                if loan.is_rw {
                    let mut diag = Diagnostic::error(format!(
                        "Cannot access '{}' because it is borrowed as &rw",
                        place_name
                    ));
                    diag.span = self.func.values[val_id.0 as usize].span.clone();
                    if !self.diagnostics.iter().any(|d| d.message == diag.message && d.span == diag.span) {
                        self.diagnostics.push(diag);
                    }
                } else if is_write {
                    let mut diag = Diagnostic::error(format!(
                        "Cannot write to '{}' because it is borrowed as &",
                        place_name
                    ));
                    diag.span = self.func.values[val_id.0 as usize].span.clone();
                    if !self.diagnostics.iter().any(|d| d.message == diag.message && d.span == diag.span) {
                        self.diagnostics.push(diag);
                    }
                }
            }
        }
    }

    fn issue_loan(&mut self, place: &Operand, is_rw: bool, val_id: ValueId, state: &mut BorrowStateData) {
        let resolved_place = self.resolve_alias(place, state);
        let place_name = print_operand_name(resolved_place);
        let active = self.active_loans(val_id, state);
        
        if self.emit_diagnostics {
            for loan in &active {
                if self.is_external_loan(loan, state) {
                    continue;
                }
                let resolved_loan_place = self.resolve_alias(&loan.place, state);
                if print_operand_name(resolved_loan_place) == place_name {
                    if loan.is_rw {
                        let mut diag = Diagnostic::error(format!(
                            "Cannot borrow '{}' as {} because it is already borrowed as &rw",
                            place_name,
                            if is_rw { "&rw" } else { "&" }
                        ));
                        diag.span = self.func.values[val_id.0 as usize].span.clone();
                        if !self.diagnostics.iter().any(|d| d.message == diag.message && d.span == diag.span) {
                            self.diagnostics.push(diag);
                        }
                    } else if is_rw {
                        let mut diag = Diagnostic::error(format!(
                            "Cannot borrow '{}' as &rw because it is already borrowed as &",
                            place_name
                        ));
                        diag.span = self.func.values[val_id.0 as usize].span.clone();
                        if !self.diagnostics.iter().any(|d| d.message == diag.message && d.span == diag.span) {
                            self.diagnostics.push(diag);
                        }
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
    fn init_entry_state(&mut self, func: &Function, state: &mut BorrowStateData) {
        for (i, val_data) in func.values.iter().enumerate() {
            if let ValueOrigin::Parameter(_) = val_data.origin {
                let is_ref = if let Some(ctx) = self.ctx {
                    matches!(ctx.types.get(val_data.ty), SemanticType::Reference(..))
                } else {
                    false
                };
                if is_ref {
                    let is_rw = if let Some(ctx) = self.ctx {
                        matches!(
                            ctx.types.get(val_data.ty),
                            SemanticType::Reference(_, luna_semantic::ty::Mutability::Mutable, _)
                        )
                    } else {
                        false
                    };
                    let val_id = ValueId(i as u32);
                    let loan = Loan {
                        id: val_id,
                        place: Operand::Value(val_id),
                        is_rw,
                    };
                    state.direct_provenance.entry(val_id).or_default().insert(loan);
                }
            }
        }
    }

    fn transfer_instruction(&mut self, val_id: ValueId, inst: &Instruction, state: &mut BorrowStateData) {
        match inst {
            Instruction::MakeClosure { env_ptr, captures, .. } => {
                self.check_access(env_ptr, false, val_id, state);
                state.closure_captures.insert(val_id, captures.iter().map(|capture| capture.mode).collect());
                for capture in captures {
                    if matches!(capture.mode, luna_semantic::CaptureMode::SharedBorrow | luna_semantic::CaptureMode::MutableBorrow) {
                        let source = Operand::Value(capture.source);
                        self.issue_loan(&source, capture.mode == luna_semantic::CaptureMode::MutableBorrow, val_id, state);
                    }
                }
            }
            Instruction::MakeTraitObject { data_ptr, .. } => {
                self.check_access(data_ptr, false, val_id, state);
                if let Operand::Value(dp_v) = data_ptr {
                    let resolved = self.resolve_alias(data_ptr, state);
                    let check_val = if let Operand::Value(rv) = resolved { *rv } else { *dp_v };
                    state.aliases.insert(val_id, Operand::Value(check_val));
                    if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.direct_provenance.get(dp_v).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(dp_v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                }
            }
            Instruction::MakeSlice { data_ptr, len } => {
                self.check_access(data_ptr, false, val_id, state);
                self.check_access(len, false, val_id, state);
                if let Operand::Value(dp_v) = data_ptr {
                    let resolved = self.resolve_alias(data_ptr, state);
                    let check_val = if let Operand::Value(rv) = resolved { *rv } else { *dp_v };
                    state.aliases.insert(val_id, Operand::Value(check_val));
                    if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.direct_provenance.get(dp_v).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(dp_v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                }
            }
            Instruction::DropVirt { obj } => {
                self.check_access(obj, false, val_id, state);
            }
            Instruction::CallVirt { obj, method_idx, args } => {
                let mut is_write = false;
                let mut trait_method_sym = None;
                if let Some(ctx) = self.ctx {
                    if let Operand::Value(obj_v) = obj {
                        let obj_ty_id = ctx.types.resolve(self.func.values[obj_v.0 as usize].ty);
                        let obj_ty = ctx.types.get(obj_ty_id);
                        let dyn_trait_sym = match obj_ty {
                            SemanticType::DynTrait(sym) => Some(*sym),
                            SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                                if let SemanticType::DynTrait(sym) = ctx.types.get(*inner) {
                                    Some(*sym)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        if let Some(trait_sym) = dyn_trait_sym {
                            if let Some(method_syms) = ctx.tables.trait_methods.get(&trait_sym) {
                                if let Some(&m_sym) = method_syms.get(*method_idx as usize) {
                                    trait_method_sym = Some(m_sym);
                                    if let Some(&m_ty) = ctx.tables.symbol_types.get(&m_sym) {
                                        if let SemanticType::Function { params, .. } = ctx.types.get(m_ty) {
                                            if let Some(&first_param) = params.first() {
                                                if matches!(ctx.types.get(first_param), SemanticType::Reference(_, luna_semantic::ty::Mutability::Mutable, _)) {
                                                    is_write = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                self.check_access(obj, is_write, val_id, state);
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                }

                let is_future_ret = if let Some(ctx) = self.ctx {
                    matches!(ctx.types.get(self.func.values[val_id.0 as usize].ty), SemanticType::Future(..))
                } else {
                    false
                };

                if is_future_ret {
                    for op in std::iter::once(obj).chain(args.iter()) {
                        if let Operand::Value(op_v) = op {
                            let is_ref_bearing = if let Some(ctx) = self.ctx {
                                let ty_id = self.func.values[op_v.0 as usize].ty;
                                matches!(ctx.types.get(ty_id), SemanticType::Reference(..) | SemanticType::Pointer(..))
                            } else {
                                true
                            };
                            if is_ref_bearing {
                                let resolved = self.resolve_alias(op, state);
                                let resolved_v = if let Operand::Value(rv) = resolved { *rv } else { *op_v };
                                if let Some(prov) = state.direct_provenance.get(&resolved_v).cloned() {
                                    state.carried_provenance.entry(val_id).or_default().extend(prov);
                                }
                                if let Some(prov) = state.carried_provenance.get(&resolved_v).cloned() {
                                    state.carried_provenance.entry(val_id).or_default().extend(prov);
                                }
                                if let Some(prov) = state.direct_provenance.get(op_v).cloned() {
                                    state.carried_provenance.entry(val_id).or_default().extend(prov);
                                }
                                if let Some(prov) = state.carried_provenance.get(op_v).cloned() {
                                    state.carried_provenance.entry(val_id).or_default().extend(prov);
                                }
                            }
                        }
                    }
                }

                // Phase 3.6C: Apply CanonicalLifetimeContract from trait method
                if let Some(m_sym) = trait_method_sym {
                    if let Some(ctx) = self.ctx {
                        if let Some(contract) = ctx.tables.fn_lifetime_contracts.get(&m_sym) {
                            let all_args: Vec<&Operand> = std::iter::once(obj).chain(args.iter()).collect();

                            // Enforce outlives constraints at CallVirt call-site
                            if self.emit_diagnostics {
                                for constraint in &contract.outlives_constraints {
                                    let longer_idx = constraint.longer as usize;
                                    let shorter_idx = constraint.shorter as usize;
                                    if longer_idx < all_args.len() && shorter_idx < all_args.len() {
                                        let longer_roots = self.resolve_argument_roots(all_args[longer_idx], state);
                                        let shorter_roots = self.resolve_argument_roots(all_args[shorter_idx], state);

                                        let caller_sym_id = self.func.name.symbol_id.or_else(|| {
                                            ctx.symbol_table.lookup(&self.func.name.name, luna_semantic::symbol::ScopeId(0))
                                        });
                                        let caller_contract = caller_sym_id.and_then(|sym| {
                                            ctx.tables.fn_lifetime_contracts.get(&sym)
                                        });

                                        let mut satisfied = !longer_roots.is_empty() && !shorter_roots.is_empty();
                                        for l_root in &longer_roots {
                                            for s_root in &shorter_roots {
                                                if !self.root_outlives(l_root, s_root, caller_contract) {
                                                    satisfied = false;
                                                    break;
                                                }
                                            }
                                            if !satisfied {
                                                break;
                                            }
                                        }

                                        if !satisfied {
                                            let mut diag = Diagnostic::error(format!(
                                                "error[E2016]: LifetimeConstraintViolation: argument for parameter (index {}) does not outlive parameter (index {})",
                                                constraint.longer, constraint.shorter
                                            ));
                                            diag.span = self.func.value(val_id).span.clone();
                                            if !self.diagnostics.iter().any(|d| d.message == diag.message && d.span == diag.span) {
                                                self.diagnostics.push(diag);
                                            }
                                        }
                                    }
                                }
                            }

                            // Propagate return provenance from all_args
                            if let Some(prov) = &contract.return_provenance {
                                for &param_idx in prov.indices() {
                                    let idx = param_idx as usize;
                                    if idx < all_args.len() {
                                        if let Operand::Value(arg_v) = all_args[idx] {
                                            if let Some(p) = state.direct_provenance.get(arg_v).cloned() {
                                                state.direct_provenance.entry(val_id).or_default().extend(p);
                                            }
                                            if let Some(p) = state.carried_provenance.get(arg_v).cloned() {
                                                state.direct_provenance.entry(val_id).or_default().extend(p);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
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
                        if let Some(prov) = state.carried_provenance.get(val_v).cloned() {
                            state.carried_provenance.entry(resolved_ptr).or_default().extend(prov);
                        }
                    }
                }
            }
            Instruction::Load { ptr } => {
                self.check_access(ptr, false, val_id, state);
                // Load from a pointer inherits its provenance if loaded value can hold references
                let is_ref_like = if let Some(ctx) = self.ctx {
                    if (val_id.0 as usize) < self.func.values.len() {
                        matches!(
                            ctx.types.get(self.func.values[val_id.0 as usize].ty),
                            SemanticType::Reference(..)
                                | SemanticType::Pointer(..)
                                | SemanticType::Future(..)
                                | SemanticType::Struct(..)
                                | SemanticType::Enum(..)
                                | SemanticType::Tuple(..)
                        )
                    } else {
                        true
                    }
                } else {
                    true
                };
                if is_ref_like {
                    if let Operand::Value(ptr_val) = ptr {
                        let resolved = self.resolve_alias(ptr, state);
                        let check_val = if let Operand::Value(rv) = resolved { *rv } else { *ptr_val };
                        state.aliases.insert(val_id, Operand::Value(check_val));
                        if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.direct_provenance.get(ptr_val).cloned() {
                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.carried_provenance.get(ptr_val).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                    }
                }
            }
            Instruction::CallDirect { args, callee, .. } => {
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                }
                
                let is_future_ret = if let Some(ctx) = self.ctx {
                    matches!(ctx.types.get(self.func.values[val_id.0 as usize].ty), SemanticType::Future(..))
                } else {
                    false
                };

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
                            if !is_future_ret && arg_effect.escape == crate::effect::EscapeKind::MayEscape {
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
                                use luna_semantic::ty::SemanticType;
                                match ctx.types.get(val_data.ty) {
                                    SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => {
                                        escape_kind = crate::effect::EscapeKind::NoEscape;
                                        access_kind = crate::effect::AccessKind::None;
                                    }
                                    SemanticType::Struct(_, _, _) | SemanticType::Enum(_, _, _) | SemanticType::Tuple(_) | SemanticType::Array(_, _) | SemanticType::Slice(_) | SemanticType::Future(_) | SemanticType::Box(_) => {
                                        // Passed by value (or opaque copy), no memory effect on the caller's aliasing
                                        escape_kind = crate::effect::EscapeKind::NoEscape;
                                        access_kind = crate::effect::AccessKind::None;
                                    }
                                    SemanticType::Reference(_, is_mut, _) => {
                                        // Safe Mellis reference contract
                                        escape_kind = crate::effect::EscapeKind::NoEscape;
                                        access_kind = if *is_mut == luna_semantic::ty::Mutability::Mutable { 
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
                                        access_kind = if *is_mut == luna_semantic::ty::Mutability::Mutable { 
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

                        if !is_future_ret && (escape_kind == crate::effect::EscapeKind::MayEscape || escape_kind == crate::effect::EscapeKind::Unknown) {
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

                // Apply CanonicalLifetimeContract provenance if declared on callee
                let mut callee_sym_id = callee.symbol_id;
                if callee_sym_id.is_none() {
                    if let Some(ctx) = self.ctx {
                        if let Some(sym_id) = ctx.symbol_table.lookup(&callee.name, luna_semantic::symbol::ScopeId(0)) {
                            callee_sym_id = Some(sym_id);
                        }
                    }
                }
                if let Some(sym_id) = callee_sym_id {
                    if let Some(ctx) = self.ctx {
                        if let Some(contract) = ctx.tables.fn_lifetime_contracts.get(&sym_id) {
                            // Phase 3.6B: Enforce outlives constraints at call-site
                            if self.emit_diagnostics {
                                for constraint in &contract.outlives_constraints {
                                    let longer_idx = constraint.longer as usize;
                                    let shorter_idx = constraint.shorter as usize;
                                    if longer_idx < args.len() && shorter_idx < args.len() {
                                        let longer_roots = self.resolve_argument_roots(&args[longer_idx], state);
                                        let shorter_roots = self.resolve_argument_roots(&args[shorter_idx], state);

                                        let caller_sym_id = self.func.name.symbol_id.or_else(|| {
                                            ctx.symbol_table.lookup(&self.func.name.name, luna_semantic::symbol::ScopeId(0))
                                        });
                                        let caller_contract = caller_sym_id.and_then(|sym| {
                                            ctx.tables.fn_lifetime_contracts.get(&sym)
                                        });

                                        let mut satisfied = !longer_roots.is_empty() && !shorter_roots.is_empty();
                                        for l_root in &longer_roots {
                                            for s_root in &shorter_roots {
                                                if !self.root_outlives(l_root, s_root, caller_contract) {
                                                    satisfied = false;
                                                    break;
                                                }
                                            }
                                            if !satisfied {
                                                break;
                                            }
                                        }

                                        if !satisfied {
                                            let mut diag = Diagnostic::error(format!(
                                                "error[E2016]: LifetimeConstraintViolation: argument for parameter (index {}) does not outlive parameter (index {})",
                                                constraint.longer, constraint.shorter
                                            ));
                                            diag.span = self.func.value(val_id).span.clone();
                                            if !self.diagnostics.iter().any(|d| d.message == diag.message && d.span == diag.span) {
                                                self.diagnostics.push(diag);
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(prov) = &contract.return_provenance {
                                for &param_idx in prov.indices() {
                                    let idx = param_idx as usize;
                                    if idx < args.len() {
                                        if let Operand::Value(arg_v) = &args[idx] {
                                            if let Some(p) = state.direct_provenance.get(arg_v).cloned() {
                                                state.direct_provenance.entry(val_id).or_default().extend(p);
                                            }
                                            if let Some(p) = state.carried_provenance.get(arg_v).cloned() {
                                                state.direct_provenance.entry(val_id).or_default().extend(p);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // FutureLoan propagation for CallDirect:
                if is_future_ret {
                    for arg in args {
                        if let Operand::Value(arg_v) = arg {
                            let resolved = self.resolve_alias(arg, state);
                            let resolved_v = if let Operand::Value(rv) = resolved { *rv } else { *arg_v };
                            if let Some(prov) = state.direct_provenance.get(&resolved_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                            if let Some(prov) = state.carried_provenance.get(&resolved_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                            if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                            if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                        }
                    }
                }
            }
            Instruction::CallIndirect { args, callee } | Instruction::CallClosure { args, closure: callee } => {
                self.check_access(callee, false, val_id, state);
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                }
                
                let is_future_ret = if let Some(ctx) = self.ctx {
                    matches!(ctx.types.get(self.func.values[val_id.0 as usize].ty), SemanticType::Future(..))
                } else {
                    false
                };

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
                            if let luna_semantic::ty::SemanticType::Function { .. } = ctx.types.get(val_data.ty) {
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
                            use luna_semantic::ty::SemanticType;
                            match ctx.types.get(val_data.ty) {
                                SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => {
                                    escape_kind = crate::effect::EscapeKind::NoEscape;
                                    access_kind = crate::effect::AccessKind::None;
                                }
                                SemanticType::Struct(_, _, _) | SemanticType::Enum(_, _, _) | SemanticType::Tuple(_) | SemanticType::Array(_, _) | SemanticType::Slice(_) | SemanticType::Future(_) | SemanticType::Box(_) => {
                                    escape_kind = crate::effect::EscapeKind::NoEscape;
                                    access_kind = crate::effect::AccessKind::None;
                                }
                                SemanticType::Reference(_, is_mut, _) => {
                                    escape_kind = crate::effect::EscapeKind::NoEscape;
                                    access_kind = if *is_mut == luna_semantic::ty::Mutability::Mutable { 
                                        crate::effect::AccessKind::ReadWrite 
                                    } else { 
                                        crate::effect::AccessKind::Read 
                                    };
                                }
                                SemanticType::Pointer(is_mut, _) => {
                                    escape_kind = crate::effect::EscapeKind::MayEscape;
                                    access_kind = if *is_mut == luna_semantic::ty::Mutability::Mutable { 
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

                    if !is_future_ret && (escape_kind == crate::effect::EscapeKind::MayEscape || escape_kind == crate::effect::EscapeKind::Unknown) {
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

                // FutureLoan propagation for CallIndirect:
                if is_future_ret {
                    for arg in args {
                        if let Operand::Value(arg_v) = arg {
                            let resolved = self.resolve_alias(arg, state);
                            let resolved_v = if let Operand::Value(rv) = resolved { *rv } else { *arg_v };
                            if let Some(prov) = state.direct_provenance.get(&resolved_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                            if let Some(prov) = state.carried_provenance.get(&resolved_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                            if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                            if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
                                state.carried_provenance.entry(val_id).or_default().extend(prov);
                            }
                        }
                    }
                }
            }
            Instruction::Borrow { is_rw, base } => {
                let is_base_ptr_or_ref = if let Operand::Value(b_v) = base {
                    if let Some(ctx) = self.ctx {
                        if (b_v.0 as usize) < self.func.values.len() {
                            matches!(
                                ctx.types.get(self.func.values[b_v.0 as usize].ty),
                                SemanticType::Pointer(..) | SemanticType::Reference(..)
                            )
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !is_base_ptr_or_ref {
                    // Issue a loan on the variable/storage being borrowed
                    self.issue_loan(base, *is_rw, val_id, state);
                }
                
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
                    if let Some(prov) = state.direct_provenance.get(base_v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(base_v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    state.aliases.insert(val_id, Operand::Value(*base_v));
                }
            }
            Instruction::Variant { args, .. } => {
                for arg in args {
                    self.check_access(arg, false, val_id, state);
                    if let Operand::Value(arg_v) = arg {
                        let resolved = self.resolve_alias(arg, state);
                        let check_val = if let Operand::Value(rv) = resolved { *rv } else { *arg_v };
                        if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.direct_provenance.get(arg_v).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.carried_provenance.get(arg_v).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                    }
                }
            }
            Instruction::Extract { value, .. } => {
                self.check_access(value, false, val_id, state);
                if let Operand::Value(v) = value {
                    let resolved = self.resolve_alias(value, state);
                    let check_val = if let Operand::Value(rv) = resolved { *rv } else { *v };
                    if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.direct_provenance.get(v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                }
            }
            Instruction::Tag { .. } => {}
            Instruction::FieldPtr { base, .. } => {
                if let Operand::Value(b) = base {
                    state.aliases.insert(val_id, Operand::Value(*b));
                }
            }
            Instruction::Add { left, right, .. }
            | Instruction::Sub { left, right, .. } => {
                self.check_access(left, false, val_id, state);
                self.check_access(right, false, val_id, state);
                for op in [left, right] {
                    if let Operand::Value(v) = op {
                        let resolved = self.resolve_alias(op, state);
                        let check_val = if let Operand::Value(rv) = resolved { *rv } else { *v };
                        if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.direct_provenance.get(v).cloned() {
                            state.direct_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if let Some(prov) = state.carried_provenance.get(v).cloned() {
                            state.carried_provenance.entry(val_id).or_default().extend(prov);
                        }
                        if !state.direct_provenance.get(&check_val).map_or(true, |s| s.is_empty())
                            || !state.carried_provenance.get(&check_val).map_or(true, |s| s.is_empty())
                        {
                            state.aliases.insert(val_id, Operand::Value(check_val));
                        }
                    }
                }
            }
            Instruction::Mul { left, right, .. }
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
            Instruction::Await { future } => {
                self.check_access(future, false, val_id, state);
                if self.emit_diagnostics {
                    let active = self.active_loans_after(val_id, state);
                    for loan in &active {
                        if !self.is_external_loan(loan, state) {
                            let resolved = self.resolve_alias(&loan.place, state);
                            let place_name = print_operand_name(resolved);
                            let mut diag = Diagnostic::error(format!(
                                "Borrow of local variable '{}' is held across an 'await' point [E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT]. Self-referential futures are not permitted.",
                                place_name
                            ));
                            diag.span = self.func.values[val_id.0 as usize].span.clone();
                            if !self.diagnostics.iter().any(|d| d.message == diag.message) {
                                self.diagnostics.push(diag);
                            }
                        }
                    }
                }
            }
            Instruction::Assign(op) => {
                if let Operand::Value(v) = op {
                    state.aliases.insert(val_id, Operand::Value(*v));
                    if let Some(prov) = state.direct_provenance.get(v).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                }
            }
            Instruction::Cast { value, .. } => {
                if let Operand::Value(v) = value {
                    let resolved = self.resolve_alias(value, state);
                    let check_val = if let Operand::Value(rv) = resolved { *rv } else { *v };
                    state.aliases.insert(val_id, Operand::Value(check_val));
                    if let Some(prov) = state.direct_provenance.get(&check_val).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(&check_val).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.direct_provenance.get(v).cloned() {
                        state.direct_provenance.entry(val_id).or_default().extend(prov);
                    }
                    if let Some(prov) = state.carried_provenance.get(v).cloned() {
                        state.carried_provenance.entry(val_id).or_default().extend(prov);
                    }
                }
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
                if modes.iter().any(|mode| matches!(mode, luna_semantic::CaptureMode::SharedBorrow | luna_semantic::CaptureMode::MutableBorrow)) {
                    let mut diag = Diagnostic::error("error[E3005]: LocalBorrowEscape: Cannot return a closure that captures a local borrow");
                    diag.span = self.func.value(*value).span.clone();
                    if !self.diagnostics.iter().any(|existing| existing.message == diag.message) {
                        self.diagnostics.push(diag);
                    }
                }
            }

            // Def-site Return Safety validation (P0-A, P0-B, P0-C, P0-D)
            let ret_ty = self.func.ret_ty;
            let is_ref_ret = if let Some(ctx) = self.ctx {
                matches!(ctx.types.get(ret_ty), SemanticType::Reference(..))
            } else {
                false
            };

            let fn_sym_id = self.func.name.symbol_id.or_else(|| {
                self.ctx.and_then(|c| c.symbol_table.lookup(&self.func.name.name, luna_semantic::symbol::ScopeId(0)))
            });
            let declared_contract = fn_sym_id.and_then(|sym| {
                self.ctx.and_then(|c| c.tables.fn_lifetime_contracts.get(&sym))
            });

            if is_ref_ret || declared_contract.is_some() {
                // Collect all loans for `value`
                let mut ret_loans = HashSet::new();
                if let Some(prov) = state.direct_provenance.get(value) {
                    ret_loans.extend(prov.clone());
                }
                if let Some(prov) = state.carried_provenance.get(value) {
                    ret_loans.extend(prov.clone());
                }
                let val_op = Operand::Value(*value);
                let resolved_op = self.resolve_alias(&val_op, state);
                if let Operand::Value(res_v) = resolved_op {
                    if res_v != value {
                        if let Some(prov) = state.direct_provenance.get(res_v) {
                            ret_loans.extend(prov.clone());
                        }
                        if let Some(prov) = state.carried_provenance.get(res_v) {
                            ret_loans.extend(prov.clone());
                        }
                    }
                }

                // Step 1: Check for local escapes (P0-A)
                let mut has_local_escape = false;
                let mut actual_param_origins = HashSet::new();
                let mut has_global_origin = false;

                for loan in &ret_loans {
                    let resolved = self.resolve_alias(&loan.place, state);
                    match resolved {
                        Operand::Global(_) => {
                            has_global_origin = true;
                        }
                        Operand::Value(v) => {
                            if (v.0 as usize) < self.func.values.len() {
                                let val_data = &self.func.values[v.0 as usize];
                                match &val_data.origin {
                                    ValueOrigin::Global => {
                                        has_global_origin = true;
                                    }
                                    ValueOrigin::Parameter(p_idx) => {
                                        let is_ref_param = if let Some(ctx) = self.ctx {
                                            matches!(ctx.types.get(val_data.ty), SemanticType::Reference(..))
                                        } else {
                                            false
                                        };
                                        if is_ref_param {
                                            actual_param_origins.insert(*p_idx as u16);
                                        } else {
                                            has_local_escape = true;
                                        }
                                    }
                                    ValueOrigin::Local | ValueOrigin::Temporary => {
                                        has_local_escape = true;
                                    }
                                }
                            } else {
                                has_local_escape = true;
                            }
                        }
                        _ => {
                            has_local_escape = true;
                        }
                    }
                }

                // If value resolved directly to an operand without explicit loans in provenance:
                if ret_loans.is_empty() && is_ref_ret {
                    if let Operand::Value(res_v) = resolved_op {
                        if (res_v.0 as usize) < self.func.values.len() {
                            let val_data = &self.func.values[res_v.0 as usize];
                            match &val_data.origin {
                                ValueOrigin::Local | ValueOrigin::Temporary => {
                                    has_local_escape = true;
                                }
                                ValueOrigin::Parameter(p_idx) => {
                                    let is_ref_param = if let Some(ctx) = self.ctx {
                                        matches!(ctx.types.get(val_data.ty), SemanticType::Reference(..))
                                    } else {
                                        false
                                    };
                                    if is_ref_param {
                                        actual_param_origins.insert(*p_idx as u16);
                                    } else {
                                        has_local_escape = true;
                                    }
                                }
                                ValueOrigin::Global => {
                                    has_global_origin = true;
                                }
                            }
                        }
                    } else if let Operand::Global(_) = resolved_op {
                        has_global_origin = true;
                    }
                }

                if has_local_escape {
                    let mut diag = Diagnostic::error("error[E3005]: LocalBorrowEscape: Reference to local variable escapes function scope");
                    diag.span = self.func.value(*value).span.clone();
                    if !self.diagnostics.iter().any(|existing| existing.message == diag.message && existing.span == diag.span) {
                        self.diagnostics.push(diag);
                    }
                } else if let Some(contract) = declared_contract {
                    // Step 2: Check declared contract satisfaction (P0-B)
                    if let Some(prov) = &contract.return_provenance {
                        let allowed_indices: HashSet<u16> = prov.indices().iter().copied().collect();
                        let invalid_origins: Vec<u16> = actual_param_origins
                            .iter()
                            .copied()
                            .filter(|idx| !allowed_indices.contains(idx))
                            .collect();

                        if !invalid_origins.is_empty() {
                            let mut diag = Diagnostic::error(format!(
                                "error[E2016]: LifetimeConstraintViolation: return value has provenance from parameter (index {}), which does not satisfy declared lifetime contract",
                                invalid_origins[0]
                            ));
                            diag.span = self.func.value(*value).span.clone();
                            if !self.diagnostics.iter().any(|existing| existing.message == diag.message && existing.span == diag.span) {
                                self.diagnostics.push(diag);
                            }
                        } else if is_ref_ret && actual_param_origins.is_empty() && !has_global_origin {
                            let mut diag = Diagnostic::error(
                                "error[E2016]: LifetimeConstraintViolation: return expression does not satisfy declared lifetime contract"
                            );
                            diag.span = self.func.value(*value).span.clone();
                            if !self.diagnostics.iter().any(|existing| existing.message == diag.message && existing.span == diag.span) {
                                self.diagnostics.push(diag);
                            }
                        }
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
