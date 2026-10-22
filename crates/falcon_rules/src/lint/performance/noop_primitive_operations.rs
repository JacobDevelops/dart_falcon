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

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::is_parenthesized;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct NoopPrimitiveOperations;

impl Rule for NoopPrimitiveOperations {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

const INT_INVARIANTS: &[&str] = &["toInt", "round", "ceil", "floor", "truncate"];

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic("The expression has no effect and can be removed.", span));
    }

    /// Reports `<target>.toString()` (with no arguments and a non-`super` target).
    fn check_to_string(&mut self, expr: Expr<'_>) {
        if let Expr::Call(call) = expr
            && call.args().positional().is_empty()
            && call.args().named().is_empty()
            && let Expr::Field(field) = call.callee()
            && field.field().name() == "toString"
            && !matches!(field.object(), Expr::Super(_))
        {
            self.report(field.field().span());
        }
    }

    fn check_adjacent(&mut self, literal: StringLitNode<'_>) {
        if !literal.is_adjacent() {
            return;
        }
        let mut segments = literal.segments().skip(1).peekable();
        while let Some((delimiter, span)) = segments.next() {
            if segments.peek().is_some() && delimiter.content.is_empty() {
                self.report(span);
            }
        }
    }

    /// Whether `callee` names `dart:core`'s `print`, possibly through an
    /// import prefix (`core.print`).
    fn is_core_print(callee: Expr<'_>, state: &SemanticState<'_>) -> bool {
        let (first, name) = match callee {
            Expr::Ident(identifier) => (identifier, QualifiedName::from(identifier.name())),
            Expr::Field(field) if !field.is_null_safe() => match field.object() {
                Expr::Ident(prefix) => (
                    prefix,
                    QualifiedName::prefixed(prefix.name(), field.field().name()),
                ),
                _ => return false,
            },
            _ => return false,
        };
        !state.environment.is_bound(first.name())
            && state.model.resolve_sdk_member(name)
                == Some(DeclarationIdentity::sdk("dart:core", "print"))
    }

    /// Reports `name` invoked on a receiver of type `receiver` when it
    /// returns the receiver unchanged.
    fn check_invocation(&mut self, receiver: &ResolvedType, name: Identifier<'_>) {
        let ResolvedTypeView::Interface {
            identity, nullable, ..
        } = receiver.view()
        else {
            return;
        };
        let method = name.name();
        let noop = (identity.is_sdk("dart:core", "String") && method == "toString" && !nullable)
            || (identity.is_sdk("dart:core", "int") && INT_INVARIANTS.contains(&method))
            || (identity.is_sdk("dart:core", "double") && method == "toDouble");
        if noop {
            self.report(name.span());
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::StringLit(literal) => {
                for interpolation in literal.interpolations() {
                    if !is_parenthesized(interpolation.expr()) {
                        self.check_to_string(interpolation.expr());
                    }
                }
                self.check_adjacent(literal);
            }
            Expr::Call(call) => {
                if Self::is_core_print(call.callee(), state) {
                    if call.args().positional().len() == 1
                        && call.args().named().is_empty()
                        && let Some(argument) = call.args().positional().next()
                        && !is_parenthesized(argument)
                    {
                        self.check_to_string(argument);
                    }
                } else if let Expr::Field(field) = call.callee() {
                    self.check_invocation(&state.infer(field.object()), field.field());
                }
            }
            // A cascade section's leading invocation targets the cascade object.
            Expr::Cascade(cascade) => {
                let receiver = state.infer(cascade.object());
                for section in cascade.sections() {
                    if let Some(CascadeOp::Call(call)) = section.ops().next() {
                        self.check_invocation(&receiver, call.name());
                    }
                }
            }
            _ => {}
        }
    }
}
