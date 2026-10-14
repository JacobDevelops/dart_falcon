//! Rule contracts and parallel analysis engine.

pub mod build_context_flow;
pub mod constant_value;
pub mod context;
pub mod cross_file;
pub mod parallel;
pub mod registry;
pub mod resolve;
pub mod rule;
pub mod suppressions;

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
    DeclarationFacts, DeclarationIdentity, IdentityIndex, IdentitySource, LibraryGrouping,
    LibrarySource, LocalTypes, MemberFacts, MemberKind, MemberResult, NameIdentity,
    PackageIdentity, ProgramSource, ProjectIndex, ReceiverTypes, ResolvedSignature, ResolvedType,
    SemanticMemberKind, SemanticModel, SignatureIndex, StaticType, SubtypeResult, TypeEnvironment,
    TypeIndex, TypeParameterId, TypeParameterScope, TypeTruth, canonical_or_lexical,
    group_libraries, library_unit,
};
pub use rule::Rule;
pub use suppressions::{FileSuppressions, RuleLookup};
