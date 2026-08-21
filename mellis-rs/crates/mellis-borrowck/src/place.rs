use mellis_mvir::LocalId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Projection {
    Field(usize),
    Deref,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: LocalId,
    pub projections: Vec<Projection>,
}

impl Place {
    pub fn new(local: LocalId) -> Self {
        Self {
            local,
            projections: Vec::new(),
        }
    }

    pub fn project(&mut self, proj: Projection) {
        self.projections.push(proj);
    }
}
