//! Warns when a StatefulWidget's local State contains only a pure build method.
//!
//! Both declarations must resolve to Flutter types in this file. State fields,
//! mixins, lifecycle methods, extra instance members, and any State member use
//! other than widget conservatively retain the StatefulWidget. Inherited custom
//! State bases are ignored because they may own lifecycle behavior.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryStatefulWidgets;
impl Rule for AvoidUnnecessaryStatefulWidgets {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            widgets: Vec::new(),
            states: Vec::new(),
            current: None,
            necessary: false,
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    widgets: Vec<(DeclarationIdentity, Span)>,
    states: Vec<DeclarationIdentity>,
    current: Option<DeclarationIdentity>,
    necessary: bool,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.widgets.iter().filter(|(identity, _)| self.states.contains(identity)).map(|(_, span)| self.ctx.diagnostic("This widget has no state or lifecycle behavior; use `StatelessWidget`.", *span)).collect()
    }
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.current = None;
        self.necessary = false;
        let TopLevelDecl::Class(class) = node else { return StructuralTraversal::SkipChildren; };
        let Some(identity) = self.model.resolve_name(class.name().name()) else { return StructuralTraversal::SkipChildren; };
        let ty = ResolvedType::interface_identity(identity.clone());
        if self.signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::flutter("StatefulWidget")), &self.model) == Truth::Yes {
            self.widgets.push((identity, class.name().span()));
            return StructuralTraversal::SkipChildren;
        }
        let Some(base) = class.extends() else { return StructuralTraversal::SkipChildren; };
        let base = self.model.resolve_type(base);
        if !matches!(&base, ResolvedType::Interface { identity, .. } if self.ctx.identities().is_some_and(|index| index.is_package_declaration(identity, "flutter", "State"))) || !class.with_clause().is_empty() {
            return StructuralTraversal::SkipChildren;
        }
        let Some(ResolvedType::Interface { identity, .. }) = base.type_argument(0) else { return StructuralTraversal::SkipChildren; };
        let mut build = false;
        for member in class.members() {
            match member {
                ClassMember::Method(method) if method.name().name() == "build" && method.body().is_some() => build = true,
                _ => self.necessary = true,
            }
        }
        if build { self.current = Some(identity); }
        StructuralTraversal::Continue
    }
    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        if !self.necessary && let Some(identity) = self.current.take() { self.states.push(identity); }
    }
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Ident(identifier) = node {
            if matches!(identifier.name(), "setState" | "mounted" | "reassemble" | "activate" | "deactivate") {
                self.necessary = true;
            }
            if !matches!(identifier.name(), "widget" | "context") {
                let ty = ResolvedType::interface_identity(DeclarationIdentity::flutter("State"));
                if self.signatures.resolved_member_facts(&ty, identifier.name(), &self.model).is_some_and(|facts| !facts.is_empty()) { self.necessary = true; }
            }
        }
        if matches!(node, Expr::This(_) | Expr::Super(_)) { self.necessary = true; }
        StructuralTraversal::Continue
    }
}
}
