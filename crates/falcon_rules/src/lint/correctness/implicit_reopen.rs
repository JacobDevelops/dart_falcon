//! Flags classes that implicitly reopen a supertype declared in the same
//! library.
//!
//! Class modifiers are only enforced across library boundaries, so inside its
//! own library an `interface` or `final` class can be extended by a class that
//! drops the restriction — silently letting other libraries extend what the
//! author meant to close. Mark the subclass with the same modifier, or annotate
//! it with package:meta's `@reopen` when reopening is intended.
//!
//! ```dart
//! interface class I {}
//! class C extends I {}          // flagged: reopens 'I' (not marked 'interface')
//!
//! final class F {}
//! base class B extends F {}     // flagged: reopens 'F' (not marked 'final')
//! ```
//!
//! Sealed and `mixin` classes and classes annotated with `@reopen` are exempt.
//! Only superclasses declared in the same library (this file or its parts) are
//! considered, including named mixin applications. A sealed superclass carries
//! the modifier its own supertypes induce (a sealed subclass of an `interface`
//! class is an interface).
//!
//! ANALYZER-BUG: the analyzer can miss reopening when a sealed class is declared
//! before its sealed superclass. Falcon computes the induced restriction
//! independently of that declaration order.
//!
//! Adopted from the official `implicit_reopen` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ImplicitReopen;

impl Rule for ImplicitReopen {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            program,
            ctx,
            model: ctx.identities.map(|identities| {
                SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types)
            }),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    model: Option<SemanticModel<'a>>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
        let siblings = ctx.library.map_or(&[][..], |library| library.siblings());
        let library = || std::iter::once(self.program).chain(siblings.iter().copied());
        let model = &self.model;
            let (name, mut annotations, modifiers, superclass) = match decl {
                TopLevelDecl::Class(node) => match node.extends() {
                    Some(superclass) => (node.name(), node.annotations(), node.modifiers(), superclass),
                    None => return StructuralTraversal::SkipChildren,
                },
                TopLevelDecl::ClassTypeAlias(node) => (
                    node.name(),
                    node.annotations(),
                    node.modifiers(),
                    node.superclass(),
                ),
                _ => return StructuralTraversal::SkipChildren,
            };
            if annotations
                .any(|annotation| annotation.name().next_back().is_some_and(|n| n.name() == "reopen"))
            {
                return StructuralTraversal::SkipChildren;
            }
            // A prefixed supertype comes from another library.
            let DartType::Named(named) = superclass else {
                return StructuralTraversal::SkipChildren;
            };
            let mut segments = named.segments();
            let (Some(super_name), None) = (segments.next(), segments.next()) else {
                return StructuralTraversal::SkipChildren;
            };
            let mut found = library()
                .flat_map(|program| program.declarations())
                .filter_map(|candidate| match candidate {
                    TopLevelDecl::Class(node) if node.name().name() == super_name.name() => {
                        Some(Some(node.modifiers()))
                    }
                    TopLevelDecl::ClassTypeAlias(node) if node.name().name() == super_name.name() => {
                        Some(Some(node.modifiers()))
                    }
                    TopLevelDecl::Mixin(node) if node.name().name() == super_name.name() => Some(None),
                    TopLevelDecl::MixinClass(node) if node.name().name() == super_name.name() => Some(None),
                    TopLevelDecl::Enum(node) if node.name().name() == super_name.name() => Some(None),
                    TopLevelDecl::ExtensionType(node) if node.name().name() == super_name.name() => Some(None),
                    TopLevelDecl::TypeAlias(node) if node.name().name() == super_name.name() => Some(None),
                    _ => None,
                });
            // Exactly one same-library class declaration must own the name.
            let (Some(Some(super_modifiers)), None) = (found.next(), found.next()) else {
                return StructuralTraversal::SkipChildren;
            };
            let super_restriction = match super_modifiers.restriction {
                Some(ClassRestriction::Sealed) => {
                    let induced =
                        model
                            .as_ref()
                            .zip(ctx.signatures)
                            .and_then(|(model, signatures)| {
                                signatures
                                    .effective_restriction(&model.resolve_name(super_name.name())?)
                            });
                    let Some(induced) = induced else {
                        return StructuralTraversal::SkipChildren;
                    };
                    induced
                }
                restriction => restriction,
            };
            let reason = match (modifiers.restriction, super_restriction) {
                (Some(ClassRestriction::Base), Some(ClassRestriction::Final)) => "final",
                (Some(ClassRestriction::Base) | None, Some(ClassRestriction::Interface)) => {
                    "interface"
                }
                _ => return StructuralTraversal::SkipChildren,
            };
            self.diags.push(ctx.diagnostic(
                "implicit-reopen",
                format!(
                    "The class '{}' reopens '{}' because it is not marked '{reason}'.",
                    name.name(), super_name.name()
                ),
                name.span(),
            ));
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
