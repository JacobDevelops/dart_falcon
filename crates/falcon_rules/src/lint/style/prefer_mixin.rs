//! Flags `with` clauses that mix in a plain class instead of a mixin.
//!
//! Dart 3 only lets `mixin` and `mixin class` declarations be mixed in from
//! other libraries; mixing in an ordinary class is a legacy pattern that also
//! couples the class's constructor-free shape to every user. Declare the
//! reusable behavior as a `mixin`.
//!
//! ```dart
//! class Logger { void log(String m) {} }
//! class Service with Logger {}  // flagged
//!
//! mixin Logging { void log(String m) {} }
//! class Worker with Logging {}  // preferred
//! ```
//!
//! Only project classes that resolve to a unique plain `class` declaration are
//! reported; unresolved or SDK types are left alone.
//!
//! Adopted from the official `prefer_mixin` lint.

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferMixin;

impl Rule for PreferMixin {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(types)) = (ctx.identities, ctx.types) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let with_clause = match decl {
                TopLevelDecl::Class(node) => &node.with_clause,
                TopLevelDecl::ClassTypeAlias(node) => &node.with_clause,
                TopLevelDecl::MixinClass(node) => &node.with_clause,
                TopLevelDecl::Enum(node) => &node.with_clause,
                _ => continue,
            };
            for mixin in with_clause {
                let is_plain_class = matches!(
                    model.resolve_type(mixin),
                    ResolvedType::Interface {
                        identity: DeclarationIdentity::Project { name, .. },
                        ..
                    } if types.type_kind(&name) == Some(TypeKind::Class)
                );
                if is_plain_class {
                    diags.push(ctx.diagnostic(
                        "prefer-mixin",
                        "Only mixins should be mixed in.",
                        *mixin.span(),
                    ));
                }
            }
        }
        diags
    }
}
