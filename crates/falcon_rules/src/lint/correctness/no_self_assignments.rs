//! Flags assignments of a variable to itself, such as `x = x;`.
//!
//! A self-assignment has no effect and usually hides a typo — most often a
//! constructor or setter that meant to write `this.x = x;`. Only plain `=`
//! assignments whose two sides are the same simple identifier (`x = x`) or the
//! same prefixed identifier (`a.b = a.b`) are reported; `this.x = x` and
//! compound assignments are left alone.
//!
//! ```dart
//! class C {
//!   int x;
//!   C(this.x);
//!   void set(int x) {
//!     x = x; // flagged: assigns the parameter to itself
//!     this.x = x; // fine
//!   }
//! }
//! ```
//!
//! Adopted from the official `no_self_assignments` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::expressions::identifier_name;

#[derive(Default)]
pub struct NoSelfAssignments;

impl Rule for NoSelfAssignments {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut finder = Finder {
            ctx,
            diagnostics: Vec::new(),
        };
        finder.visit_program(program);
        RulePreparation::Diagnostics(finder.diagnostics)
    }
}

struct Finder<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for Finder<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Assign {
            target,
            op: AssignOp::Eq,
            value,
            span,
        } = node
            && let (Some(left), Some(right)) = (identifier_name(target), identifier_name(value))
            && left == right
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "no-self-assignments",
                "The variable or property is being assigned to itself.",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}
