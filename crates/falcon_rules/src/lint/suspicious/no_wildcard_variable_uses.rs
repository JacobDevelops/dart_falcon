//! Flags references to a wildcard variable or parameter (a name made solely of
//! underscores, such as `_` or `__`).
//!
//! An all-underscore name marks a binding as intentionally unused, and under
//! Dart's wildcard-variable semantics such names are non-binding — reading one
//! back does not retrieve the value you expect. Referencing it is therefore
//! either a mistake or a reliance on behavior that is changing. Declaring a
//! wildcard is fine; only uses in expression position are reported. Give the
//! binding a real name if you actually need its value.

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
