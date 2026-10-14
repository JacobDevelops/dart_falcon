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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};

#[derive(Default)]
pub struct PreferFunctionDeclarationsOverVariables;

impl Rule for PreferFunctionDeclarationsOverVariables {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'c> {
    ctx: &'a AnalyzeContext<'c>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_local_var(&mut self, local: &LocalVarDecl) {
        if local.keyword != falcon_syntax::ast::VarKeyword::Var {
            for decl in &local.declarators {
                if let Some(Expr::FuncExpr { .. }) = &decl.initializer {
                    self.diags.push(self.ctx.diagnostic(
                        "prefer-function-declarations-over-variables",
                        "Use a function declaration to bind a function to a name.".to_string(),
                        decl.name.span,
                    ));
                }
            }
        }
        visitor::walk_local_var(self, local);
    }
}
