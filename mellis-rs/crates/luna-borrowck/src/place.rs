use luna_mvir::ValueId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Projection {
    Field(usize),
    Deref,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: ValueId,
    pub projections: Vec<Projection>,
}

impl Place {
    pub fn new(local: ValueId) -> Self {
        Self {
            local,
            projections: Vec::new(),
        }
    }

    pub fn project(&mut self, proj: Projection) {
        self.projections.push(proj);
    }
    
    pub fn projected(mut self, proj: Projection) -> Self {
        self.projections.push(proj);
        self
    }
    
    pub fn is_ancestor_of(&self, other: &Place) -> bool {
        if self.local != other.local { return false; }
        if self.projections.len() > other.projections.len() { return false; }
        for (i, proj) in self.projections.iter().enumerate() {
            if proj != &other.projections[i] { return false; }
        }
        true
    }
    
    pub fn is_descendant_of(&self, other: &Place) -> bool {
        other.is_ancestor_of(self)
    }
}
