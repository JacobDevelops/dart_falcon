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
//! `implements` clauses of classes, mixins, enums, and extension types are
//! checked. Only a concrete `==` declared by a class counts: an abstract `==`
//! or one supplied by a mixin is not reported, nor are `dart:core` classes
//! (whose `==` is the platform's). The rule stays silent when the implemented
//! type's member chain cannot be resolved.
//!
//! Adopted from the official `avoid_implementing_value_types` lint.

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, SemanticModel, SignatureIndex,
};
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
                TopLevelDecl::ClassTypeAlias(alias) => &alias.implements,
                TopLevelDecl::MixinClass(class) => &class.implements,
                TopLevelDecl::Mixin(mixin) => &mixin.implements,
                TopLevelDecl::Enum(enumeration) => &enumeration.implements,
                TopLevelDecl::ExtensionType(extension) => &extension.implements,
                _ => continue,
            };
            for interface in implements {
                if overrides_equals(interface, signatures, &model, ctx) {
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

/// Whether the nearest `==` of `interface` is a concrete operator declared by a
/// project class (not a mixin, and not `Object`'s). An abstract nearest
/// declaration or an unresolved chain stays silent.
fn overrides_equals(
    interface: &DartType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    ctx: &AnalyzeContext,
) -> bool {
    let resolved = model.resolve_type(interface);
    let Some(facts) = signatures.resolved_member_facts(&resolved, "==", model) else {
        return false;
    };
    let concrete = facts
        .iter()
        .any(|fact| fact.kind == MemberKind::Operator && !fact.is_static && !fact.is_abstract);
    let Some(owner) = facts.first().map(|fact| &fact.declaring_type) else {
        return false;
    };
    concrete
        && match owner {
            DeclarationIdentity::Project { name, .. } => ctx.types.is_some_and(|types| {
                matches!(
                    types.type_kind(name),
                    Some(TypeKind::Class | TypeKind::MixinClass)
                )
            }),
            DeclarationIdentity::Sdk { .. } => false,
            DeclarationIdentity::Package { .. } => true,
        }
}
