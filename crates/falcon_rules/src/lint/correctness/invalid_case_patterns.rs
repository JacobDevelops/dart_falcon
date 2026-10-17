//! Flags pre-3.0 switch case expressions that change meaning or break under Dart 3 patterns.
//!
//! Dart 3 reinterprets `case` operands as patterns. Code still on a pre-3.0
//! language version can prepare for the migration by avoiding case expressions
//! that are invalid or mean something different as patterns: non-`const` list,
//! map, or set literals; implicitly-const constructor calls; `identical(...)`
//! calls; prefix operators on anything but an integer literal; binary,
//! conditional and `is` expressions; `.length` accesses whose receiver, after
//! unwrapping parentheses, is not an identifier; and the identifier `_` (a
//! wildcard in Dart 3). The receiver test applies to ordinary and null-aware
//! accesses alike. The lint applies only to libraries whose language version
//! predates 3.0, where falcon parses case operands as expressions.
//!
//! Adopted from the official `invalid_case_patterns` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, QualifiedName, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Feature;
use falcon_syntax::cst::*;

use crate::lint::flutter::scoped_constructor_invocation;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct InvalidCasePatterns;

impl Rule for InvalidCasePatterns {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if ctx.language_version().supports(Feature::Patterns) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Switch(switch) = node {
            for case in switch.cases() {
                for kind in case.cases() {
                    match kind {
                        SwitchCaseKind::Pattern(_) | SwitchCaseKind::Default(_) => {}
                        SwitchCaseKind::Expr(label) => {
                            let expression = label.expr();
                            if invalid_expression(expression, state) {
                                self.diags.push(self.ctx.diagnostic(
                                    "invalid-case-patterns",
                                    "This expression is not valid in a 'case' clause in Dart 3.0.",
                                    expression.span(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The analyzer's verdict on a Dart 2 case expression.
fn invalid_expression<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    match expression.unparenthesized() {
        Expr::List(node) => !node.is_const(),
        Expr::Set(node) => !node.is_const(),
        Expr::Map(node) => !node.is_const(),
        Expr::Unary(node) => !matches!(node.operand().unparenthesized(), Expr::IntLit(_)),
        Expr::Binary(_) | Expr::Conditional(_) | Expr::Is(_) => true,
        Expr::Call(node) => {
            if scoped_constructor_invocation(expression, state).is_some() {
                return true;
            }
            let (first, name) = match node.callee().unparenthesized() {
                Expr::Ident(name) => (name, QualifiedName::from(name.name())),
                Expr::Field(field) => {
                    let Expr::Ident(prefix) = field.object().unparenthesized() else {
                        return false;
                    };
                    (
                        prefix,
                        QualifiedName::prefixed(prefix.name(), field.field().name()),
                    )
                }
                _ => return false,
            };
            !state.environment.is_bound(first.name())
                && state.model.resolve_sdk_member(name)
                    == Some(DeclarationIdentity::sdk("dart:core", "identical"))
        }
        Expr::Field(node) => {
            node.field().name() == "length"
                && !matches!(node.object().unparenthesized(), Expr::Ident(_))
        }
        Expr::Ident(name) => name.name() == "_",
        _ => false,
    }
}
