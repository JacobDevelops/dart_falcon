//! Rule contracts and parallel analysis engine.

pub mod constant_value;
pub mod context;
pub mod cross_file;
pub mod environment;
mod intern;
mod memo;
pub mod parallel;
pub mod registry;
pub mod resolve;
pub mod rule;
pub mod semantic_scope;
pub mod source;
pub mod structural_scope;
pub mod suppressions;

pub use constant_value::{
    ConstantCache, ConstantDeclaration, ConstantType, ConstantValue, Construction,
    evaluate_constant, evaluate_constant_cached, evaluate_constant_in_library,
    evaluate_constant_in_member_context, evaluate_constant_name_with_local_values,
    evaluate_constant_with_local_values, evaluate_construction,
    evaluate_construction_with_local_values, evaluate_pattern_constant, invocation_type_args,
    parse_int,
};
pub use context::AnalyzeContext;
pub use cross_file::{
    CrossFileRule, CrossFileRuleAnalysis, CrossFileRuleRegistry, NodeRef, ParsedFile, ProjectFile,
    SemanticIndexes,
};
pub use environment::{
    AnalysisEnvironment, ExternalLibraries, ExternalOrigin, Feature, LanguageVersion,
    PackageConfig, percent_decode,
};
pub use intern::InternedString;
pub use parallel::{
    Workspace, analyze_parallel, analyze_parallel_collecting,
    analyze_parallel_collecting_resolving, analyze_sequential, analyze_sequential_collecting,
    analyze_sequential_collecting_resolving, syntax_error_diagnostics,
};
pub use registry::{RuleRegistry, rules_thread_diagnostic, with_rules_stack};
pub use resolve::{
    DeclarationFacts, DeclarationIdentity, DefaultSyntaxKey, DocImport, DocName, ExternalSources,
    IdentityIndex, IdentitySource, InferenceContext, LibraryGrouping, LibraryId, LibrarySource,
    MemberFacts, MemberKind, MemberResult, MemberSelector, MemberSelectorKind, NameIdentity,
    PackageIdentity, ParameterContract, ParameterDefault, ParameterOwner, QualifiedName,
    ResolvedInvocation, ResolvedSignature, ResolvedType, SemanticModel, SignatureIndex,
    SourceSelector, Truth, TypeEnvironment, TypeIndex, TypeParameterId, TypeParameterScope,
    canonical_or_lexical, expression_has_dot_shorthand_base, expression_segments, group_libraries,
    has_immutable_annotation, import_path, library_doc_imports, library_unit, null_shorted,
    resolve_call_constructor, resolve_new_constructor,
};
pub use rule::{Rule, RuleAnalysis, RulePreparation};
pub use structural_scope::{
    StructuralInterests, StructuralKind, StructuralNode, StructuralRuleVisitor, StructuralTraversal,
};
pub use suppressions::{FileSuppressions, RuleLookup};
