//! Flags a redundant `.new` on constructor declarations and invocations.
//!
//! `X.new(...)` names the default (unnamed) constructor explicitly, which is exactly
//! equivalent to the shorter `X(...)`, so the `.new` is noise. The rule catches both
//! the plain call form (`X.new(...)`) and the `new`/`const` form (`const X.new(...)`).
//! Declarations such as `class X { X.new(); }` are also reported.
//! A bare `X.new` constructor tear-off that is not invoked is deliberately left alone:
//! there the `.new` is required to reference the constructor as a value.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryConstructorName;

impl Rule for UnnecessaryConstructorName {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    // `class A { A.new(); }` — declaring the default constructor as `.new`.
    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        if let Some(name) = node.constructor_name
            && name.name() == "new"
        {
            self.diags.push(self.ctx.diagnostic(
                "Unnecessary `.new`; the default constructor can be invoked without it.",
                name.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            // `X.new(...)` — the invocation form parses as a call whose callee is a
            // `.new` field access. The bare tear-off `X.new` is a `Field` not wrapped
            // in a `Call`, so it is not reached here.
            Expr::Call(call) => {
                if let Expr::Field(access) = call.callee().unparenthesized()
                    && access.field().name() == "new"
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Unnecessary `.new`; the default constructor can be invoked without it.",
                        access.field().span(),
                    ));
                }
            }
            // `new X.new(...)` / `const X.new(...)` — redundant named constructor.
            Expr::New(expression) => {
                if let Some(name) = expression.constructor_name()
                    && name.name() == "new"
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Unnecessary `.new`; the default constructor can be invoked without it.",
                        name.span(),
                    ));
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}
