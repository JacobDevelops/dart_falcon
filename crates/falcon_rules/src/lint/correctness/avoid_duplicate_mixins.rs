//! Flags repeated mixins in a with clause or a same-library superclass chain.
//! Implemented interfaces do not count. Unknown ancestry, super calls, and
//! intervening member overrides are skipped: reapplication can change behavior.

use crate::lint::declarations::{member_names, members_of};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst;
use std::collections::HashSet;

#[derive(Default)]
pub struct AvoidDuplicateMixins;

impl Rule for AvoidDuplicateMixins {
    fn prepare<'a>(
        &self,
        program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            program,
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    program: cst::Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn local_type(&self, type_: cst::DartType<'_>) -> Option<cst::TopLevelDecl<'a>> {
        let cst::DartType::Named(named) = type_ else {
            return None;
        };
        if named.is_nullable() {
            return None;
        }
        let mut segments = named.segments();
        let (Some(name), None) = (segments.next(), segments.next()) else {
            return None;
        };
        let siblings = self
            .ctx
            .library()
            .map_or(&[][..], |library| library.siblings());
        let mut found = std::iter::once(self.program)
            .chain(siblings.iter().copied())
            .flat_map(|program| program.declarations())
            .filter(|candidate| match candidate {
                cst::TopLevelDecl::Class(node) => node.name().name() == name.name(),
                cst::TopLevelDecl::ClassTypeAlias(node) => node.name().name() == name.name(),
                cst::TopLevelDecl::MixinClass(node) => node.name().name() == name.name(),
                cst::TopLevelDecl::Mixin(node) => node.name().name() == name.name(),
                _ => false,
            });
        match (found.next(), found.next()) {
            (Some(owner), None) => Some(owner),
            _ => None,
        }
    }

    fn mixin_members(
        &self,
        type_: cst::DartType<'_>,
    ) -> Option<cst::AstChildren<'a, cst::ClassMember<'a>>> {
        let owner = self.local_type(type_)?;
        match owner {
            cst::TopLevelDecl::Mixin(_) => {}
            cst::TopLevelDecl::MixinClass(node)
                if node.extends().is_none() && node.with_clause().next().is_none() => {}
            _ => return None,
        }
        // A super call can run a different implementation after reapplication.
        if owner
            .syntax()
            .descendants()
            .filter_map(cst::Expr::cast)
            .any(|expr| matches!(expr, cst::Expr::Super(_)))
        {
            return None;
        }
        members_of(owner)
    }

    fn inherited(
        &self,
        node: cst::TopLevelDecl<'a>,
        mixin: cst::DartType<'a>,
        names: &HashSet<&str>,
    ) -> bool {
        let siblings = self
            .ctx
            .library()
            .map_or(&[][..], |library| library.siblings());
        let limit = std::iter::once(self.program)
            .chain(siblings.iter().copied())
            .map(|program| program.declarations().count())
            .sum::<usize>();
        let mut parent = superclass(node);
        // The declaration count bounds malformed cycles without a visited set.
        for _ in 0..=limit {
            let Some(type_) = parent else { return false };
            let Some(owner) = self.local_type(type_) else {
                return false;
            };
            if same(type_, mixin) {
                return true;
            }
            if let Some(members) = members_of(owner) {
                if conflicts(members, names) {
                    return false;
                }
            } else if !matches!(owner, cst::TopLevelDecl::ClassTypeAlias(_)) {
                return false;
            }
            let cst::DartType::Named(named) = type_ else {
                return false;
            };
            if named.type_args().next().is_some() {
                return false;
            }
            let (applied, generic) = match owner {
                cst::TopLevelDecl::Class(node) => {
                    (node.with_clause(), node.type_params().next().is_some())
                }
                cst::TopLevelDecl::ClassTypeAlias(node) => {
                    (node.with_clause(), node.type_params().next().is_some())
                }
                cst::TopLevelDecl::MixinClass(node) => {
                    (node.with_clause(), node.type_params().next().is_some())
                }
                _ => return false,
            };
            if generic {
                return false;
            }
            // Only mixins after an earlier application can override its members.
            for applied in applied.rev() {
                if same(applied, mixin) {
                    return true;
                }
                let Some(members) = self.mixin_members(applied) else {
                    return false;
                };
                if conflicts(members, names) {
                    return false;
                }
            }
            parent = superclass(owner);
        }
        false
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: cst::TopLevelDecl<'a>) -> StructuralTraversal {
        let mixins = match node {
            cst::TopLevelDecl::Class(node) => node.with_clause(),
            cst::TopLevelDecl::ClassTypeAlias(node) => node.with_clause(),
            cst::TopLevelDecl::MixinClass(node) => node.with_clause(),
            cst::TopLevelDecl::Enum(node) => node.with_clause(),
            _ => return StructuralTraversal::Continue,
        };
        let count = mixins.clone().count();
        for (index, mixin) in mixins.clone().enumerate() {
            let redundant = self.mixin_members(mixin).is_some_and(|members| {
                let names = member_names(members);
                for earlier in mixins.clone().rev().skip(count - index) {
                    if same(earlier, mixin) { return true; }
                    let Some(members) = self.mixin_members(earlier) else { return false };
                    if conflicts(members, &names) { return false; }
                }
                self.inherited(node, mixin, &names)
            });
            if redundant {
                self.diags.push(self.ctx.diagnostic(
                    "This mixin is already applied without intervening overrides — remove it or replace it with the intended mixin.",
                    mixin.span(),
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
}
}

fn same(left: cst::DartType<'_>, right: cst::DartType<'_>) -> bool {
    crate::lint::support::syntax::same_syntax(left.syntax(), right.syntax())
}

fn conflicts(members: cst::AstChildren<'_, cst::ClassMember<'_>>, names: &HashSet<&str>) -> bool {
    members.into_iter().any(|member| match member {
        cst::ClassMember::Field(field) => field
            .declarators()
            .any(|declaration| names.contains(declaration.name().name())),
        cst::ClassMember::Method(method) => names.contains(method.name().name()),
        cst::ClassMember::Getter(getter) => names.contains(getter.name().name()),
        cst::ClassMember::Setter(setter) => names.contains(setter.name().name()),
        cst::ClassMember::Operator(_) | cst::ClassMember::Error(_) => true,
        _ => false,
    })
}

fn superclass(node: cst::TopLevelDecl<'_>) -> Option<cst::DartType<'_>> {
    match node {
        cst::TopLevelDecl::Class(node) => node.extends(),
        cst::TopLevelDecl::ClassTypeAlias(node) => Some(node.superclass()),
        cst::TopLevelDecl::MixinClass(node) => node.extends(),
        _ => None,
    }
}
