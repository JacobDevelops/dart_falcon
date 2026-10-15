//! Index-owned string storage for external declaration identities.

use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// A static builtin name or a shared name owned by the indexes using it.
#[derive(Clone)]
pub enum InternedString {
    Static(&'static str),
    Shared(Arc<str>),
}

impl InternedString {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Static(value) => value,
            Self::Shared(value) => value,
        }
    }
}

impl fmt::Debug for InternedString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl Borrow<str> for InternedString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for InternedString {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl From<&'static str> for InternedString {
    fn from(value: &'static str) -> Self {
        Self::Static(value)
    }
}

impl PartialEq for InternedString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for InternedString {}
impl PartialOrd for InternedString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for InternedString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl Hash for InternedString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
impl PartialEq<str> for InternedString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<&str> for InternedString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Deduplicates names for one index; dropping the index releases its storage.
#[derive(Debug, Clone, Default)]
pub struct StringInterner {
    names: HashSet<Arc<str>>,
}

impl StringInterner {
    pub fn get(&self, text: &str) -> Option<InternedString> {
        self.names.get(text).cloned().map(InternedString::Shared)
    }

    pub fn intern(&mut self, text: &str) -> InternedString {
        if let Some(name) = self.get(text) {
            return name;
        }
        let name: Arc<str> = text.into();
        self.names.insert(name.clone());
        InternedString::Shared(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interns_each_name_once_and_releases_storage() {
        let mut names = StringInterner::default();
        let first = names.intern("Widget");
        let second = names.intern("Widget");
        let (InternedString::Shared(first), InternedString::Shared(second)) = (first, second)
        else {
            panic!("shared names");
        };
        assert!(Arc::ptr_eq(&first, &second));
        let weak = Arc::downgrade(&first);
        drop(first);
        drop(second);
        drop(names);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn static_and_shared_names_have_value_identity() {
        let mut names = StringInterner::default();
        let shared = names.intern("Widget");
        let builtin = InternedString::Static("Widget");
        assert_eq!(builtin, shared);
        assert_eq!(builtin.cmp(&shared), std::cmp::Ordering::Equal);
        assert!(names.intern("Alpha") < builtin);
        assert_eq!(format!("{shared:?}"), "\"Widget\"");
        assert_eq!(format!("{builtin:?}"), "\"Widget\"");
        let mut set = HashSet::new();
        set.insert(builtin);
        assert!(set.contains(&shared));
        assert!(set.contains("Widget"));
    }
}
