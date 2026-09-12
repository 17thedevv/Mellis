use crate::ids::SymbolId;
use std::collections::HashMap;

#[derive(Default)]
pub struct StringInterner {
    map: HashMap<String, SymbolId>,
    vec: Vec<String>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, name: &str) -> SymbolId {
        if let Some(&id) = self.map.get(name) {
            return id;
        }

        let id = SymbolId(self.vec.len() as u32);
        self.map.insert(name.to_string(), id);
        self.vec.push(name.to_string());
        id
    }

    pub fn lookup(&self, id: SymbolId) -> &str {
        &self.vec[id.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interning() {
        let mut interner = StringInterner::new();
        let id1 = interner.intern("hello");
        let id2 = interner.intern("world");
        let id3 = interner.intern("hello");

        assert_eq!(id1, id3);
        assert_ne!(id1, id2);
        assert_eq!(interner.lookup(id1), "hello");
        assert_eq!(interner.lookup(id2), "world");
    }
}
