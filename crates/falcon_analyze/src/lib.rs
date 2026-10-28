//! Shared rule contracts, semantic queries, and resolving analysis entrypoints.
//!
//! Implementation modules are private; consumers import the supported contracts
//! and queries directly from this crate.

mod const_contexts;
mod constant_value;
mod context;
mod cross_file;
mod environment;
mod function_bodies;
mod intern;
mod memo;
mod parallel;
mod registry;
mod resolve;
mod rule;
mod semantic_indexes;
mod semantic_scope;
mod source;
mod structural_scope;
mod suppressions;

pub use const_contexts::ConstContexts;
pub use constant_value::{
    ConstantCache, ConstantContext, ConstantValue, LocalConstants, Number,
    class_has_primitive_equality, constant_cast_validity, constant_operands_all, contextual_number,
    evaluate_constant, evaluate_constant_name, evaluate_construction, evaluate_pattern_constant,
    invocation_type_args, parse_int,
};
pub use context::AnalyzeContext;
pub use cross_file::{
    CrossFileRule, CrossFileRuleAnalysis, CrossFileRuleRegistry, NodeRef, ParsedFile, ProjectFile,
    SemanticIndexes,
};
pub use environment::packages::{
    FilePackage, PackageMetadata, canonical_package_path, enclosing_pubspec, package_identities,
    package_identity,
};
pub use environment::{
    AnalysisEnvironment, DartSdk, EnvironmentCacheKey, ExternalLibraries, ExternalOrigin,
    ExternalTarget, Feature, LanguageVersion, PackageConfig, SdkLocator, percent_decode,
    percent_encode,
};
pub use function_bodies::{BodyOwner, FunctionBodies};
pub use intern::InternedString;
pub use parallel::{
    Workspace, analyze_parallel_collecting_resolving, analyze_sequential_collecting_resolving,
    source_size_diagnostic, syntax_error_diagnostics,
};
pub(crate) use registry::with_rules_stack;
pub use registry::{RuleRegistry, rules_thread_diagnostic};
pub use resolve::flow::FlowContext;
pub use resolve::lexical_usage::{
    LocalWrite, LocalWriteKind, locally_bound_identifiers, used_constructor_parameters,
    used_parameters, used_primary_constructor_parameters,
};
pub use resolve::{
    DeclarationFacts, DeclarationIdentity, DocName, ExternalSources, FunctionTypeShape,
    IdentityIndex, IdentitySource, LibraryGrouping, LibraryId, LibrarySource, MemberFacts,
    MemberKind, MemberResult, MemberSelector, MemberSelectorKind, NameIdentity, PackageIdentity,
    ParameterContract, ParameterDefault, QualifiedName, ResolvedInvocation, ResolvedSignature,
    ResolvedType, ResolvedTypeView, SemanticModel, SignatureIndex, SourceSelector, Truth,
    TypeIndex, TypeKind, TypeParameterId, TypeParameterScope, TypeRelation, canonical_or_lexical,
    doc_imports, expression_segments, group_libraries, has_immutable_annotation, import_path,
    is_generated_dart_file, lexical_normalize, library_doc_imports, library_unit, library_unit_by,
    resolve_call_constructor, resolve_new_constructor,
};
pub(crate) use resolve::{
    InferenceContext, ParameterOwner, TypeEnvironment, expression_has_dot_shorthand_base,
};
pub use rule::{Rule, RuleAnalysis, RulePreparation};
pub use semantic_indexes::{ExternalIndexParts, build_semantic_indexes, update_semantic_indexes};
pub use semantic_scope::{
    CascadeAssignment, SemanticRuleVisitor, SemanticState, explicit_call_receiver,
    resolved_super_param_type, returned_expressions, visit_program,
};
pub use source::{is_source_too_large, read_source};
#[doc(hidden)]
pub use structural_scope::StructuralInterests;
pub use structural_scope::{StructuralNode, StructuralRuleVisitor, StructuralTraversal};
pub use suppressions::{FileSuppressions, RuleLookup};
