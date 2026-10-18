//! Lint rules, organized biome-style by group. Each rule lives in
//! `lint/<group>/<rule>.rs`; the `source` field of its `RuleMeta` entry
//! (see `meta.rs`) records its upstream provenance.

pub mod a11y;
pub mod complexity;
pub(crate) mod const_eval;
pub(crate) mod core_imports;
pub mod correctness;
pub(crate) mod declarations;
pub(crate) mod doc_comments;
pub(crate) mod expressions;
pub(crate) mod flutter;
pub(crate) mod function_bodies;
pub(crate) mod inherited_members;
pub(crate) mod lexical_usage;
pub(crate) mod naming;
pub(crate) mod options;
pub(crate) mod package_layout;
pub mod performance;
pub(crate) mod resource_leaks;
pub mod security;
#[cfg(test)]
mod semantic_scope_tests;
pub(crate) mod semantic_type_operations;
pub(crate) mod string_lit;
pub mod style;
pub mod suspicious;
pub(crate) mod tokens;
