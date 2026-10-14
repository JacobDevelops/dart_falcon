//! Flags primitive operations that return their receiver unchanged.
//!
//! Some calls are no-ops on their receiver's type: `toInt()`, `round()`,
//! `ceil()`, `floor()` and `truncate()` on an `int`, `toDouble()` on a `double`,
//! and `toString()` on a non-nullable `String`. Likewise `toString()` is
//! redundant on an interpolated expression (`'${x.toString()}'`) or the sole
//! argument of `print`, which both stringify their operand already, and an empty
//! string literal between two adjacent string literals adds nothing. The rule
//! reports the redundant method name, or the empty literal.
//!
//! Adopted from the official `noop_primitive_operations` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct NoopPrimitiveOperations;

impl Rule for NoopPrimitiveOperations {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

const INT_INVARIANTS: &[&str] = &["toInt", "round", "ceil", "floor", "truncate"];

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(
            "noop-primitive-operations",
            "The expression has no effect and can be removed.",
            span,
        ));
    }

    /// Reports `<target>.toString()` (with no arguments and a non-`super` target).
    fn check_to_string(&mut self, expr: &Expr) {
        if let Expr::Call { callee, args, .. } = expr
            && args.positional.is_empty()
            && args.named.is_empty()
            && let Expr::Field { object, field, .. } = callee.as_ref()
            && field.name == "toString"
            && !matches!(object.as_ref(), Expr::Super { .. })
        {
            self.report(field.span);
        }
    }

    fn check_adjacent(&mut self, literal: &StringLitNode) {
        if !literal.is_adjacent || literal.segment_spans.len() < 3 {
            return;
        }
        let segments: Vec<_> = literal.segments().collect();
        let last = segments.len().saturating_sub(1);
        for (index, (delimiter, span)) in segments.into_iter().enumerate() {
            if index > 0 && index < last && delimiter.content.is_empty() {
                self.report(span);
            }
        }
    }

    fn is_core_print(callee: &Expr, state: &SemanticState<'_>) -> bool {
        let Expr::Ident(identifier) = callee else {
            return false;
        };
        !state.environment.is_bound(&identifier.name)
            && state.model.resolve_sdk_member(identifier.name.as_str())
                == Some(DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "print",
                })
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::StringLit(literal) => {
                for interpolation in &literal.interpolations {
                    self.check_to_string(&interpolation.expr);
                }
                self.check_adjacent(literal);
            }
            Expr::Call { callee, args, .. } => {
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    let receiver = state.infer(object);
                    let ResolvedType::Interface {
                        identity, nullable, ..
                    } = &receiver
                    else {
                        return;
                    };
                    let name = field.name.as_str();
                    let noop = (identity.is_sdk("dart:core", "String")
                        && name == "toString"
                        && !nullable)
                        || (identity.is_sdk("dart:core", "int") && INT_INVARIANTS.contains(&name))
                        || (identity.is_sdk("dart:core", "double") && name == "toDouble");
                    if noop {
                        self.report(field.span);
                    }
                } else if Self::is_core_print(callee, state)
                    && args.positional.len() == 1
                    && args.named.is_empty()
                {
                    self.check_to_string(&args.positional[0]);
                }
            }
            _ => {}
        }
    }
}
