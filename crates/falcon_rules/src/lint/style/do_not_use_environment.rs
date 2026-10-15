//! Disallow reading compilation environment declarations, including type aliases.
//!
//! Reports invocations of `bool.fromEnvironment`, `int.fromEnvironment`,
//! `String.fromEnvironment`, and `bool.hasEnvironment`. Constructor identity
//! is resolved through type aliases and import prefixes; dot-shorthand
//! invocations are checked using their expected type. User-defined constructors
//! with the same names and constructor tear-offs are not reported.
//!
//! Adopted from the official `do_not_use_environment` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, expression_segments};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{self, SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct DoNotUseEnvironment;

const NAME: &str = "do-not-use-environment";
const MESSAGE: &str = "Invalid use of an environment declaration.";

impl Rule for DoNotUseEnvironment {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if !ctx.source.contains("Environment") {
            return Vec::new();
        }
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        semantic_scope::visit_program(&mut collector, program, state);
        collector.diags
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

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let (ty, constructor, span) = match node {
            Expr::Call { callee, .. } => {
                if let Expr::DotShorthand { name, .. } = callee.unparenthesized() {
                    let Some(ty) = state.expected() else {
                        return;
                    };
                    if reads_environment(ty, &name.name) {
                        self.diags
                            .push(self.ctx.diagnostic(NAME, MESSAGE, name.span));
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
                (ty, constructor, *callee.span())
            }
            Expr::New {
                dart_type: DartType::Named(named),
                constructor_name,
                ..
            } if named.type_args.is_empty() => {
                let mut segments: Vec<&str> = named
                    .segments
                    .iter()
                    .map(|segment| segment.name.as_str())
                    .collect();
                let constructor = match constructor_name {
                    Some(name) => name.name.as_str(),
                    None => {
                        let Some(name) = segments.pop() else {
                            return;
                        };
                        name
                    }
                };
                let Some(ty) = state.model.resolve_type_name(&segments) else {
                    return;
                };
                let end = constructor_name
                    .as_ref()
                    .map_or(named.span.end, |name| name.span.end);
                (
                    ty,
                    constructor,
                    Span {
                        start: named.span.start,
                        end,
                    },
                )
            }
            _ => return,
        };
        if reads_environment(&ty, constructor) {
            self.diags.push(self.ctx.diagnostic(NAME, MESSAGE, span));
        }
    }
}
