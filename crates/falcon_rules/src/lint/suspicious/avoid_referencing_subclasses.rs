//! Keeps base classes independent from their transitive subclasses.
//! `ignored-annotations` replaces the default ["freezed", "Freezed", "unfreezed"].
//! Annotation names match the final name segment exactly, including case.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedTypeView, Rule, RulePreparation,
    SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
    TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashMap;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Annotation names that exempt a class from subclass-reference checks.
    ignored_annotations: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ignored_annotations: ["freezed", "Freezed", "unfreezed"]
                .into_iter().map(str::to_owned).collect(),
        }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct AvoidReferencingSubclasses;

impl Rule for AvoidReferencingSubclasses {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let ignored = &crate::lint::options::get::<Options>(ctx, Self::NAME).ignored_annotations;
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            ignored,
            owner: None,
            references: ctx.local_declarations(program),
            parameters: TypeParameterScope::default(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    ignored: &'a [String],
    owner: Option<DeclarationIdentity>,
    references: &'a HashMap<usize, NodeId>,
    parameters: TypeParameterScope,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, identity: DeclarationIdentity, span: Span) {
        let Some(owner) = &self.owner else {
            return;
        };
        if &identity != owner
            && self
                .signatures
                .member_supertype_matches(&identity, &|parent| parent == owner)
                == Truth::Yes
        {
            self.diagnostics.push(self.ctx.diagnostic("Avoid referencing a subclass from its base class.", span));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        let parameters = node.type_params();
        let (name, annotations) = match node {
            TopLevelDecl::Class(class) => (class.name(), class.annotations()),
            TopLevelDecl::MixinClass(class) => (class.name(), class.annotations()),
            _ => return StructuralTraversal::SkipChildren,
        };
        if self.ignored.iter().any(|name| crate::lint::declarations::annotation(annotations.clone(), name)) {
            return StructuralTraversal::SkipChildren;
        }
        let Some(owner) = self.model.resolve_name(name.name()) else { return StructuralTraversal::SkipChildren; };
        self.owner = Some(owner);
        self.parameters.push(parameters, &self.model);
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        if self.owner.take().is_some() { self.parameters.pop(); }
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.parameters.push(node.type_params(), &self.model);
        StructuralTraversal::Continue
    }
    fn exit_method_decl(&mut self, _node: MethodDecl<'a>) { self.parameters.pop(); }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        self.parameters.push(node.type_params(), &self.model);
        StructuralTraversal::Continue
    }
    fn exit_formal_param(&mut self, _node: FormalParam<'a>) { self.parameters.pop(); }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            self.parameters.push(function.type_params(), &self.model);
        }
        let reference = match node {
            Expr::Ident(name) if !self.references.contains_key(&name.span().start)
                && self.parameters.lookup(name.name()).is_none()
                && self.owner.as_ref().is_none_or(|owner| {
                    self.signatures.declared_member_facts(owner, name.name()).is_empty()
                        && self.signatures.inherited_concrete_member(owner, name.name()).flatten().is_none()
                }) => self.model.resolve_name(name.name()).map(|identity| (identity, name.span())),
            Expr::Field(field) => {
                falcon_analyze::expression_segments(node, false)
                    .filter(|parts| parts.first().is_some_and(|_| {
                        let Expr::Ident(head) = field.object().unparenthesized() else { return false; };
                        !self.references.contains_key(&head.span().start)
                    }))
                    .and_then(|parts| QualifiedName::from_segments(&parts).and_then(|name| self.model.resolve_name(name)))
                    .map(|identity| (identity, field.field().span()))
            }
            _ => None,
        };
        if let Some((identity, span)) = reference { self.check(identity, span); }
        StructuralTraversal::Continue
    }
    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) { self.parameters.pop(); }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.parameters.push(function.type_params(), &self.model);
        }
        StructuralTraversal::Continue
    }
    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if matches!(node, Stmt::LocalFunc(_)) { self.parameters.pop(); }
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Function(function) = node {
            self.parameters.push(function.type_params(), &self.model);
        }
        if let DartType::Named(named) = node
            && let ResolvedTypeView::Interface { identity, .. } = self.model.resolve_type_in(node, &self.parameters).view()
            && let Some(name) = named.segments().last()
        { self.check(identity.clone(), name.span()); }
        StructuralTraversal::Continue
    }
    fn exit_dart_type(&mut self, node: DartType<'a>) {
        if matches!(node, DartType::Function(_)) { self.parameters.pop(); }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
