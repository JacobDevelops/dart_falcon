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
use falcon_syntax::cst::{
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
use types::*;
pub use types::{
    FunctionTypeAlias, FunctionTypeShape, ParameterOwner, ResolvedType, TypeParameterId,
};
mod model;
pub(crate) use model::AliasTemplates;
pub use model::{SemanticModel, TypeParameterScope};
mod inference_call;
use inference_call::*;
pub use inference_call::{
    InferenceContext, expression_has_dot_shorthand_base, expression_segments, null_shorted,
    resolve_call_constructor, resolve_new_constructor,
};
mod substitution;
use substitution::*;
mod index;
use index::*;
pub use index::{
    ConstConstructorBody, ConstructorFacts, DeclarationFacts, MemberFacts, ParameterContract,
    ParameterDefault, ParameterFacts, ResolvedSignature, StaticConstFacts,
};
mod index_fixpoint;
use index_fixpoint::*;
mod index_build;
use index_build::*;
mod index_sdk;
use index_sdk::*;
mod index_hierarchy;
mod index_members;
mod inference_env;
pub use inference_env::TypeEnvironment;
use inference_env::*;
mod inference_expr;
mod inference_function;
use inference_function::*;
mod inference_collections;
use inference_collections::*;
mod join;
pub use index::SignatureIndex;
pub(super) use join::operator_result;
use join::*;

#[cfg(test)]
mod fixpoint_bench;
#[cfg(test)]
mod relation_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod type_graph_tests;
