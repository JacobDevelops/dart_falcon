//! Only throw instances of classes that extend `Exception` or `Error`.
//!
//! Throwing arbitrary objects breaks `on Exception` / `on Error` handlers and
//! loses the conventions tooling relies on:
//!
//! ```dart
//! throw 'Something went wrong'; // BAD
//! throw FormatException('Something went wrong'); // GOOD
//! ```
//!
//! Literals are always reported. Other thrown values are reported only when
//! their resolved type provably implements neither `Exception` nor `Error`;
//! `dynamic`, `Never`, and anything unresolved stay silent. Function and
//! record values cannot implement either throwable interface and are reported.
//! Resolved types include promotions on the current control-flow path, and
//! extension types are checked through their representation types.
//!
//! Adopted from the official `only_throw_errors` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct OnlyThrowErrors;

impl Rule for OnlyThrowErrors {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diagnostics: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check(&mut self, thrown: Expr<'_>, state: &SemanticState<'_>) {
        if is_literal(thrown) || not_throwable(&state.infer(thrown), state) {
            self.diagnostics.push(self.ctx.diagnostic("Don't create and throw instances of classes that don't extend 'Exception' or 'Error'.", thrown.span()));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        if let Expr::Throw(expr) = node {
            self.check(expr.expr(), state);
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'_>) {
        if let Stmt::Throw(statement) = node {
            self.check(statement.value(), state);
        }
    }
}

fn is_literal(expr: Expr<'_>) -> bool {
    matches!(
        expr.unparenthesized(),
        Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::SymbolLit(_)
            | Expr::List(_)
            | Expr::Map(_)
            | Expr::Set(_)
            | Expr::Record(_)
    )
}

/// Whether `ty` provably implements neither `dart:core` `Exception` nor `Error`.
/// Like the analyzer's `typeForInterfaceCheck`, an extension type is checked
/// through its representation type.
fn not_throwable(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    let mut ty = ty;
    while let ResolvedType::TypeParameter { bound, .. }
    | ResolvedType::NonNullTypeParameter { bound, .. } = ty
    {
        ty = bound;
    }
    let erased;
    let ty = if matches!(
        ty,
        ResolvedType::Interface {
            extension_type: true,
            ..
        }
    ) {
        let Some(representation) = state.signatures.extension_type_erasure(ty, &state.model) else {
            return false;
        };
        erased = representation;
        &erased
    } else {
        ty
    };
    if matches!(
        ty,
        ResolvedType::Function { .. } | ResolvedType::Record { .. }
    ) {
        return true;
    }
    if !matches!(
        ty,
        ResolvedType::Interface { .. }
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::NonNullTypeParameter { .. }
    ) {
        return false;
    }
    [("dart:core", "Exception"), ("dart:core", "Error")]
        .iter()
        .all(|(library, name)| {
            state
                .signatures
                .interface_check_sdk(ty, library, name, &state.model)
                == Truth::No
        })
}
