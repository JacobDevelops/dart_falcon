//! Flags references to a wildcard variable or parameter (a name made solely of
//! underscores, such as `_` or `__`).
//!
//! An all-underscore name marks a binding as intentionally unused, and under
//! Dart's wildcard-variable semantics such names are non-binding — reading one
//! back does not retrieve the value you expect. Referencing it is therefore
//! either a mistake or a reliance on behavior that is changing. Declaring a
//! wildcard is fine; only uses in expression position are reported. Give the
//! binding a real name if you actually need its value.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct NoWildcardVariableUses;

impl Rule for NoWildcardVariableUses {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut visitor = WildcardUseVisitor {
            diags: Vec::new(),
            ctx,
        };
        visitor.visit_program(program);
        RulePreparation::Diagnostics(visitor.diags)
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
            self.diags.push(
                self.ctx
                    .diagnostic("no-wildcard-variable-uses", MESSAGE, id.span),
            );
        }
        walk_expr(self, node);
    }
}
