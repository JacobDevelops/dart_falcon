//! Flags methods that return `this` only to enable a fluent interface.
//!
//! Dart's cascade operator (`..`) chains calls on the same receiver without any
//! cooperation from the API, so a method returning its own receiver adds
//! nothing but a misleading return type. Return `void` and let callers cascade.
//!
//! ```dart
//! class Builder {
//!   Builder add(int x) {        // flagged at `return this`
//!     _items.add(x);
//!     return this;
//!   }
//!   Builder clear() => this;    // flagged at `clear`
//! }
//! ```
//!
//! Only methods whose declared return type is the enclosing class, enum, or
//! mixin and whose every `return` yields an unparenthesized `this` are reported.
//! Methods that override an inherited member are exempt.
//!
//! Adopted from the official `avoid_returning_this` lint.

use falcon_analyze::{AnalyzeContext, MemberKind, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::returned_expressions;

#[derive(Default)]
pub struct AvoidReturningThis;

impl Rule for AvoidReturningThis {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let returns_this = |expr: &Expr| matches!(expr, Expr::This { .. });
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (name, members) = match decl {
                TopLevelDecl::Class(node) => (&node.name.name, &node.members),
                TopLevelDecl::MixinClass(node) => (&node.name.name, &node.members),
                TopLevelDecl::Enum(node) => (&node.name.name, &node.members),
                TopLevelDecl::Mixin(node) => (&node.name.name, &node.members),
                _ => continue,
            };
            let Some(owner) = model.resolve_name(name.as_str()) else {
                continue;
            };
            for member in members {
                let (member_name, return_type, body, annotations, getter) = match member {
                    ClassMember::Method(method) => (
                        &method.name,
                        &method.return_type,
                        &method.body,
                        &method.annotations,
                        false,
                    ),
                    ClassMember::Getter(getter) => (
                        &getter.name,
                        &getter.return_type,
                        &getter.body,
                        &getter.annotations,
                        true,
                    ),
                    _ => continue,
                };
                let (Some(return_type), Some(body)) = (return_type, body) else {
                    continue;
                };
                if !names_type(return_type, name) || has_override(annotations) {
                    continue;
                }
                let overrides = signatures
                    .inherited_member_facts(&owner, &member_name.name)
                    .unwrap_or_default()
                    .iter()
                    .any(|fact| {
                        if getter {
                            fact.kind.has_getter()
                        } else {
                            fact.kind == MemberKind::Method
                        }
                    });
                if overrides {
                    continue;
                }
                let span = match body {
                    FunctionBody::Arrow(expr, _) => returns_this(expr).then_some(member_name.span),
                    FunctionBody::Block(_) => {
                        let returned = returned_expressions(body);
                        (!returned.is_empty() && returned.iter().all(|expr| returns_this(expr)))
                            .then(|| *returned[0].span())
                    }
                    FunctionBody::Native(..) => None,
                };
                if let Some(span) = span {
                    diags.push(ctx.diagnostic(
                        "avoid-returning-this",
                        "Don't return 'this' from a method.",
                        span,
                    ));
                }
            }
        }
        diags
    }
}

/// Whether `ty` names the enclosing declaration (type arguments and
/// nullability do not change the referenced element).
fn names_type(ty: &DartType, name: &str) -> bool {
    matches!(ty, DartType::Named(named)
        if matches!(named.segments.as_slice(), [segment] if segment.name == name))
}

fn has_override(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|annotation| matches!(annotation.name.as_slice(), [name] if name.name == "override"))
}
