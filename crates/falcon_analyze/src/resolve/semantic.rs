//! Canonical semantic facts shared by resolver-dependent rules.
//!
//! This is deliberately conservative: every unresolved or ambiguous identity
//! becomes `Unknown`, and consumers diagnose only from proven facts.

use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::{Borrow, Cow};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use falcon_syntax::cst::visit::{Visitor, walk_expr, walk_stmt};
pub(super) use falcon_syntax::cst::{
    Annotation, AstChildren, BinaryOp, ClassMember, ClassRestriction, DartType, EffectiveMembers,
    Expr, FormalParam, FormalParamList, FunctionBody, FunctionKind, InitializingFormal, NodeId,
    ParamKind, Program, Span, TopLevelDecl, TypeParam, UnaryOp, VarKeyword,
};

use super::identity::TypeAlias;
use super::{
    DeclarationIdentity, FileId, IdentityIndex, LibraryId, MemberKind, NameIdentity, QualifiedName,
    Truth, TypeIndex, TypeKind,
};
use crate::environment::ExternalLibraries;

mod default_syntax;
pub use default_syntax::DefaultSyntaxKey;

mod inherited;
mod queries;
pub use queries::ResolvedInvocation;
mod relation;
mod unrelated;
pub use relation::TypeRelation;

#[cfg(test)]
mod alias_tests;

use super::{DocName, flow, flow_writes, identity};
mod types;
pub(super) use types::has_inference_holes;

use types::{
    AliasComparison, AliasPresenceKey, AliasProvenance, AliasTemplate, BorrowedHashKey,
    BorrowedIdentity, SharedHashKey, collect_parameter_ids, empty_named_types,
    empty_type_arguments, find_type_including_bounds, parameter_bound, refine_inferred_type,
};
pub(crate) use types::{FunctionTypeAlias, ResolvedTypeKind};
pub use types::{
    FunctionTypeShape, ParameterOwner, ResolvedType, ResolvedTypeView, TypeParameterId,
};
mod model;
pub(crate) use model::AliasTemplates;
pub use model::{SemanticModel, TypeParameterScope};
mod inference_call;
pub use inference_call::{
    InferenceContext, expression_has_dot_shorthand_base, expression_segments, null_shorted,
    resolve_call_constructor, resolve_new_constructor,
};
use inference_call::{
    TypeConstraintContext, constructed_identity, expression_qualified_name, infer_type_constraints,
    inference_candidate,
};
mod substitution;
use substitution::{
    SubstitutionKey, SubstitutionMemo, SubstitutionSourceKey, instantiate_parameter_bound,
};
mod index;
pub use index::{
    ConstConstructorBody, ConstructorFacts, DeclarationFacts, MemberFacts, ParameterContract,
    ParameterDefault, ParameterFacts, ResolvedSignature, StaticConstFacts,
};
use index::{
    ExtensionFacts, ExtensionMemberName, ExtensionMembers, InstanceAccess, MemberEntry,
    SignatureMap, SuperFormal, TypeFacts, empty_parameter_contracts, setter_parameter_contracts,
};
mod index_fixpoint;
use index_fixpoint::initializer_value_name;
mod index_build;
use index_build::{SignatureContext, inherits_method_types, mentions_any, same_library, signature};
mod index_sdk;
use index_sdk::SDK_INTERFACE_MEMBERS;
#[cfg(test)]
use index_sdk::sdk_parameter;
mod index_hierarchy;
mod index_members;
mod inference_env;
use inference_env::FunctionSyntax;
pub use inference_env::TypeEnvironment;
mod inference_expr;
mod inference_function;
use inference_function::ClosureReturns;
mod inference_collections;
use inference_collections::CollectionTypes;
mod join;
pub use index::SignatureIndex;
pub(super) use join::operator_result;
#[cfg(test)]
use join::{InferenceJoinMemo, inference_join_memoized};
pub(crate) use join::{binary_result, null_coalesce_result};
use join::{
    contextual_join, fixnum_and_int, inference_join, numeric, operator_parameter_type,
    operator_return_type, top_rank,
};

#[cfg(test)]
mod fixpoint_bench;
#[cfg(test)]
mod relation_tests;
#[cfg(test)]
pub(super) mod tests;
#[cfg(test)]
mod type_graph_tests;
