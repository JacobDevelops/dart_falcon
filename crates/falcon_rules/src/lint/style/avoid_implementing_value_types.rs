//! Flags `implements` of a class that overrides `==`.
//!
//! A class that overrides `==` is a value type: its equality depends on private
//! state and invariants that an implementing class cannot see. Implementing its
//! interface yields an object that claims to be equal-comparable with the
//! original but cannot honor that contract, so `==` becomes asymmetric. Extend
//! the value type, or compose it, instead of implementing it.
//!
//! ```dart
//! class Size {
//!   final int inBytes;
//!   const Size(this.inBytes);
//!   @override
//!   bool operator ==(Object other) => other is Size && other.inBytes == inBytes;
//!   @override
//!   int get hashCode => inBytes.hashCode;
//! }
//!
//! class CustomSize implements Size { ... } // flagged
//! ```
//!
//! Only class declarations are checked. An inherited or abstract `==`,
//! including one supplied by a mixin, counts when its declaring library is
//! not `dart:core`. Unresolved external interfaces remain silent.
//!
//! Adopted from the official `avoid_implementing_value_types` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel, SignatureIndex};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidImplementingValueTypes;

impl Rule for AvoidImplementingValueTypes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let implements = match decl {
                TopLevelDecl::Class(class) => &class.implements,
                _ => continue,
            };
            for interface in implements {
                if overrides_equals(interface, signatures, &model) {
                    diags.push(ctx.diagnostic(
                        "avoid-implementing-value-types",
                        "Classes that override '==' should not be implemented.",
                        *interface.span(),
                    ));
                }
            }
        }
        diags
    }
}

/// Whether the interface's `==` is declared outside `dart:core`.
fn overrides_equals(
    interface: &DartType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> bool {
    let resolved = model.resolve_type(interface);
    let Some(facts) = signatures.resolved_member_facts(&resolved, "==", model) else {
        return false;
    };
    facts.first().is_some_and(|fact| {
        !matches!(&fact.declaring_type, DeclarationIdentity::Sdk { library, .. } if *library == "dart:core")
    })
}
