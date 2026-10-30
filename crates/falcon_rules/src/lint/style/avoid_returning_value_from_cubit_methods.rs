//! Warns when a public Cubit instance method returns a value.
//!
//! Cubit commands should expose their results through observable state rather than
//! return values. Methods returning `void`, `Never`, `Future<void>`, or
//! `FutureOr<void>` are allowed, as are private methods, static utilities, getters,
//! and overrides of inherited API methods.

use crate::lint::bloc::BlocTypes;
use falcon_analyze::{
    AnalyzeContext, FunctionBodies, MemberKind, ResolvedType, ResolvedTypeView, Rule,
    RulePreparation, SemanticModel, SemanticRuleVisitor, SemanticState,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidReturningValueFromCubitMethods;

impl Rule for AvoidReturningValueFromCubitMethods {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let types = BlocTypes::new(program);
        let mut diagnostics = Vec::new();
        let mut inferred = Vec::new();
        for declaration in program.declarations() {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            if !types.is_cubit_class(class, &model, ctx.signatures()) {
                continue;
            }
            let owner = model.resolve_name(class.name().name());
            for member in class.members() {
                let ClassMember::Method(method) = member else {
                    continue;
                };
                if method.is_static()
                    || method.name().name().starts_with('_')
                    || crate::lint::support::annotations::has_override(method.annotations(), model)
                    || owner.as_ref().is_some_and(|owner| {
                        ctx.signatures().is_some_and(|signatures| {
                            signatures.has_combined_inherited_member(
                                owner,
                                method.name().name(),
                                MemberKind::Method,
                                &model,
                            )
                        })
                    })
                {
                    continue;
                }
                if let Some(return_type) = method.return_type() {
                    if !is_void_result(&model.resolve_type(return_type)) {
                        diagnostics.push(ctx.diagnostic(MESSAGE, method.name().span()));
                    }
                } else if matches!(
                    method.async_marker(),
                    AsyncMarker::SyncStar | AsyncMarker::AsyncStar
                ) {
                    diagnostics.push(ctx.diagnostic(MESSAGE, method.name().span()));
                } else if let Some(body) = method.body() {
                    inferred.push((body.span(), method.name().span()));
                }
            }
        }
        if inferred.is_empty() {
            return RulePreparation::Diagnostics(diagnostics);
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            bodies: ctx.function_bodies(program),
            inferred,
            diagnostics,
        }))
    }
}

const MESSAGE: &str =
    "Avoid returning values from `Cubit` methods; expose results through state instead.";

fn is_void_result(ty: &ResolvedType) -> bool {
    matches!(ty.view(), ResolvedTypeView::Void | ResolvedTypeView::Never)
        || (ty.interface("dart:async", "Future") || ty.interface("dart:async", "FutureOr"))
            && matches!(ty.arguments(), [argument] if matches!(argument.view(), ResolvedTypeView::Void))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    bodies: &'a FunctionBodies<'a>,
    inferred: Vec<(Span, Span)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn returned(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Some(body) = self.bodies.enclosing(&expression.span()) else {
            return;
        };
        let Some(index) = self
            .inferred
            .iter()
            .position(|(span, _)| *span == body.span)
        else {
            return;
        };
        let ty = state.infer(expression);
        if !is_void_result(&ty) && !matches!(ty.view(), ResolvedTypeView::Unknown) {
            let (_, name) = self.inferred.swap_remove(index);
            self.diagnostics.push(self.ctx.diagnostic(MESSAGE, name));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Return(statement) = statement
            && let Some(value) = statement.value()
        {
            self.returned(value, state);
        }
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if self
            .bodies
            .enclosing(&expression.span())
            .is_some_and(|body| body.arrow_expression == Some(expression.span()))
        {
            self.returned(expression, state);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
