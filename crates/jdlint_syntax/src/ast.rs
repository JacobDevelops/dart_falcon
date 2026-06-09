/// Top-level representation of a parsed Dart compilation unit.
#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

/// A top-level declaration in a Dart file.
#[derive(Debug, Clone)]
pub enum Declaration {
    // Populated during M1 parser implementation
}
