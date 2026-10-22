//! Warns when a widget's build method constructs the same widget again.
//!
//! Resolved Flutter StatelessWidget subclasses and State<T> subclasses are
//! checked in build only. Constructor identity, not spelling, is compared;
//! unrelated factories, createState, and other methods are ignored.
//! Deferred builders and conditional expressions are excluded because recursive
//! data-driven widget trees can terminate normally.

use crate::lint::support::controller_disposal::constructed_identity;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidRecursiveWidgetCalls;
impl Rule for AvoidRecursiveWidgetCalls {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            target: None,
            building: false,
            locals: ctx.local_declarations(program),
            diagnostics: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    target: Option<DeclarationIdentity>,
    building: bool,
    locals: &'a std::collections::HashMap<usize, NodeId>,
    diagnostics: Vec<Diagnostic>,
}
impl<'a> Collector<'a> {
    fn element<E: Element<'a>>(&mut self, node: E) -> StructuralTraversal {
        if matches!(
            node.syntax().kind(),
            NodeKind::IfElement
                | NodeKind::ForElement
                | NodeKind::MapIfElement
                | NodeKind::MapForElement
        ) {
            StructuralTraversal::SkipChildren
        } else {
            StructuralTraversal::Continue
        }
    }
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.target = None;
        let TopLevelDecl::Class(class) = node else { return StructuralTraversal::SkipChildren; };
        let Some(identity) = self.model.resolve_name(class.name().name()) else { return StructuralTraversal::SkipChildren; };
        let ty = ResolvedType::interface_identity(identity.clone());
        if self.signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::flutter("StatelessWidget")), &self.model) == Truth::Yes {
            self.target = Some(identity);
        } else if let Some(instance) = self.signatures.as_instance_of(&ty, &DeclarationIdentity::flutter("State"), &self.model)
            && let Some(ResolvedType::Interface { identity, .. }) = instance.type_argument(0) {
            self.target = Some(identity);
        }
        if self.target.is_some() { StructuralTraversal::Continue } else { StructuralTraversal::SkipChildren }
    }
    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        self.building = matches!(node, ClassMember::Method(method) if !method.is_static() && method.name().name() == "build");
        if self.building { StructuralTraversal::Continue } else { StructuralTraversal::SkipChildren }
    }
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if matches!(node, Stmt::If(_) | Stmt::Switch(_) | Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_)) {
            StructuralTraversal::SkipChildren
        } else { StructuralTraversal::Continue }
    }
    fn visit_collection_element(&mut self, node: CollectionElement<'a>) -> StructuralTraversal {
        self.element(node)
    }
    fn visit_map_element(&mut self, node: MapElement<'a>) -> StructuralTraversal {
        self.element(node)
    }
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr(_) | Expr::Conditional(_)) {
            return StructuralTraversal::SkipChildren;
        }
        if let Expr::Call(call) = node
            && self.locals.contains_key(&call.callee().unparenthesized().span().start)
        {
            return StructuralTraversal::Continue;
        }
        if self.building && let Some(identity) = constructed_identity(node, &self.model, self.signatures)
            && self.target.as_ref() == Some(&identity) {
            self.diagnostics.push(self.ctx.diagnostic("This build constructs its own widget recursively; return a different widget.", node.span()));
        }
        StructuralTraversal::Continue
    }
}
}
