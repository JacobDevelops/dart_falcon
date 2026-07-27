//! Rule visitor infrastructure and parallel analysis engine.
//!
//! Owns the `Rule` and `RuleVisitor` trait contracts (locked at M0.5).
//! Drives per-file Rayon parallelism: each .dart file is one work unit.

pub mod build_context_flow;
pub mod constant_value;
pub mod context;
pub mod cross_file;
pub mod parallel;
pub mod registry;
pub mod resolve;
pub mod rule;
pub mod suppressions;
pub mod visitor;

pub use build_context_flow::BuildContextFlowAnalyzer;
pub use constant_value::{ConstantValue, evaluate_constant, parse_int};
pub use context::AnalyzeContext;
pub use cross_file::{CrossFileRule, CrossFileRuleRegistry, ProjectFile};
pub use parallel::{
    analyze_parallel, analyze_parallel_collecting, analyze_parallel_collecting_resolving,
    analyze_sequential, analyze_sequential_collecting, analyze_sequential_collecting_resolving,
    syntax_error_diagnostics,
};
pub use registry::{RuleRegistry, with_rules_stack};
pub use resolve::{
    ConstructorFacts, DeclarationFacts, DeclarationIdentity, IdentityIndex, IdentitySource,
    InheritedParameterNames, LibraryGrouping, LibrarySource, LibraryUnit, LocalTypes, MemberFacts,
    MemberKind, MemberResult, NameIdentity, PackageIdentity, ParameterFacts, ProgramSource,
    ProjectIndex, ReceiverTypes, ResolvedSignature, ResolvedType, SemanticMemberKind,
    SemanticModel, SignatureIndex, StaticConstFacts, StaticType, SubtypeResult, TypeEnvironment,
    TypeIndex, TypeKind, TypeParameterId, TypeParameterScope, TypeTruth, group_libraries,
    library_unit,
};
pub use rule::Rule;
pub use suppressions::{FileSuppressions, MALFORMED_SUPPRESSION, RuleLookup};
pub use visitor::RuleVisitor;
