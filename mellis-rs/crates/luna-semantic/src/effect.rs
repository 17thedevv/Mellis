use std::collections::HashSet;
use luna_common::ids::SymbolId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    Pure,
    IO,
    Extern,
    Nondeterministic,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectSet {
    pub effects: HashSet<Effect>,
}

impl EffectSet {
    pub fn pure() -> Self {
        Self {
            effects: HashSet::new(),
        }
    }

    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn contains(&self, effect: Effect) -> bool {
        self.effects.contains(&effect)
    }

    pub fn add(&mut self, effect: Effect) {
        self.effects.insert(effect);
    }

    pub fn merge(&mut self, other: &EffectSet) {
        self.effects.extend(other.effects.iter().copied());
    }
}
