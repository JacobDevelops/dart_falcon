//! Requires `@override` on members that override an inherited declaration.
//!
//! Adopted from the official `annotate_overrides` lint.
//! An instance method, operator, getter, setter, or field of a class, mixin
//! class, enum, or mixin
//! is reported when its inherited interface has a combined member signature
//! for that name — a getter for fields and getters, a setter for setters.
//! Conflicting inherited signatures with no most-specific existing candidate
//! do not count, even when this declaration validly overrides all candidates.
//! Extension-type members are never checked. Unloaded dependency signatures
//! remain unknown rather than being inferred from member names.
//!
//! Declaring `var`/`final` primary-constructor parameters are also instance
//! fields and are checked as such.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::declarations::annotation;
use crate::lint::declarations::operator_token_span;
use crate::lint::inherited_members::inheriting_declaration;

#[derive(Default)]
pub struct AnnotateOverrides;

impl Rule for AnnotateOverrides {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let Some(declaration) = inheriting_declaration(decl) else {
            return StructuralTraversal::SkipChildren;
        };
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let diags = &mut self.diags;
        let mut report = |owner: &DeclarationIdentity, name: &str, kind: MemberKind, span: Span| {
            let overrides = signatures.has_combined_inherited_member(owner, name, kind, model);
            if overrides {
                diags.push(ctx.diagnostic(
                    "Annotate overridden members with `@override`.",
                    span,
                ));
            }
        };
        let Some(owner) = model.resolve_name(declaration.name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        for member in declaration.members.clone() {
            match member {
                ClassMember::Field(field)
                    if !field.is_static() && !annotation(field.annotations(), "override") =>
                {
                    for declarator in field.declarators() {
                        report(
                            &owner,
                            declarator.name().name(),
                            MemberKind::field(field, declarator),
                            declarator.name().span(),
                        );
                    }
                }
                ClassMember::Method(method)
                    if !method.is_static() && !annotation(method.annotations(), "override") =>
                {
                    report(
                        &owner,
                        method.name().name(),
                        MemberKind::Method,
                        method.name().span(),
                    );
                }
                ClassMember::Getter(getter)
                    if !getter.is_static() && !annotation(getter.annotations(), "override") =>
                {
                    report(
                        &owner,
                        getter.name().name(),
                        MemberKind::Getter,
                        getter.name().span(),
                    );
                }
                ClassMember::Setter(setter)
                    if !setter.is_static() && !annotation(setter.annotations(), "override") =>
                {
                    report(
                        &owner,
                        setter.name().name(),
                        MemberKind::Setter,
                        setter.name().span(),
                    );
                }
                ClassMember::Operator(operator)
                    if !annotation(operator.annotations(), "override") =>
                {
                    report(
                        &owner,
                        operator.lookup_name(),
                        MemberKind::Operator,
                        operator_token_span(operator),
                    );
                }
                ClassMember::Field(_)
                | ClassMember::Method(_)
                | ClassMember::Getter(_)
                | ClassMember::Setter(_)
                | ClassMember::Operator(_)
                | ClassMember::Constructor(_)
                | ClassMember::PrimaryConstructorBody(_)
                | ClassMember::Error(_) => {}
            }
        }
        // Declaring `var`/`final` primary-constructor parameters are fields.
        for param in (EffectiveMembers {
            declared: declaration.members,
            primary: declaration.primary_constructor,
            representation: false,
        }
        .declaring_params()
        .filter(|param| !annotation(param.annotations(), "override")))
        {
            report(
                &owner,
                param.name().name(),
                MemberKind::Field {
                    writable: param.is_var(),
                    covariant: param.is_covariant(),
                },
                param.name().span(),
            );
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
