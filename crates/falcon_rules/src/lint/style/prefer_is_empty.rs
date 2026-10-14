//! Flags `.length` comparisons equivalent to a `.isEmpty` check.
//!
//! Collections and strings expose an `isEmpty` getter that states intent
//! directly and, for lazy iterables, can be cheaper than computing the full
//! `length`. Comparing `length` against a constant forces the reader to decode
//! the boundary, so `list.isEmpty` should replace forms such as `length == 0`,
//! `length < 1`, and `length <= 0`, together with their operand-swapped mirrors
//! (`0 == length`, `1 > length`, `0 >= length`). Adopted from package:lints
//! `prefer_is_empty`.
//!
//! Only a plain `.length` property access matches against the relevant integer
//! literal. A null-aware `receiver?.length` is deliberately left alone: that
//! comparison also has to account for a `null` receiver, so `isEmpty` is not a
//! drop-in replacement.
//!
//! Type knowledge only ever *suppresses*. When a [`TypeIndex`] is on the context
//! and the receiver's static type is *positively proven* to be a concrete type
//! that (a) definitely has no `isEmpty` member (`member_lookup … Absent`)
//! and (b) is definitely not a core collection/string (`is_subtype … No`
//! for `Iterable`, `String`, `Map`), suggesting `isEmpty` would be wrong, so the
//! diagnostic is withheld. An `Unknown` receiver — the common case, and every
//! receiver when no type index is attached — keeps firing.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberResult, ResolvedType, Rule, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct PreferIsEmpty;
impl Rule for PreferIsEmpty {
    fn name(&self) -> &'static str {
        "prefer-is-empty"
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
pub(super) fn lacks_collection_member(
    ctx: &AnalyzeContext<'_>,
    receiver: &Expr,
    state: &SemanticState<'_>,
    member: &str,
    targets: &[&'static str],
) -> bool {
    let ty = state.infer(receiver);
    if state
        .signatures
        .resolved_member_facts(&ty, member, &state.model)
        .is_some()
        || targets.iter().any(|name| {
            state.signatures.is_subtype_of(
                &ty,
                &DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name,
                },
                &state.model,
            ) == Truth::Yes
        })
    {
        return false;
    }
    let (
        Some(types),
        ResolvedType::Interface {
            identity: identity @ DeclarationIdentity::Project { .. },
            ..
        },
    ) = (ctx.types, ty)
    else {
        return false;
    };
    let name = identity.name();
    // The identity index proves positive facts; TypeIndex still supplies negative proofs.
    types.member_lookup(name, member) == MemberResult::Absent
        && targets
            .iter()
            .all(|target| types.is_subtype(name, target) == Truth::No)
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
        if let Some(receiver) = empty_comparison_receiver(op, left, right)
            && !lacks_collection_member(
                self.ctx,
                receiver,
                state,
                "isEmpty",
                &["Iterable", "String", "Map"],
            )
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-is-empty",
                "Use 'isEmpty' instead of comparing 'length' to 0.",
                *span,
            ));
        }
    }
}

fn empty_comparison_receiver<'a>(
    op: &BinaryOp,
    left: &'a Expr,
    right: &'a Expr,
) -> Option<&'a Expr> {
    match op {
        // length == 0 / 0 == length
        BinaryOp::EqEq => length_receiver(left)
            .filter(|_| is_int(right, 0))
            .or_else(|| length_receiver(right).filter(|_| is_int(left, 0))),
        // length < 1
        BinaryOp::Lt => length_receiver(left).filter(|_| is_int(right, 1)),
        // 1 > length
        BinaryOp::Gt => length_receiver(right).filter(|_| is_int(left, 1)),
        // length <= 0
        BinaryOp::LtEq => length_receiver(left).filter(|_| is_int(right, 0)),
        // 0 >= length
        BinaryOp::GtEq => length_receiver(right).filter(|_| is_int(left, 0)),
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
        PreferIsEmpty.analyze(&program, &ctx).len()
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
        PreferIsEmpty.analyze(&program, &ctx).len()
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        // Bare identifier — Unknown type — must fire either way (the baseline).
        let src = "void f() { if (list.length == 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_collection_suppresses_only_with_types() {
        // `Ruler` has only `length`; not a collection/string → Absent isEmpty.
        let src = "class Ruler { int get length => 3; } \
                   void f(Ruler r) { if (r.length == 0) return; }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no isEmpty → suppress");
    }

    #[test]
    fn class_implementing_iterable_still_fires() {
        // Bag inherits `isEmpty` from Iterable → member Found, not Absent.
        let src = "class Bag implements Iterable<int> { int get length => 0; } \
                   void f(Bag b) { if (b.length == 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn supertype_leaving_project_still_fires() {
        // Unknown ancestor → chain not fully resolved → Unknown, never suppress.
        let src = "class Offsite extends Frobnicator { int get length => 0; } \
                   void f(Offsite o) { if (o.length == 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn static_method_call_receiver_still_fires() {
        // `Config.load()` is a *static method* returning a Map, not a `Config`
        // instance. Its receiver type must stay Unknown, so the diagnostic keeps
        // firing — mis-typing it as `Config` (which lacks isEmpty) would suppress.
        let src = "class Config { static Map<String, String> load() => {}; } \
                   void f() { if (Config.load().length == 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(
            count_with_types(src),
            1,
            "static-method return is Unknown → fire"
        );
    }

    #[test]
    fn named_constructor_call_receiver_suppresses() {
        // `Box.zero()` is a named constructor → receiver type is `Box`. `Box` has
        // only `length`, no isEmpty, and is no collection → proven → suppress.
        let src = "class Box { Box.zero(); int get length => 0; } \
                   void f() { if (Box.zero().length == 0) return; }";

        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(
            count_with_types(src),
            0,
            "named ctor → Box, proven no isEmpty → suppress"
        );
    }

    #[test]
    fn string_receiver_still_fires() {
        // String has isEmpty; suggesting isEmpty is correct → keep firing.
        let src = "void f(String s) { if (s.length == 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn all_mirrored_forms_detected() {
        let src = "void f() { \
            if (a.length == 0) return; \
            if (0 == b.length) return; \
            if (c.length < 1) return; \
            if (d.length <= 0) return; \
            if (1 > e.length) return; \
            if (0 >= g.length) return; }";
        assert_eq!(count_no_types(src), 6);
    }
}
