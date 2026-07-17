//! Rule visitor infrastructure and parallel analysis engine.
//!
//! Owns the `Rule` and `RuleVisitor` trait contracts (locked at M0.5).
//! Drives per-file Rayon parallelism: each .dart file is one work unit.

pub mod context;
pub mod cross_file;
pub mod parallel;
pub mod registry;
pub mod resolve;
pub mod rule;
pub mod suppressions;
pub mod visitor;

pub use context::AnalyzeContext;
pub use parallel::{
    analyze_parallel, analyze_parallel_collecting, analyze_parallel_collecting_resolving,
    analyze_sequential, analyze_sequential_collecting, analyze_sequential_collecting_resolving,
};
pub use cross_file::{CrossFileRule, CrossFileRuleRegistry, ProjectFile};
pub use registry::RuleRegistry;
pub use resolve::{
    LibraryGrouping, LibrarySource, LibraryUnit, LocalTypes, MemberKind, MemberResult,
    ProjectIndex, ReceiverTypes, StaticType, SubtypeResult, TypeIndex, TypeKind, group_libraries,
    library_unit,
};
pub use rule::Rule;
pub use suppressions::{FileSuppressions, MALFORMED_SUPPRESSION, RuleLookup};
pub use visitor::RuleVisitor;
