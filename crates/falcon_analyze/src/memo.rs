//! Concurrent memo tables for pure queries over immutable indexes.
//!
//! Resolution queries (`name` visible from a library, a library's export
//! namespace) are asked again for every reference in every file; their
//! answers depend only on the index, so the first answer is kept. Keys hash
//! with FxHash and are looked up by borrowed parts, so looking up a cached
//! key neither allocates nor runs SipHash. Returning an owned answer may clone it.

use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::{PoisonError, RwLock};

use rustc_hash::{FxBuildHasher, FxHashMap};

/// A query key: a copyable scope plus an optional qualifier and a name.
pub(crate) struct NameKey<S> {
    scope: S,
    qualifier: Option<Box<str>>,
    name: Box<str>,
}

/// The borrowed parts of a [`NameKey`], so lookups need no owned key.
pub(crate) trait NameKeyParts<S> {
    fn parts(&self) -> (&S, Option<&str>, &str);
}

impl<S> NameKeyParts<S> for NameKey<S> {
    fn parts(&self) -> (&S, Option<&str>, &str) {
        (&self.scope, self.qualifier.as_deref(), &self.name)
    }
}

impl<S> NameKeyParts<S> for (S, Option<&str>, &str) {
    fn parts(&self) -> (&S, Option<&str>, &str) {
        (&self.0, self.1, self.2)
    }
}

impl<S: Hash> Hash for dyn NameKeyParts<S> + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parts().hash(state);
    }
}

impl<S: PartialEq> PartialEq for dyn NameKeyParts<S> + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}

impl<S: Eq> Eq for dyn NameKeyParts<S> + '_ {}

impl<S: Hash> Hash for NameKey<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parts().hash(state);
    }
}

impl<S: PartialEq> PartialEq for NameKey<S> {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}

impl<S: Eq> Eq for NameKey<S> {}

impl<'a, S: 'a> Borrow<dyn NameKeyParts<S> + 'a> for NameKey<S> {
    fn borrow(&self) -> &(dyn NameKeyParts<S> + 'a) {
        self
    }
}

const SHARDS: usize = 32;

#[repr(align(128))]
struct MemoShard<S, V>(RwLock<FxHashMap<NameKey<S>, V>>);

/// Memoized answers of one pure query keyed by `(scope, qualifier, name)`.
/// Clones start empty: a memo is derived data of the index owning it.
pub(crate) struct NameMemo<S, V> {
    answers: [MemoShard<S, V>; SHARDS],
}

impl<S, V> Default for NameMemo<S, V> {
    fn default() -> Self {
        Self {
            answers: std::array::from_fn(|_| MemoShard(RwLock::new(FxHashMap::default()))),
        }
    }
}

impl<S, V> Clone for NameMemo<S, V> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<S, V> std::fmt::Debug for NameMemo<S, V> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NameMemo")
    }
}

impl<S: Copy + Hash + Eq, V: Clone> NameMemo<S, V> {
    fn shard(&self, scope: S, qualifier: Option<&str>, name: &str) -> &MemoShard<S, V> {
        let hash = FxBuildHasher.hash_one((scope, qualifier, name));
        &self.answers[(hash >> 32) as usize % SHARDS]
    }

    /// The memoized answer for the key, if any.
    pub(crate) fn get(&self, scope: S, qualifier: Option<&str>, name: &str) -> Option<V> {
        self.shard(scope, qualifier, name)
            .0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&(scope, qualifier, name) as &dyn NameKeyParts<S>)
            .cloned()
    }

    /// The memoized answer for the key, computing it on a miss. `compute`
    /// runs without the lock held, so it may consult this memo recursively.
    pub(crate) fn get_or_compute(
        &self,
        scope: S,
        qualifier: Option<&str>,
        name: &str,
        compute: impl FnOnce() -> V,
    ) -> V {
        if let Some(answer) = self.get(scope, qualifier, name) {
            return answer;
        }
        let answer = compute();
        self.shard(scope, qualifier, name)
            .0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                NameKey {
                    scope,
                    qualifier: qualifier.map(Box::from),
                    name: Box::from(name),
                },
                answer.clone(),
            );
        answer
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.answers
            .iter()
            .map(|shard| shard.0.read().unwrap_or_else(PoisonError::into_inner).len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::NameMemo;

    #[test]
    fn borrowed_keys_keep_scope_qualifier_and_absence_distinct() {
        let memo: NameMemo<usize, Option<usize>> = NameMemo::default();
        for (scope, qualifier, answer) in [
            (0, None, None),
            (0, Some(""), Some(1)),
            (0, Some("p"), Some(2)),
            (1, None, Some(3)),
        ] {
            assert_eq!(
                memo.get_or_compute(scope, qualifier, "Name", || answer),
                answer
            );
            let owned_name = String::from("Name");
            assert_eq!(memo.get(scope, qualifier, &owned_name), Some(answer));
            assert_eq!(
                memo.get_or_compute(scope, qualifier, &owned_name, || panic!("cache miss")),
                answer
            );
        }
        assert_eq!(memo.len(), 4);
        assert_eq!(memo.get(0, None, "Other"), None);
        assert_eq!(memo.clone().len(), 0);
    }

    #[test]
    fn computations_can_query_the_same_memo_recursively() {
        let memo: NameMemo<usize, usize> = NameMemo::default();
        assert_eq!(
            memo.get_or_compute(0, None, "Outer", || {
                memo.get_or_compute(0, None, "Inner", || 7) + 1
            }),
            8
        );
        assert_eq!(memo.len(), 2);
    }

    #[test]
    fn concurrent_queries_preserve_answers_across_scopes() {
        let memo: NameMemo<usize, usize> = NameMemo::default();
        std::thread::scope(|threads| {
            for _ in 0..8 {
                let memo = &memo;
                threads.spawn(move || {
                    for scope in 0..1024 {
                        assert_eq!(
                            memo.get_or_compute(scope, Some("p"), "Name", || scope + 1),
                            scope + 1
                        );
                        assert_eq!(memo.get(scope, Some("p"), "Name"), Some(scope + 1));
                    }
                });
            }
        });
        assert_eq!(memo.len(), 1024);
    }
}
