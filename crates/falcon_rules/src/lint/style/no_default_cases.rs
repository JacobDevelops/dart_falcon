//! Flags `default:` in switch statements over enums and enum-like classes.
//!
//! A `default` case hides newly added enum values: the switch keeps compiling
//! and silently routes them to the fallback. Listing every value explicitly
//! lets the compiler (or `exhaustive-cases`) point at the switch when the type
//! grows. Only switch statements whose subject is provably an enum or an
//! enum-like class (private non-factory constructors, at least two static
//! constants of its own type, no subclasses in its library) are checked.
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

    /// The `default` keyword of the label at `index` in `case`: it follows the
    /// previous label's pattern (or guard), or opens the case.
    fn default_keyword(&self, case: &SwitchCase, index: usize) -> Span {
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
        let start = self
            .ctx
            .syntax
            .keywords_in(Span::new(from, case.span.end), TokenKind::Default)
            .next()
            .map_or(case.span.start, |token| token.offset());
        Span::new(start, start + "default".len())
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
            let span = self.default_keyword(case, index);
            self.diags.push(self.ctx.diagnostic(
                "no-default-cases",
                "Invalid use of 'default' member in a switch.",
                span,
            ));
        }
    }
}
