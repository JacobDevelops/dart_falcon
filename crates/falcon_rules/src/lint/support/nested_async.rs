use falcon_analyze::{
    AnalyzeContext, RulePreparation, SemanticModel, StructuralRuleVisitor, StructuralTraversal,
    TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
pub(crate) enum Nesting {
    Futures,
    Mixed,
}

pub(crate) fn prepare_nested<'a>(
    ctx: &'a AnalyzeContext<'a>,
    nesting: Nesting,
) -> RulePreparation<'a> {
    let Some(identities) = ctx.identities() else {
        return RulePreparation::Diagnostics(Vec::new());
    };
    RulePreparation::Structural(Box::new(Collector {
        ctx,
        nesting,
        model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
        parameters: TypeParameterScope::default(),
        diagnostics: Vec::new(),
    }))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    nesting: Nesting,
    model: SemanticModel<'a>,
    parameters: TypeParameterScope,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn enter(&mut self, parameters: AstChildren<'a, TypeParam<'a>>) {
        if !parameters.is_empty() {
            self.parameters.push(parameters, &self.model);
        }
    }

    fn exit(&mut self, parameters: AstChildren<'a, TypeParam<'a>>) {
        if !parameters.is_empty() {
            self.parameters.pop();
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.enter(node.type_params());
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        self.exit(node.type_params());
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.enter(node.type_params());
        StructuralTraversal::Continue
    }

    fn exit_method_decl(&mut self, node: MethodDecl<'a>) { self.exit(node.type_params()); }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node { self.enter(function.type_params()); }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if let Stmt::LocalFunc(function) = node { self.exit(function.type_params()); }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node { self.enter(function.type_params()); }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if let Expr::FuncExpr(function) = node { self.exit(function.type_params()); }
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        self.enter(node.type_params());
        StructuralTraversal::Continue
    }

    fn exit_formal_param(&mut self, node: FormalParam<'a>) { self.exit(node.type_params()); }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Function(function) = node { self.enter(function.type_params()); }
        let ty = self.model.resolve_type_in(node, &self.parameters);
        if let Some(inner) = ty.type_argument(0) {
            let outer_future = ty.interface("dart:async", "Future")
                || ty.interface("dart:async", "FutureOr");
            let inner_future = inner.interface("dart:async", "Future")
                || inner.interface("dart:async", "FutureOr");
            let (report, message) = match self.nesting {
                Nesting::Futures => (
                    outer_future && inner_future,
                    "Remove the nested future wrapper; return a single `Future` or `FutureOr` instead.",
                ),
                Nesting::Mixed => (
                    (ty.interface("dart:async", "Stream") && inner_future)
                        || (outer_future && inner.interface("dart:async", "Stream")),
                    "Remove the nested stream/future wrapper; expose a single asynchronous sequence.",
                ),
            };
            if report { self.diagnostics.push(self.ctx.diagnostic(message, node.span())); }
        }
        StructuralTraversal::Continue
    }

    fn exit_dart_type(&mut self, node: DartType<'a>) {
        if let DartType::Function(function) = node { self.exit(function.type_params()); }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
