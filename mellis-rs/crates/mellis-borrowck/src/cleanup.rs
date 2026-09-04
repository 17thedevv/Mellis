use mellis_mvir::{Function, Instruction, ValueId};
use std::collections::HashSet;

/// Eliminates redundant `Instruction::Drop`s by replacing them with `Instruction::Nop`.
/// This relies on `mellis_borrowck::borrow_check_function` accurately determining
/// which auto-generated drops were on already-moved or uninitialized data.
pub fn eliminate_redundant_drops(func: &mut Function, redundant_drops: &HashSet<ValueId>) {
    for val_id in redundant_drops {
        if let Some(val_data) = func.values.get_mut(val_id.0 as usize) {
            // Only replace if it's actually a Drop (sanity check).
            if matches!(val_data.inst, Instruction::Drop { .. }) {
                val_data.inst = Instruction::Nop;
            }
        }
    }
}
