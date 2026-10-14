//! Lint rules, organized biome-style by group. Each rule lives in
//! `lint/<group>/<rule>.rs`; the `source` field of its `RuleMeta` entry
//! (see `meta.rs`) records its upstream provenance.

pub mod complexity;
pub(crate) mod const_eval;
pub mod correctness;
pub(crate) mod declarations;
pub(crate) mod expressions;
pub(crate) mod flutter;
pub(crate) mod function_bodies;
pub(crate) mod lexical_usage;
pub(crate) mod naming;
pub(crate) mod options;
pub mod performance;
pub(crate) mod resource_leaks;
pub(crate) mod semantic_scope;
pub(crate) mod semantic_type_operations;
pub(crate) mod string_lit;
pub mod style;
pub mod suspicious;
