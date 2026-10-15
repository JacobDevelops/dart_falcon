//! Flags `==` and `hashCode` overrides in classes not marked `@immutable`.
//!
//! A value-based `==` and `hashCode` on a mutable object is a trap: once the
//! object is stored in a `Set` or used as a `Map` key, mutating it changes its
//! hash code and the collection silently loses track of it. Only classes that
//! are (transitively) annotated with package:meta's `@immutable` should
//! override them.
//!
//! ```dart
//! class Point {                 // not @immutable
//!   int x;
//!   Point(this.x);
//!   @override
//!   bool operator ==(Object other) => other is Point && other.x == x; // flagged
//!   @override
//!   int get hashCode => x.hashCode; // flagged
//! }
//! ```
//!
//! The annotation may sit on the class itself or on any supertype (superclass,
//! mixin, or interface). Unresolved supertypes leave immutability unknown.
//!
//! Adopted from the official `avoid_equals_and_hash_code_on_mutable_classes`
//! lint.

use falcon_analyze::{AnalyzeContext, Rule, SemanticModel, Truth, has_immutable_annotation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::declarations::first_token_after_annotations;

#[derive(Default)]
pub struct AvoidEqualsAndHashCodeOnMutableClasses;

impl Rule for AvoidEqualsAndHashCodeOnMutableClasses {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (name, annotations, members, has_supertypes) = match decl {
                TopLevelDecl::Class(class) => (
                    &class.name.name,
                    &class.annotations,
                    &class.members,
                    class.extends.is_some()
                        || !class.with_clause.is_empty()
                        || !class.implements.is_empty(),
                ),
                TopLevelDecl::MixinClass(class) => (
                    &class.name.name,
                    &class.annotations,
                    &class.members,
                    class.extends.is_some()
                        || !class.with_clause.is_empty()
                        || !class.implements.is_empty(),
                ),
                _ => continue,
            };
            let immutable = match (ctx.identities, ctx.signatures) {
                (Some(identities), Some(signatures)) => {
                    SemanticModel::new(ctx.file_path, identities, ctx.types)
                        .resolve_name(&**name)
                        .map_or(Truth::Unknown, |identity| {
                            signatures.has_immutable_supertype(&identity, false)
                        })
                }
                _ if has_immutable_annotation(annotations, program) => Truth::Yes,
                _ if !has_supertypes => Truth::No,
                _ => Truth::Unknown,
            };
            if immutable != Truth::No {
                continue;
            }
            for member in members {
                let (member_name, annotations, span) = match member {
                    ClassMember::Operator(op) if op.op == "==" => ("==", &op.annotations, op.span),
                    ClassMember::Getter(getter) if getter.name.name == "hashCode" => {
                        ("hashCode", &getter.annotations, getter.span)
                    }
                    ClassMember::Method(method) if method.name.name == "hashCode" => {
                        ("hashCode", &method.annotations, method.span)
                    }
                    _ => continue,
                };
                let start = first_token_after_annotations(annotations, span, ctx.source);
                diags.push(ctx.diagnostic(
                    "avoid-equals-and-hash-code-on-mutable-classes",
                    format!(
                        "The method '{member_name}' should not be overridden in classes not annotated with '@immutable'."
                    ),
                    Span::new(start, span.end),
                ));
            }
        }
        diags
    }
}
