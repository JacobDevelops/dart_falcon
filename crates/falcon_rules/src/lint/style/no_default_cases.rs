//! Flags `default:` in switch statements over enums and enum-like classes.
//!
//! A `default` case hides newly added enum values: the switch keeps compiling
//! and silently routes them to the fallback. Listing every value explicitly
//! lets the compiler (or `exhaustive-cases`) point at the switch when the type
//! grows. Only switch statements whose subject is provably an enum or an
//! enum-like class (private non-factory constructors, at least two static
//! constants of its own type, no subclasses in its library) are checked.
//! The diagnostic covers the default member including its statement body.
//!
//! ```dart
//! switch (color) {
//!   case Color.red: paint();
//!   default: skip(); // flagged
//! }
//! ```
//!
//! Adopted from the official `no_default_cases` lint.

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::{Program, TokenKind};

use crate::lint::correctness::exhaustive_cases::is_enum_like;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct NoDefaultCases;

impl Rule for NoDefaultCases {
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

impl Collector<'_> {
    fn is_enum_or_enum_like(&self, identity: &DeclarationIdentity, state: &SemanticState) -> bool {
        if self
            .ctx
            .types
            .is_some_and(|types| types.type_kind_identity(identity) == Some(TypeKind::Enum))
        {
            return true;
        }
        state
            .signatures
            .declaration(identity)
            .is_some_and(|facts| is_enum_like(facts, identity))
    }

    /// The default member includes its statements, not just its keyword.
    /// Skip any preceding case labels grouped with the same statement body.
    fn default_member_span(&self, case: &SwitchCase, index: usize) -> Span {
        let from = case.cases[..index]
            .iter()
            .rev()
            .find_map(|kind| match kind {
                SwitchCaseKind::Pattern(pattern, guard) => Some(
                    guard
                        .as_ref()
                        .map_or(pattern.span().end, |guard| guard.span().end),
                ),
                SwitchCaseKind::Expr(expr) => Some(expr.span().end),
                SwitchCaseKind::Default => None,
            })
            .unwrap_or(case.span.start);
        let keyword = self
            .ctx
            .syntax
            .keywords_in(Span::new(from, case.span.end), TokenKind::Default)
            .next()
            .map_or(case.span.start, |token| token.offset());
        let start = case
            .labels
            .iter()
            .filter(|label| from <= label.span.start && label.span.end <= keyword)
            .map(|label| label.span.start)
            .min()
            .unwrap_or(keyword);
        let end = if index + 1 == case.cases.len() {
            case.span.end
        } else {
            self.ctx
                .syntax
                .tokens_in(Span::new(keyword, case.span.end))
                .find(|token| token.kind == TokenKind::Colon)
                .map_or(keyword + "default:".len(), |token| {
                    token.offset() + token.len()
                })
        };
        Span::new(start, end)
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::Switch(switch) = node else {
            return;
        };
        let Some((case, index)) = switch.cases.iter().find_map(|case| {
            case.cases
                .iter()
                .position(|kind| matches!(kind, SwitchCaseKind::Default))
                .map(|index| (case, index))
        }) else {
            return;
        };
        // Enum-like extension types can hold values other than their declared
        // constants, so a `default` may be necessary there.
        let ResolvedType::Interface {
            identity,
            extension_type: false,
            ..
        } = state.infer(&switch.subject)
        else {
            return;
        };
        if self.is_enum_or_enum_like(&identity, state) {
            let span = self.default_member_span(case, index);
            self.diags.push(self.ctx.diagnostic(
                "no-default-cases",
                "Invalid use of 'default' member in a switch.",
                span,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference as span_test_support;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn reports_default_body_without_preceding_case_label() {
        let source = r"
enum E { a, b }
void use(E e) {
  switch (e) {
    case E.a:
    default:
      print(e);
      break;
  }
}
";
        let diagnostics = span_test_support::analyze(
            &NoDefaultCases,
            Path::new("test.dart"),
            source,
            &FalconConfig::default(),
        );
        assert_eq!(diagnostics.len(), 1);
        let span = diagnostics[0].span;
        assert_eq!(span.start, source.find("default:").unwrap());
        assert_eq!(
            &source[span.start..span.end],
            "default:\n      print(e);\n      break;"
        );
    }

    #[test]
    fn reports_default_labels_but_not_following_case_members() {
        for (statement, expected) in [
            (
                "switch (e) { case E.a: l: default: break; }",
                "l: default: break;",
            ),
            ("switch (e) { default: case E.a: print(e); }", "default:"),
        ] {
            let source = format!("enum E {{ a, b }}\nvoid use(E e) {{ {statement} }}");
            let diagnostics = span_test_support::analyze(
                &NoDefaultCases,
                Path::new("test.dart"),
                &source,
                &FalconConfig::default(),
            );
            assert_eq!(diagnostics.len(), 1);
            let span = diagnostics[0].span;
            assert_eq!(span.start, source.find(expected).unwrap());
            assert_eq!(&source[span.start..span.end], expected);
        }
    }
}
