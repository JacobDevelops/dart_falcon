//! Minimal type-resolution layer.
//!
//! Falcon has no full semantic model (no symbol tables, scopes, or type
//! inference). This module is the *ponytail* minimum that lets three otherwise
//! off-by-default rules run reliably — it is deliberately **not** a Dart type
//! checker:
//!
//! * [`LocalTypes`] — a file-local, per-function-body scope tracker that answers
//!   "what is the static type of this expression?" for the handful of cases a
//!   rule can trust without a resolver (declared types + trivial initializer
//!   inference).
//! * [`ProjectIndex`] — a cross-file declaration index mapping a declaration
//!   name to its declared return type, plus a curated builtin table for Dart
//!   core-library members whose return types matter to `avoid-ignoring-return-values`.
//!
//! Guiding invariant: **never report a type fact that could be wrong.** Every
//! inference returns [`StaticType::Unknown`] the moment certainty is lost. This
//! trades precision for soundness on purpose — a rule consuming this layer may
//! miss violations, but it must not fire a false positive because of a bad
//! type fact.

mod local_types;
mod project_index;

pub use local_types::LocalTypes;
pub use project_index::{ProgramSource, ProjectIndex};

use falcon_syntax::ast::DartType;

/// A coarse static type — a tiny subset of the Dart type system, only what the
/// resolver-dependent rules actually need.
///
/// This is intentionally not a general type representation: generic arguments,
/// type parameters, and structural types collapse into [`StaticType::Other`] or
/// [`StaticType::Unknown`]. Nullability is tracked for the core scalar types
/// (and `Other`) because `bool?` vs `bool` is the exact distinction that makes
/// `no-boolean-literal-compare` unsafe without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticType {
    Bool {
        nullable: bool,
    },
    Int {
        nullable: bool,
    },
    Double {
        nullable: bool,
    },
    Num {
        nullable: bool,
    },
    String {
        nullable: bool,
    },
    /// The `void` type. A distinct, *known* fact (unlike [`StaticType::Unknown`]).
    Void,
    /// The `dynamic` type.
    Dynamic,
    /// Type is genuinely unknown / could not be resolved soundly.
    Unknown,
    /// Any other named type (classes, `Future`, `List`, enums, …).
    Other {
        name: String,
        nullable: bool,
    },
}

impl StaticType {
    /// Whether this type is (statically known to be) nullable. [`StaticType::Unknown`]
    /// and [`StaticType::Dynamic`] are treated as *possibly* nullable and return
    /// `true` — callers that need certainty should match on the variant.
    pub fn is_nullable(&self) -> bool {
        match self {
            StaticType::Bool { nullable }
            | StaticType::Int { nullable }
            | StaticType::Double { nullable }
            | StaticType::Num { nullable }
            | StaticType::String { nullable }
            | StaticType::Other { nullable, .. } => *nullable,
            StaticType::Void => false,
            StaticType::Dynamic | StaticType::Unknown => true,
        }
    }

    /// `true` only for a *non-nullable* `bool`. This is the precise predicate
    /// `no-boolean-literal-compare` needs: `x == true` is redundant only when `x`
    /// is a non-nullable bool (for `bool?` it is the idiomatic null-safe form).
    pub fn is_non_nullable_bool(&self) -> bool {
        matches!(self, StaticType::Bool { nullable: false })
    }

    /// `true` for the known `void` type.
    pub fn is_void(&self) -> bool {
        matches!(self, StaticType::Void)
    }

    /// Return a copy with the given nullability, where the variant carries one.
    /// Variants without a nullability flag (`Void`, `Dynamic`, `Unknown`) are
    /// returned unchanged.
    pub fn with_nullable(&self, nullable: bool) -> StaticType {
        match self {
            StaticType::Bool { .. } => StaticType::Bool { nullable },
            StaticType::Int { .. } => StaticType::Int { nullable },
            StaticType::Double { .. } => StaticType::Double { nullable },
            StaticType::Num { .. } => StaticType::Num { nullable },
            StaticType::String { .. } => StaticType::String { nullable },
            StaticType::Other { name, .. } => StaticType::Other {
                name: name.clone(),
                nullable,
            },
            other => other.clone(),
        }
    }

    /// Map a syntactic [`DartType`] to a coarse [`StaticType`].
    ///
    /// Core scalar names map to their dedicated variants (carrying the written
    /// nullability); `void`/`dynamic` map to their variants; every other named
    /// type becomes [`StaticType::Other`]. Function and record types are not
    /// modeled — they collapse to `Other` with a synthetic name so nullability
    /// is still available, which is all any consumer needs.
    pub fn from_dart_type(ty: &DartType) -> StaticType {
        match ty {
            DartType::Void { .. } => StaticType::Void,
            DartType::Dynamic { .. } => StaticType::Dynamic,
            DartType::Never { .. } => StaticType::Unknown,
            DartType::Function(f) => StaticType::Other {
                name: "Function".to_string(),
                nullable: f.is_nullable,
            },
            DartType::Record(r) => StaticType::Other {
                name: "Record".to_string(),
                nullable: r.is_nullable,
            },
            DartType::Named(n) => {
                let nullable = n.is_nullable;
                // Use the final segment ("prefix.Type" → "Type") as the name.
                let name = n.segments.last().map(|id| id.name.as_str()).unwrap_or("");
                match name {
                    "bool" => StaticType::Bool { nullable },
                    "int" => StaticType::Int { nullable },
                    "double" => StaticType::Double { nullable },
                    "num" => StaticType::Num { nullable },
                    "String" => StaticType::String { nullable },
                    "void" => StaticType::Void,
                    "dynamic" => StaticType::Dynamic,
                    "Never" => StaticType::Unknown,
                    "" => StaticType::Unknown,
                    other => StaticType::Other {
                        name: other.to_string(),
                        nullable,
                    },
                }
            }
        }
    }
}
