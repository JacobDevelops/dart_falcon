//! Flags primitive operations that return their receiver unchanged.
//!
//! Some calls are no-ops on their receiver's type: `toInt()`, `round()`,
//! `ceil()`, `floor()` and `truncate()` on an `int`, `toDouble()` on a `double`,
//! and `toString()` on a non-nullable `String`. Likewise `toString()` is
//! redundant on an interpolated expression (`'${x.toString()}'`) or the sole
//! argument of `print` (also through a `dart:core` import prefix), which both
//! stringify their operand already, and an empty
//! string literal between two adjacent string literals adds nothing. The rule
//! reports the redundant method name, or the empty literal. A parenthesized
//! method call inside `print` or an interpolation is left alone.
//!
//! Adopted from the official `noop_primitive_operations` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::expressions::is_parenthesized;
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

    /// Whether `callee` names `dart:core`'s `print`, possibly through an
    /// import prefix (`core.print`).
    fn is_core_print(callee: &Expr, state: &SemanticState<'_>) -> bool {
        let (first, name) = match callee {
            Expr::Ident(identifier) => (identifier, QualifiedName::from(identifier.name.as_str())),
            Expr::Field {
                object,
                field,
                is_null_safe: false,
                ..
            } => match object.as_ref() {
                Expr::Ident(prefix) => (prefix, QualifiedName::prefixed(&prefix.name, &field.name)),
                _ => return false,
            },
            _ => return false,
        };
        !state.environment.is_bound(&first.name)
            && state.model.resolve_sdk_member(name)
                == Some(DeclarationIdentity::sdk("dart:core", "print"))
    }

    /// Reports `name` invoked on a receiver of type `receiver` when it
    /// returns the receiver unchanged.
    fn check_invocation(&mut self, receiver: &ResolvedType, name: &Identifier) {
        let ResolvedType::Interface {
            identity, nullable, ..
        } = receiver
        else {
            return;
        };
        let method = name.name.as_str();
        let noop = (identity.is_sdk("dart:core", "String") && method == "toString" && !nullable)
            || (identity.is_sdk("dart:core", "int") && INT_INVARIANTS.contains(&method))
            || (identity.is_sdk("dart:core", "double") && method == "toDouble");
        if noop {
            self.report(name.span);
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::StringLit(literal) => {
                for interpolation in &literal.interpolations {
                    if !is_parenthesized(&interpolation.expr) {
                        self.check_to_string(&interpolation.expr);
                    }
                }
                self.check_adjacent(literal);
            }
            Expr::Call { callee, args, .. } => {
                if Self::is_core_print(callee, state) {
                    if args.positional.len() == 1
                        && args.named.is_empty()
                        && !is_parenthesized(&args.positional[0])
                    {
                        self.check_to_string(&args.positional[0]);
                    }
                } else if let Expr::Field { object, field, .. } = callee.as_ref() {
                    self.check_invocation(&state.infer_invocation(object), field);
                }
            }
            // A cascade section's leading invocation targets the cascade object.
            Expr::Cascade {
                object, sections, ..
            } => {
                let receiver = state.infer_invocation(object);
                for section in sections {
                    if let [CascadeOp::Call(name, _, _), ..] = section.ops.as_slice() {
                        self.check_invocation(&receiver, name);
                    }
                }
            }
            _ => {}
        }
    }
}
