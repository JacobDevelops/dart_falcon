//! Lexical execution contexts for Flutter hooks.

use crate::lint::flutter_hooks::{
    hook_name, hook_widget_context, is_hook_builder_context, non_hook_widget, non_widget_context,
};
use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{AnalyzeContext, FunctionBodies};
use falcon_syntax::cst::visit::{Visitor, walk_local_func_decl};
use falcon_syntax::cst::*;
/// An executable's permission never extends into another function body.
pub(crate) struct HookContexts<'a> {
    bodies: &'a FunctionBodies<'a>,
    methods: Vec<(Span, &'a str, bool)>,
    hooks: Vec<Span>,
    builders: Vec<Span>,
}

impl<'a> HookContexts<'a> {
    pub(crate) fn new(program: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> Self {
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
            bodies: ctx.function_bodies(program),
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
