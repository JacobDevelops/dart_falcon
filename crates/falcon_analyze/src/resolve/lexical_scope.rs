//! Lexical scope storage shared by binding analyses, with owned or copy-on-write maps.
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub(crate) trait ScopeStorage: Default {
    type Binding;

    fn get(&self, name: &str) -> Option<&Self::Binding>;
    fn get_mut(&mut self, name: &str) -> Option<&mut Self::Binding>;
    fn insert(&mut self, name: String, value: Self::Binding) -> &mut Self::Binding;
}

impl<B, H: BuildHasher + Default> ScopeStorage for HashMap<String, B, H> {
    type Binding = B;

    fn get(&self, name: &str) -> Option<&B> {
        HashMap::get(self, name)
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut B> {
        HashMap::get_mut(self, name)
    }

    fn insert(&mut self, name: String, value: B) -> &mut B {
        self.entry(name).insert_entry(value).into_mut()
    }
}

impl<S: ScopeStorage + Clone> ScopeStorage for Arc<S> {
    type Binding = S::Binding;

    fn get(&self, name: &str) -> Option<&Self::Binding> {
        self.as_ref().get(name)
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Self::Binding> {
        Arc::make_mut(self).get_mut(name)
    }

    fn insert(&mut self, name: String, value: Self::Binding) -> &mut Self::Binding {
        Arc::make_mut(self).insert(name, value)
    }
}

/// `Default` is empty for cached results; `new` starts an active traversal at its root.
#[derive(Debug, Clone, Default)]
pub(crate) struct LexicalScopes<S: ScopeStorage>(Vec<S>);

impl<S: ScopeStorage> LexicalScopes<S> {
    pub(crate) fn new() -> Self {
        Self(vec![S::default()])
    }

    pub(crate) fn push_scope(&mut self) {
        self.0.push(S::default());
    }

    pub(crate) fn pop_scope(&mut self) {
        if self.0.len() > 1 {
            self.0.pop();
        }
    }

    pub(crate) fn declare(&mut self, name: String, value: S::Binding) -> &mut S::Binding {
        self.0
            .last_mut()
            .expect("lexical scope")
            .insert(name, value)
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<(usize, &S::Binding)> {
        self.0
            .iter()
            .enumerate()
            .rev()
            .find_map(|(depth, scope)| scope.get(name).map(|binding| (depth, binding)))
    }

    pub(crate) fn lookup_mut(&mut self, name: &str) -> Option<&mut S::Binding> {
        let (depth, _) = self.lookup(name)?;
        self.0[depth].get_mut(name)
    }
}

impl<S: ScopeStorage> Deref for LexicalScopes<S> {
    type Target = [S];

    fn deref(&self) -> &[S] {
        &self.0
    }
}

impl<S: ScopeStorage> DerefMut for LexicalScopes<S> {
    fn deref_mut(&mut self) -> &mut [S] {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope_contract<S: ScopeStorage<Binding = usize>>() {
        let mut scopes = LexicalScopes::<S>::default();
        assert!(scopes.is_empty());
        scopes.pop_scope();
        assert!(scopes.is_empty());

        let mut scopes = LexicalScopes::<S>::new();
        scopes.declare("x".to_owned(), 1);
        scopes.declare("x".to_owned(), 2);
        assert_eq!(scopes.lookup("x"), Some((0, &2)));
        scopes.push_scope();
        scopes.declare("x".to_owned(), 3);
        scopes.push_scope();
        assert_eq!(scopes.lookup("x"), Some((1, &3)));
        *scopes.lookup_mut("x").unwrap() = 4;
        assert_eq!(scopes.lookup("x"), Some((1, &4)));
        assert!(scopes.lookup_mut("missing").is_none());
        assert!(scopes.lookup("missing").is_none());
        scopes.pop_scope();
        scopes.pop_scope();
        assert_eq!(scopes.lookup("x"), Some((0, &2)));
        scopes.pop_scope();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes.lookup("x"), Some((0, &2)));
    }

    #[test]
    fn owned_scope_contract() {
        scope_contract::<HashMap<String, usize>>();
    }

    #[test]
    fn shared_scope_contract() {
        scope_contract::<Arc<HashMap<String, usize>>>();
    }

    #[test]
    fn shared_mutation_copies_only_the_matching_scope() {
        let mut scopes = LexicalScopes::<Arc<HashMap<String, usize>>>::new();
        scopes.declare("x".to_owned(), 1);
        scopes.push_scope();
        scopes.declare("y".to_owned(), 2);
        let snapshot = scopes.clone();

        assert!(scopes.lookup_mut("missing").is_none());
        assert!(Arc::ptr_eq(&scopes[0], &snapshot[0]));
        assert!(Arc::ptr_eq(&scopes[1], &snapshot[1]));
        *scopes.lookup_mut("x").unwrap() = 3;
        assert!(!Arc::ptr_eq(&scopes[0], &snapshot[0]));
        assert!(Arc::ptr_eq(&scopes[1], &snapshot[1]));
        assert_eq!(snapshot.lookup("x"), Some((0, &1)));
        assert_eq!(scopes.lookup("x"), Some((0, &3)));

        scopes.declare("y".to_owned(), 4);
        assert!(!Arc::ptr_eq(&scopes[1], &snapshot[1]));
        assert_eq!(snapshot.lookup("y"), Some((1, &2)));
        assert_eq!(scopes.lookup("y"), Some((1, &4)));
    }
}
