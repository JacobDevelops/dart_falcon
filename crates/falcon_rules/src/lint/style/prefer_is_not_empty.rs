//! Flags `.length` comparisons equivalent to a `.isNotEmpty` check.
//!
//! Collections and strings expose an `isNotEmpty` getter that reads as plain
//! intent and, for lazy iterables, can avoid computing the full `length`.
//! Comparing `length` against a constant makes the reader infer the meaning, so
//! `list.isNotEmpty` should replace forms such as `length != 0`, `length > 0`,
//! and `length >= 1`, along with their operand-swapped mirrors (`0 != length`,
//! `0 < length`, `1 <= length`). Adopted from package:lints
//! `prefer_is_not_empty`.
//!
//! Only a plain `.length` property access matches against the relevant integer
//! literal. A null-aware `receiver?.length` is deliberately left alone: that
//! comparison also has to account for a `null` receiver, so `isNotEmpty` is not
//! a drop-in replacement.
//!
//! Type knowledge only ever *suppresses*. When a [`TypeIndex`] is on the context
//! and the receiver's static type is *positively proven* to be a concrete type
//! that (a) definitely has no `isNotEmpty` member (`member_lookup …
//! Absent`) and (b) is definitely not a core collection/string
//! (`is_subtype … No` for `Iterable`, `String`, `Map`), suggesting
//! `isNotEmpty` would be wrong, so the diagnostic is withheld. An `Unknown`
//! receiver — the common case, and every receiver when no type index is
//! attached — keeps firing.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use super::prefer_is_empty::lacks_collection_member;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct PreferIsNotEmpty;
impl Rule for PreferIsNotEmpty {
    fn name(&self) -> &'static str {
        "prefer-is-not-empty"
    }
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

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op,
            left,
            right,
            span,
        } = node
        else {
            return;
        };
        if let Some(receiver) = not_empty_comparison_receiver(op, left, right)
            && !lacks_collection_member(
                self.ctx,
                receiver,
                state,
                "isNotEmpty",
                &["Iterable", "String", "Map"],
            )
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-is-not-empty",
                "Use 'isNotEmpty' instead of comparing 'length' to 0.",
                *span,
            ));
        }
    }
}

fn not_empty_comparison_receiver<'a>(
    op: &BinaryOp,
    left: &'a Expr,
    right: &'a Expr,
) -> Option<&'a Expr> {
    match op {
        // length != 0 / 0 != length
        BinaryOp::NotEq => length_receiver(left)
            .filter(|_| is_int(right, 0))
            .or_else(|| length_receiver(right).filter(|_| is_int(left, 0))),
        // length > 0
        BinaryOp::Gt => length_receiver(left).filter(|_| is_int(right, 0)),
        // 0 < length
        BinaryOp::Lt => length_receiver(right).filter(|_| is_int(left, 0)),
        // length >= 1
        BinaryOp::GtEq => length_receiver(left).filter(|_| is_int(right, 1)),
        // 1 <= length
        BinaryOp::LtEq => length_receiver(right).filter(|_| is_int(left, 1)),
        _ => None,
    }
}

/// The receiver object of a `.length` property access, if `expr` is one.
fn length_receiver(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } if field.name == "length" => Some(object),
        _ => None,
    }
}

/// True for an integer literal equal to `n`.
fn is_int(expr: &Expr, n: i64) -> bool {
    if let Expr::IntLit { value, .. } = expr {
        value.replace('_', "").parse::<i64>().ok() == Some(n)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, SemanticModel, SignatureIndex, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Diagnostic count with no type index (baseline fire-on-unknown behavior).
    fn count_no_types(source: &str) -> usize {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(&path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let ctx = AnalyzeContext::new(&path, source, &config)
            .with_identities(&identities)
            .with_signatures(&signatures);
        PreferIsNotEmpty.analyze(&program, &ctx).len()
    }

    /// Diagnostic count with a single-file `TypeIndex` attached (production shape
    /// for a resolver-dependent rule in degraded single-file mode).
    fn count_with_types(source: &str) -> usize {
        let program = parse(source).0;
        let types = TypeIndex::from_program(&program);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        let ctx = AnalyzeContext::new(&path, source, &config)
            .with_types(&types)
            .with_identities(&identities)
            .with_signatures(&signatures);
        PreferIsNotEmpty.analyze(&program, &ctx).len()
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        let src = "void f() { if (list.length != 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_collection_suppresses_only_with_types() {
        let src = "class Ruler { int get length => 3; } \
                   void f(Ruler r) { if (r.length != 0) return; }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no isNotEmpty → suppress");
    }

    #[test]
    fn class_implementing_iterable_still_fires() {
        let src = "class Bag implements Iterable<int> { int get length => 0; } \
                   void f(Bag b) { if (b.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn supertype_leaving_project_still_fires() {
        let src = "class Offsite extends Frobnicator { int get length => 0; } \
                   void f(Offsite o) { if (o.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn string_receiver_still_fires() {
        let src = "void f(String s) { if (s.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn all_mirrored_forms_detected() {
        let src = "void f() { \
            if (a.length != 0) return; \
            if (0 != b.length) return; \
            if (c.length > 0) return; \
            if (0 < d.length) return; \
            if (e.length >= 1) return; \
            if (1 <= g.length) return; }";
        assert_eq!(count_no_types(src), 6);
    }
}
