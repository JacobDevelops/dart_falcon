//! Require configured error parameter names in catches and SDK error callbacks.
//!
//! Set `allowed_names` to a nonempty string array to enable the rule. Optional
//! `ignored_names` exempts names. Stack-trace parameters are not checked. Callback
//! checks require a resolved dart:async Future or Stream receiver; unrelated APIs,
//! tear-offs and dynamic receivers are excluded.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

const RULE: &str = "prefer-correct-error-name";

#[derive(Default)]
pub struct PreferCorrectErrorName;

impl Rule for PreferCorrectErrorName {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        let options = crate::lint::options::for_rule(ctx, RULE);
        let allowed: Vec<_> = crate::lint::options::strings(options, "allowed_names")
            .into_iter()
            .flatten()
            .filter(|name| !name.is_empty())
            .collect();
        if allowed.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let ignored = crate::lint::options::strings(options, "ignored_names")
            .into_iter()
            .flatten()
            .collect();
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            allowed,
            ignored,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    allowed: Vec<&'a str>,
    ignored: Vec<&'a str>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>) {
        if name.name().bytes().all(|byte| byte == b'_') {
            return;
        }
        if !self.allowed.contains(&name.name()) && !self.ignored.contains(&name.name()) {
            self.diags.push(self.ctx.diagnostic(RULE,
                format!("This error parameter does not match the configured names; rename it to one of: {}.", self.allowed.join(", ")),
                name.span()));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, _: &SemanticState<'a>) {
        if let Stmt::TryCatch(statement) = node {
            for catch in statement.catches() {
                if let Some(name) = catch.exception_var() {
                    self.check(name);
                }
            }
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return;
        };
        let receiver = state.infer(field.object());
        let name = field.field().name();
        let future = receiver.interface("dart:async", "Future");
        let stream = receiver.interface("dart:async", "Stream");
        let args = call.args();
        let callback = match name {
            "catchError" if future => args.positional().next(),
            "handleError" if stream => args.positional().next(),
            "then" if future => args
                .named()
                .find(|arg| arg.name().name() == "onError")
                .map(|arg| arg.value()),
            "listen" if stream => args
                .named()
                .find(|arg| arg.name().name() == "onError")
                .map(|arg| arg.value()),
            _ => None,
        };
        if let Some(Expr::FuncExpr(function)) = callback.map(Expr::unparenthesized)
            && let Some(parameter) = function.params().positional().next()
        {
            self.check(parameter.name());
        }
    }
}
