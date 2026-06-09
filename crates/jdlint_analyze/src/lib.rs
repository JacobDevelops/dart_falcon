//! Rule visitor infrastructure and parallel analysis engine.
//!
//! Owns the `Rule` and `RuleVisitor` trait contracts (locked at M0.5).
//! Drives per-file Rayon parallelism: each .dart file is one work unit.

pub mod context;
pub mod parallel;
pub mod registry;
pub mod rule;
pub mod visitor;

pub use context::AnalyzeContext;
pub use parallel::{analyze_parallel, analyze_sequential};
pub use registry::RuleRegistry;
pub use rule::Rule;
pub use visitor::RuleVisitor;
