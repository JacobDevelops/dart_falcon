//! Flags a local variable bound to a function literal that should be a local
//! function declaration.
//!
//! Binding a closure to a variable (`final f = () {...};`) hides that it is a
//! named function; a proper declaration (`void f() {...}`) reads more clearly,
//! supports recursion and generic type parameters, and gives a return type a
//! natural home. The rule only reports `final` or `const` local bindings, which
//! provably can never be reassigned. A plain `var f = () {...}` is left alone,
//! since proving it is never reassigned requires dataflow this syntax-only pass
//! does not perform.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferFunctionDeclarationsOverVariables;

impl Rule for PreferFunctionDeclarationsOverVariables {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'c> {
    ctx: &'a AnalyzeContext<'c>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn visit_local_var(&mut self, local: LocalVarDecl<'a>) -> StructuralTraversal {
        if local.keyword() != VarKeyword::Var {
            for decl in local.declarators() {
                if let Some(Expr::FuncExpr(_)) =
                    decl.initializer().map(Expr::unparenthesized)
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Use a function declaration to bind a function to a name.".to_string(),
                        decl.name().span(),
                    ));
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
