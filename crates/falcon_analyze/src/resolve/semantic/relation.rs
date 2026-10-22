use super::{DeclarationIdentity, ResolvedType, SemanticModel, SignatureIndex, Truth};

/// A resolved-type relation to prove. Missing semantic facts remain unknown.
#[derive(Clone, Copy, Debug)]
pub enum TypeRelation<'a> {
    /// Subtyping against a fully instantiated type.
    SubtypeOf(&'a ResolvedType),
    /// Membership in a declaration's hierarchy, without constraining arguments.
    SubtypeOfDeclaration(&'a DeclarationIdentity),
    /// Structural unrelatedness, retaining instantiated supertype arguments.
    UnrelatedTo(&'a ResolvedType),
    /// The linter's unrelatedness relation after extension-type erasure.
    RuntimeUnrelatedTo(&'a ResolvedType),
}

impl SignatureIndex {
    /// Proves a relation using three-valued semantic facts.
    ///
    /// A declaration-hierarchy miss is `No` only when every reachable edge is
    /// known; an unresolved edge or cyclic hierarchy remains `Unknown`.
    pub fn relation(
        &self,
        ty: &ResolvedType,
        relation: TypeRelation<'_>,
        model: &SemanticModel<'_>,
    ) -> Truth {
        match relation {
            TypeRelation::SubtypeOf(target) => self.resolved_subtype(ty, target, model),
            TypeRelation::SubtypeOfDeclaration(target) => {
                self.declaration_subtype(ty, target, model)
            }
            TypeRelation::UnrelatedTo(target) => self.instantiated_unrelated(ty, target, model),
            TypeRelation::RuntimeUnrelatedTo(target) => self.runtime_unrelated(ty, target, model),
        }
    }
}
