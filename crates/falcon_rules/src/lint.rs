//! Lint rules, organized biome-style by group. Each rule lives in
//! `lint/<group>/<rule>.rs`; the `source` field of its `RuleMeta` entry
//! (see `meta.rs`) records its upstream provenance.

pub mod complexity;
pub mod correctness;
pub(crate) mod lexical_usage;
pub mod performance;
pub(crate) mod semantic_scope;
pub(crate) mod semantic_type_operations;
pub mod style;
pub mod suspicious;
