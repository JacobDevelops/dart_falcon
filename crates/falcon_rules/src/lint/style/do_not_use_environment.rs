//! Disallow reading compilation environment declarations, including type aliases.
//!
//! Reports invocations of `bool.fromEnvironment`, `int.fromEnvironment`,
//! `String.fromEnvironment`, and `bool.hasEnvironment`. Constructor identity
//! is resolved through type aliases and import prefixes; dot-shorthand
//! invocations are checked using their expected type. User-defined constructors
//! with the same names and constructor tear-offs are not reported.
//!
//! Adopted from the official `do_not_use_environment` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, expression_segments};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct DoNotUseEnvironment;


impl Rule for DoNotUseEnvironment {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !ctx.source().contains("Environment") {
            return RulePreparation::Diagnostics(Vec::new());
        }
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

fn reads_environment(ty: &ResolvedType, constructor: &str) -> bool {
    (constructor == "fromEnvironment"
        && ["bool", "int", "String"]
            .into_iter()
            .any(|name| ty.interface("dart:core", name)))
        || (constructor == "hasEnvironment" && ty.interface("dart:core", "bool"))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let (ty, constructor, span) = match node {
            Expr::Call(call) => {
                let callee = call.callee();
                if let Expr::DotShorthand(shorthand) = callee.unparenthesized() {
                    let name = shorthand.name();
                    let Some(ty) = state.expected() else {
                        return;
                    };
                    if reads_environment(ty, name.name()) {
                        self.diags
                            .push(self.ctx.diagnostic("Avoid using environment declarations.", name.span()));
                    }
                    return;
                }
                let Some(mut segments) = expression_segments(callee, false) else {
                    return;
                };
                let Some(head) = segments.first() else {
                    return;
                };
                if state.environment.is_bound(head) || state.type_parameters.contains(head) {
                    return;
                }
                let Some(constructor) = segments.pop() else {
                    return;
                };
                let Some(ty) = state.model.resolve_type_name(&segments) else {
                    return;
                };
                (ty, constructor, callee.span())
            }
            Expr::New(construction) => {
                let DartType::Named(named) = construction.dart_type() else {
                    return;
                };
                if !named.type_args().is_empty() {
                    return;
                }
                let constructor_name = construction.constructor_name();
                let mut segments: Vec<&str> =
                    named.segments().map(|segment| segment.name()).collect();
                let Some(constructor) = constructor_name
                    .map(|name| name.name())
                    .or_else(|| segments.pop())
                else {
                    return;
                };
                let Some(ty) = state.model.resolve_type_name(&segments) else {
                    return;
                };
                let end = constructor_name.map_or(named.span().end, |name| name.span().end);
                (
                    ty,
                    constructor,
                    Span {
                        start: named.span().start,
                        end,
                    },
                )
            }
            _ => return,
        };
        if reads_environment(&ty, constructor) {
            self.diags.push(self.ctx.diagnostic("Avoid using environment declarations.", span));
        }
    }
}
