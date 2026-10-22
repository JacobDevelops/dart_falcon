//! Lint rules, organized biome-style by group. Each rule lives in
//! `lint/<group>/<rule>.rs`; its group's declaration supplies its module,
//! identity, constructor, and metadata.

pub mod a11y;
pub(crate) mod bloc;
pub mod complexity;
pub(crate) mod const_eval;
pub(crate) mod core_imports;
pub mod correctness;
pub(crate) mod declarations;
pub(crate) mod doc_comments;
pub(crate) mod expressions;
pub(crate) mod flutter;
pub(crate) mod flutter_hooks;
pub(crate) mod inherited_members;
pub(crate) mod lexical_usage;
pub(crate) mod naming;
pub(crate) mod options;
pub mod performance;
#[cfg(test)]
mod registry_tests;
pub(crate) mod resource_leaks;
pub(crate) mod riverpod;
pub(crate) mod riverpod_async;
pub mod security;
pub(crate) mod semantic_type_operations;
pub(crate) mod string_lit;
pub mod style;
pub(crate) mod support;
pub mod suspicious;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod tokens;
