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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct NoSelfAssignments;

impl Rule for NoSelfAssignments {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut finder = Finder {
            ctx,
            diagnostics: Vec::new(),
        };
        finder.visit_program(program);
        finder.diagnostics
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
            && !self.parenthesized(target, value)
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

impl Finder<'_, '_> {
    /// Parentheses change the analyzer's RHS node kind; comments do not.
    fn parenthesized(&self, target: &Expr, value: &Expr) -> bool {
        let Some(between) = self.ctx.source.get(target.span().end..value.span().start) else {
            return true;
        };
        Lexer::new(between)
            .tokenize()
            .into_iter()
            .filter(|token| !token.is_trivia() && token.kind != TokenKind::Eof)
            .any(|token| token.text(between) != "=")
    }
}

/// The analyzer `Identifier` name: `x` for a simple identifier and `a.b` for a
/// prefixed identifier whose prefix is itself a simple identifier.
fn identifier_name(expr: &Expr) -> Option<(&str, Option<&str>)> {
    match expr {
        Expr::Ident(identifier) => Some((identifier.name.as_str(), None)),
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } => match object.as_ref() {
            Expr::Ident(prefix) => Some((prefix.name.as_str(), Some(field.name.as_str()))),
            _ => None,
        },
        _ => None,
    }
}
