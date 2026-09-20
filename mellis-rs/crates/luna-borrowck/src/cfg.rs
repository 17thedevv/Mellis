use luna_mvir::{Function, Terminator, ValueId};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct NaturalLoop {
    pub header: String,
    pub latches: HashSet<String>,
    pub blocks: HashSet<String>,
    pub body_blocks: HashSet<String>,
    pub local_values: HashSet<ValueId>,
}

#[derive(Clone, Debug, Default)]
pub struct LoopInfo {
    pub natural_loops: Vec<NaturalLoop>,
    /// Map from ValueId to its defining basic block label
    pub value_defining_block: HashMap<ValueId, String>,
}

impl LoopInfo {
    /// Identifies all ValueIds whose dynamic lifetime region ends when crossing edge (from -> to).
    /// This includes:
    /// 1. Back-edges / continue edges of a loop L (header == to, from in L.blocks): ends the current iteration of L.
    /// 2. Exit edges of a loop L (from in L.blocks, to not in L.blocks): exits loop L (e.g. break, loop condition false).
    pub fn ending_lifetime_locals(&self, from: &str, to: &str) -> HashSet<ValueId> {
        let mut ending = HashSet::new();
        for nl in &self.natural_loops {
            let is_back_edge = nl.header == to && nl.blocks.contains(from);
            let is_exit_edge = nl.blocks.contains(from) && !nl.blocks.contains(to);
            if is_back_edge || is_exit_edge {
                ending.extend(nl.local_values.iter().copied());
            }
        }
        ending
    }
}

pub fn analyze_loops(func: &Function) -> LoopInfo {
    let mut info = LoopInfo::default();
    if func.blocks.is_empty() {
        return info;
    }

    // Map each ValueId to its defining block
    for block in &func.blocks {
        for &val_id in &block.insts {
            info.value_defining_block.insert(val_id, block.label.name.clone());
        }
    }

    let (preds, succs) = compute_cfg_edges(func);
    let doms = compute_dominators(func, &preds);

    // Find back-edges: edge u -> v where v dominates u (v is header, u is latch)
    let mut header_to_latches: HashMap<String, HashSet<String>> = HashMap::new();
    for block in &func.blocks {
        let u = &block.label.name;
        if let Some(successors) = succs.get(u) {
            for v in successors {
                if let Some(u_doms) = doms.get(u) {
                    if u_doms.contains(v) {
                        header_to_latches.entry(v.clone()).or_default().insert(u.clone());
                    }
                }
            }
        }
    }

    // Compute natural loop blocks and local values for each header
    for (header, latches) in header_to_latches {
        let mut loop_blocks = HashSet::new();
        loop_blocks.insert(header.clone());

        let mut stack = Vec::new();
        for latch in &latches {
            loop_blocks.insert(latch.clone());
            if latch != &header {
                stack.push(latch.clone());
            }
        }

        while let Some(curr) = stack.pop() {
            if let Some(predecessors) = preds.get(&curr) {
                for pred in predecessors {
                    if loop_blocks.insert(pred.clone()) {
                        stack.push(pred.clone());
                    }
                }
            }
        }

        // Body blocks are loop blocks excluding the header
        let body_blocks: HashSet<String> = loop_blocks
            .iter()
            .filter(|b| *b != &header)
            .cloned()
            .collect();

        let mut local_values = HashSet::new();
        for b_name in &body_blocks {
            if let Some(block) = func.blocks.iter().find(|b| &b.label.name == b_name) {
                for &val_id in &block.insts {
                    local_values.insert(val_id);
                }
            }
        }

        info.natural_loops.push(NaturalLoop {
            header,
            latches,
            blocks: loop_blocks,
            body_blocks,
            local_values,
        });
    }

    // Fallback: detect structured loop headers without back-edges (e.g., loops where every path breaks)
    for block in &func.blocks {
        let name = &block.label.name;
        if (name.starts_with("while_cond") || name.starts_with("for_cond")) && !info.natural_loops.iter().any(|nl| &nl.header == name) {
            if let Some(Terminator::CondBr { true_target, false_target, .. }) = &block.terminator {
                let mut body_blocks = HashSet::new();
                let mut stack = vec![true_target.name.clone()];
                while let Some(curr) = stack.pop() {
                    if curr != false_target.name && curr != *name && body_blocks.insert(curr.clone()) {
                        if let Some(successors) = succs.get(&curr) {
                            for s in successors {
                                stack.push(s.clone());
                            }
                        }
                    }
                }
                let mut loop_blocks = body_blocks.clone();
                loop_blocks.insert(name.clone());

                let mut local_values = HashSet::new();
                for b_name in &body_blocks {
                    if let Some(b) = func.blocks.iter().find(|b| &b.label.name == b_name) {
                        for &val_id in &b.insts {
                            local_values.insert(val_id);
                        }
                    }
                }

                info.natural_loops.push(NaturalLoop {
                    header: name.clone(),
                    latches: HashSet::new(),
                    blocks: loop_blocks,
                    body_blocks,
                    local_values,
                });
            }
        }
    }

    info
}

pub fn compute_cfg_edges(
    func: &Function,
) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut preds: HashMap<String, Vec<String>> = HashMap::new();
    let mut succs: HashMap<String, Vec<String>> = HashMap::new();

    for block in &func.blocks {
        preds.entry(block.label.name.clone()).or_default();
        let s = succs.entry(block.label.name.clone()).or_default();

        if let Some(term) = &block.terminator {
            match term {
                Terminator::Br { target } => {
                    s.push(target.name.clone());
                }
                Terminator::CondBr {
                    true_target,
                    false_target,
                    ..
                } => {
                    s.push(true_target.name.clone());
                    s.push(false_target.name.clone());
                }
                _ => {}
            }
        }

        for succ in s.clone() {
            preds.entry(succ).or_default().push(block.label.name.clone());
        }
    }

    (preds, succs)
}

pub fn compute_dominators(
    func: &Function,
    preds: &HashMap<String, Vec<String>>,
) -> HashMap<String, HashSet<String>> {
    let mut doms: HashMap<String, HashSet<String>> = HashMap::new();
    let all_blocks: HashSet<String> = func.blocks.iter().map(|b| b.label.name.clone()).collect();

    let entry = &func.blocks[0].label.name;
    let mut entry_set = HashSet::new();
    entry_set.insert(entry.clone());
    doms.insert(entry.clone(), entry_set);

    for block in &func.blocks[1..] {
        doms.insert(block.label.name.clone(), all_blocks.clone());
    }

    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks[1..] {
            let name = &block.label.name;
            let block_preds = match preds.get(name) {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            // Intersection of doms of all predecessors
            let mut new_dom: Option<HashSet<String>> = None;
            for pred in block_preds {
                if let Some(pred_dom) = doms.get(pred) {
                    match &mut new_dom {
                        None => new_dom = Some(pred_dom.clone()),
                        Some(current) => current.retain(|item| pred_dom.contains(item)),
                    }
                }
            }

            let mut new_dom = new_dom.unwrap_or_default();
            new_dom.insert(name.clone());

            if doms.get(name) != Some(&new_dom) {
                doms.insert(name.clone(), new_dom);
                changed = true;
            }
        }
    }

    doms
}
