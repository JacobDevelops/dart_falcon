//! Rule visitor infrastructure and parallel analysis engine.
//!
//! Owns the `Rule` and `RuleVisitor` trait contracts (locked at M0.5).
//! Drives per-file Rayon parallelism: each .dart file is one work unit.

pub mod context;
pub mod registry;
pub mod rule;

pub use context::AnalyzeContext;
pub use registry::RuleRegistry;
pub use rule::Rule;
