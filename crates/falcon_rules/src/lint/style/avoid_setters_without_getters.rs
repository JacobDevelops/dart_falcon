//! Flags a setter that has no corresponding getter.
//!
//! A property that can be written but not read is surprising: `a.x = 1;`
//! compiles, but `a.x` does not. Each setter of a class, enum, or extension
//! type needs a getter of the same name, declared alongside it (a getter or a
//! field) or inherited from a superclass:
//!
//! ```dart
//! class A {
//!   set width(int value) {}    // flagged
//!
//!   int get height => 0;
//!   set height(int value) {}   // fine
//! }
//! ```
//!
//! Setters that override an inherited setter are not reported. Inheritance is
//! resolved through falcon's project index; when a supertype cannot be
//! resolved the rule stays silent, and any inherited getter — even one only
//! inherited from an interface — counts as a match.
//!
//! Adopted from the official `avoid_setters_without_getters` lint.

use falcon_analyze::{AnalyzeContext, Rule, SemanticModel, SignatureIndex};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::SemanticState;

/// Concrete getters every type inherits from `Object`.
const OBJECT_GETTERS: [&str; 2] = ["hashCode", "runtimeType"];

#[derive(Default)]
pub struct AvoidSettersWithoutGetters;

impl Rule for AvoidSettersWithoutGetters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let (name, members) = match declaration {
                TopLevelDecl::Class(node) => (&node.name, &node.members),
                TopLevelDecl::MixinClass(node) => (&node.name, &node.members),
                TopLevelDecl::Enum(node) => (&node.name, &node.members),
                TopLevelDecl::ExtensionType(node) => (&node.name, &node.members),
                TopLevelDecl::ClassTypeAlias(_)
                | TopLevelDecl::Mixin(_)
                | TopLevelDecl::Extension(_)
                | TopLevelDecl::Function(_)
                | TopLevelDecl::Variable(_)
                | TopLevelDecl::TypeAlias(_)
                | TopLevelDecl::Error(_) => continue,
            };
            check_members(
                &name.name,
                members,
                &state.model,
                &state.signatures,
                ctx,
                &mut diags,
            );
        }
        diags
    }
}

fn declares_getter(members: &[ClassMember], name: &str) -> bool {
    members.iter().any(|member| match member {
        ClassMember::Getter(getter) => getter.name.name == name,
        ClassMember::Field(field) => field
            .declarators
            .iter()
            .any(|declarator| declarator.name.name == name),
        ClassMember::Constructor(_)
        | ClassMember::Method(_)
        | ClassMember::Setter(_)
        | ClassMember::Operator(_)
        | ClassMember::Error(_) => false,
    })
}

fn check_members(
    owner: &str,
    members: &[ClassMember],
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let owner = model.resolve_name(owner);
    for member in members {
        let ClassMember::Setter(setter) = member else {
            continue;
        };
        let name = setter.name.name.as_str();
        let overrides = setter
            .annotations
            .iter()
            .any(|annotation| annotation.name.last().is_some_and(|n| n.name == "override"));
        if overrides || declares_getter(members, name) || OBJECT_GETTERS.contains(&name) {
            continue;
        }
        let Some(inherited) = owner
            .as_ref()
            .and_then(|owner| signatures.inherited_member_facts(owner, name))
        else {
            continue;
        };
        let has_inherited = inherited
            .iter()
            .any(|fact| fact.kind.has_setter() || fact.kind.has_getter());
        if !has_inherited {
            diags.push(ctx.diagnostic(
                "avoid-setters-without-getters",
                "Setter has no corresponding getter.",
                setter.name.span,
            ));
        }
    }
}
