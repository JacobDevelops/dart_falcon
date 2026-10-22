//! Flags confirmed Flutter hooks outside hook builds, custom hooks, and HookBuilder/HookConsumer builders.
//!
//! Only resolved flutter_hooks functions and useX/_useX functions that actually
//! compose them count as hooks. Nested callbacks do not inherit the enclosing
//! build or custom-hook permission. Confirmed non-hook widget builds are left
//! to avoid-misused-hooks; unresolved calls, unresolved build ancestry, and shadowed names are ignored.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter_hooks::{
    HookCalls, hook_name, hook_widget_context, is_hook_builder_context, non_hook_widget,
    non_widget_context,
};
use crate::lint::function_bodies::FunctionBodies;
use falcon_syntax::cst::visit::{Visitor, walk_local_func_decl};

#[derive(Default)]
pub struct AvoidHooksOutsideBuild;

impl Rule for AvoidHooksOutsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            contexts: HookContexts::new(program),
            calls: HookCalls::new(program),
            candidates: Vec::new(),
        }))
    }
}

/// An executable's permission never extends into another function body.
pub(crate) struct HookContexts<'a> {
    bodies: FunctionBodies<'a>,
    methods: Vec<(Span, &'a str, bool)>,
    hooks: Vec<Span>,
    builders: Vec<Span>,
}

impl<'a> HookContexts<'a> {
    pub(crate) fn new(program: Program<'a>) -> Self {
        let mut methods = Vec::new();
        let mut hooks = Vec::new();
        for declaration in program.declarations() {
            match declaration {
                TopLevelDecl::Function(function) if hook_name(function.name().name()) => {
                    if let Some(body) = function.body() {
                        hooks.push(body.span());
                    }
                }
                declaration if crate::lint::declarations::members_of(declaration).is_some() => {
                    for member in crate::lint::declarations::members_of(declaration)
                        .into_iter()
                        .flatten()
                    {
                        if let ClassMember::Method(method) = member
                            && let Some(body) = method.body()
                        {
                            methods.push((body.span(), method.name().name(), method.is_static()));
                            if hook_name(method.name().name()) {
                                hooks.push(body.span());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        LocalHooks(&mut hooks).visit_program(program);
        Self {
            bodies: FunctionBodies::new(program),
            methods,
            hooks,
            builders: Vec::new(),
        }
    }

    pub(crate) fn observe(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        if !is_hook_builder_context(expr, state) {
            return;
        }
        let Some((_, _, args)) = crate::lint::flutter::scoped_constructor_invocation(expr, state)
        else {
            return;
        };
        if let Some(argument) = args
            .named()
            .find(|argument| argument.name().name() == "builder")
            && let Expr::FuncExpr(function) = argument.value().unparenthesized()
            && function.async_marker() == AsyncMarker::Sync
        {
            self.builders.push(function.body().span());
        }
    }

    pub(crate) fn non_hook_build(&self, expr: Expr<'_>, state: &SemanticState<'_>) -> bool {
        self.build_body(expr) && non_hook_widget(state)
    }

    fn build_body(&self, expr: Expr<'_>) -> bool {
        let Some(body) = self.bodies.enclosing(&expr.span()) else {
            return false;
        };
        self.methods
            .iter()
            .any(|&(span, name, is_static)| span == body.span && name == "build" && !is_static)
    }

    pub(crate) fn allowed(&self, expr: Expr<'_>, state: &SemanticState<'_>) -> bool {
        let Some(body) = self.bodies.enclosing(&expr.span()) else {
            return false;
        };
        self.hooks.contains(&body.span)
            || self.builders.contains(&body.span)
            || (self.build_body(expr) && !non_widget_context(state))
    }

    pub(crate) fn valid_context(&self, expr: Expr<'_>, state: &SemanticState<'_>) -> bool {
        let Some(body) = self.bodies.enclosing(&expr.span()) else {
            return false;
        };
        if self.hooks.contains(&body.span) || self.builders.contains(&body.span) {
            return true;
        }
        self.build_body(expr) && hook_widget_context(state)
    }
}

struct LocalHooks<'b>(&'b mut Vec<Span>);

impl<'a> Visitor<'a> for LocalHooks<'_> {
    fn visit_local_func_decl(&mut self, node: LocalFuncDecl<'a>) {
        if hook_name(node.name().name()) {
            self.0.push(node.body().span());
        }
        walk_local_func_decl(self, node);
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    contexts: HookContexts<'a>,
    calls: HookCalls<'a>,
    candidates: Vec<Expr<'a>>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        self.calls.observe(expr, state);
        self.contexts.observe(expr, state);
        if matches!(expr, Expr::Call(_)) && !self.contexts.allowed(expr, state) {
            self.candidates.push(expr);
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.calls.finish();
        self.candidates.iter().filter(|&&expr| self.calls.contains(expr)).map(|expr| {
            self.ctx.diagnostic("Call hooks directly in a hook widget `build`, custom hook, or `HookBuilder`/`HookConsumer` builder.", expr.span())
        }).collect()
    }
}
