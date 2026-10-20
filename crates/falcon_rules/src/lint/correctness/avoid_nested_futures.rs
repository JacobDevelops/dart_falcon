//! Avoid `Future<Future<T>>`, `FutureOr<Future<T>>`, and other directly
//! nested future wrappers. Return `Future<T>` (or `FutureOr<T>`) instead.
//!
//! Checks explicit type occurrences, including expanded typedefs, using SDK
//! declaration identities and lexical type parameters. Collections of futures
//! are not nested wrappers; inferred-only types are deliberately not reported.
//!
//! Reference: https://dcm.dev/docs/rules/common/avoid-nested-futures/

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidNestedFutures;

impl Rule for AvoidNestedFutures {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_nested(ctx, Nesting::Futures)
    }
}

pub(super) enum Nesting {
    Futures,
    Mixed,
}

pub(super) fn prepare_nested<'a>(
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

fn declaration_parameters<'a>(node: TopLevelDecl<'a>) -> Option<AstChildren<'a, TypeParam<'a>>> {
    Some(match node {
        TopLevelDecl::Class(node) => node.type_params(),
        TopLevelDecl::ClassTypeAlias(node) => node.type_params(),
        TopLevelDecl::Mixin(node) => node.type_params(),
        TopLevelDecl::MixinClass(node) => node.type_params(),
        TopLevelDecl::Enum(node) => node.type_params(),
        TopLevelDecl::Extension(node) => node.type_params(),
        TopLevelDecl::ExtensionType(node) => node.type_params(),
        TopLevelDecl::Function(node) => node.type_params(),
        TopLevelDecl::TypeAlias(node) => node.type_params(),
        TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => return None,
    })
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let Some(parameters) = declaration_parameters(node) { self.enter(parameters); }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let Some(parameters) = declaration_parameters(node) { self.exit(parameters); }
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
            let (report, rule, message) = match self.nesting {
                Nesting::Futures => (
                    outer_future && inner_future,
                    "avoid-nested-futures",
                    "Remove the nested future wrapper; return a single Future or FutureOr instead.",
                ),
                Nesting::Mixed => (
                    (ty.interface("dart:async", "Stream") && inner_future)
                        || (outer_future && inner.interface("dart:async", "Stream")),
                    "avoid-nested-streams-and-futures",
                    "Remove the nested stream/future wrapper; expose a single asynchronous sequence.",
                ),
            };
            if report { self.diagnostics.push(self.ctx.diagnostic(rule, message, node.span())); }
        }
        StructuralTraversal::Continue
    }

    fn exit_dart_type(&mut self, node: DartType<'a>) {
        if let DartType::Function(function) = node { self.exit(function.type_params()); }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
