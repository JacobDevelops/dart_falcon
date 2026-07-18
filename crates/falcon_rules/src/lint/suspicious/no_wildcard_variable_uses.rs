//! Flags uses of a wildcard variable or parameter (a name made solely of
//! underscores, e.g. `_`, `__`). Ported from package:lints
//! `no_wildcard_variable_uses`. Declaring a wildcard is fine; referencing it is
//! not, so only identifiers in expression position are reported.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct NoWildcardVariableUses;

impl Rule for NoWildcardVariableUses {
    fn name(&self) -> &'static str {
        "no-wildcard-variable-uses"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut visitor = WildcardUseVisitor {
            diags: Vec::new(),
            ctx,
        };
        visitor.visit_program(program);
        visitor.diags
    }
}

const MESSAGE: &str = "Don't use a wildcard parameter or variable.";

fn is_wildcard(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b == b'_')
}

struct WildcardUseVisitor<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for WildcardUseVisitor<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Ident(id) = node
            && is_wildcard(&id.name)
        {
            self.diags.push(Diagnostic::new(
                "no-wildcard-variable-uses",
                Severity::Warning,
                MESSAGE,
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: id.span.start,
                    end: id.span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}
