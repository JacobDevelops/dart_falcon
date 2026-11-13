use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberResult, ResolvedTypeView, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

pub(crate) fn lacks_collection_member(
    ctx: &AnalyzeContext<'_>,
    receiver: Expr<'_>,
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
            state.signatures.relation(
                &ty,
                TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::sdk("dart:core", name)),
                &state.model,
            ) == Truth::Yes
        })
    {
        return false;
    }
    let (Some(types), ResolvedTypeView::Interface { identity, .. }) = (ctx.types(), ty.view())
    else {
        return false;
    };
    // The identity index proves positive facts; TypeIndex still supplies negative proofs.
    types.member_lookup_identity(identity, member) == MemberResult::Absent
        && targets.iter().all(|target| {
            types.supertype_matches(
                identity,
                |ancestor| ancestor.is_sdk("dart:core", target),
                |name| name == *target,
            ) == Truth::No
        })
}

#[derive(Clone, Copy)]
pub(crate) enum Emptiness {
    LengthComparison,
    NegatedIsEmpty,
}

enum EmptinessCheck {
    Empty,
    NotEmpty,
    AlwaysTrue,
    AlwaysFalse,
    NegatedIsEmpty,
}

impl EmptinessCheck {
    fn details(&self) -> (&'static str, &'static str) {
        match self {
            Self::Empty => ("isEmpty", "Use `isEmpty` instead of comparing `length`."),
            Self::NotEmpty => (
                "isNotEmpty",
                "Use `isNotEmpty` instead of comparing `length`.",
            ),
            Self::AlwaysTrue => (
                "isEmpty",
                "The comparison is always `true` because `length` is nonnegative.",
            ),
            Self::AlwaysFalse => (
                "isEmpty",
                "The comparison is always `false` because `length` is nonnegative.",
            ),
            Self::NegatedIsEmpty => (
                "isNotEmpty",
                "Use `isNotEmpty` rather than negating `isEmpty`.",
            ),
        }
    }
}

fn length_comparison(binary: BinaryExpr<'_>) -> Option<(Expr<'_>, EmptinessCheck)> {
    let (length, literal, op) = if matches!(binary.left().unparenthesized(), Expr::IntLit(_)) {
        let op = match binary.op() {
            BinaryOp::Lt => BinaryOp::Gt,
            BinaryOp::LtEq => BinaryOp::GtEq,
            BinaryOp::Gt => BinaryOp::Lt,
            BinaryOp::GtEq => BinaryOp::LtEq,
            op => op,
        };
        (binary.right(), binary.left(), op)
    } else {
        (binary.left(), binary.right(), binary.op())
    };
    let Expr::Field(field) = length.unparenthesized() else {
        return None;
    };
    let Expr::IntLit(literal) = literal.unparenthesized() else {
        return None;
    };
    if field.is_null_safe() || field.field().name() != "length" {
        return None;
    }
    let value = falcon_analyze::parse_int(literal.value())?;
    let check = match (op, value) {
        (BinaryOp::EqEq | BinaryOp::LtEq, 0) | (BinaryOp::Lt, 1) => EmptinessCheck::Empty,
        (BinaryOp::NotEq | BinaryOp::Gt, 0) | (BinaryOp::GtEq, 1) => EmptinessCheck::NotEmpty,
        (BinaryOp::GtEq, 0) => EmptinessCheck::AlwaysTrue,
        (BinaryOp::Lt, 0) => EmptinessCheck::AlwaysFalse,
        _ => return None,
    };
    Some((field.object(), check))
}

pub(crate) fn analyze_emptiness<'a>(
    ctx: &'a AnalyzeContext<'a>,
    emptiness: Emptiness,
) -> Box<dyn SemanticRuleVisitor<'a> + 'a> {
    struct Collector<'a> {
        ctx: &'a AnalyzeContext<'a>,
        emptiness: Emptiness,
        diags: Vec<Diagnostic>,
    }
    impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            self.diags
        }

        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            let (receiver, check) = match (self.emptiness, node) {
                (Emptiness::LengthComparison, Expr::Binary(binary)) => {
                    let Some((receiver, check)) = length_comparison(binary) else {
                        return;
                    };
                    (receiver, check)
                }
                (Emptiness::NegatedIsEmpty, Expr::Unary(unary)) if unary.op() == UnaryOp::Bang => {
                    let Expr::Field(field) = unary.operand().unparenthesized() else {
                        return;
                    };
                    if field.is_null_safe() || field.field().name() != "isEmpty" {
                        return;
                    }
                    (field.object(), EmptinessCheck::NegatedIsEmpty)
                }
                _ => return,
            };
            let (member, message) = check.details();
            if !lacks_collection_member(
                self.ctx,
                receiver,
                state,
                member,
                &["Iterable", "String", "Map"],
            ) {
                self.diags.push(self.ctx.diagnostic(message, node.span()));
            }
        }
    }
    Box::new(Collector {
        ctx,
        emptiness,
        diags: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::style::prefer_is_empty::PreferIsEmpty;
    use crate::lint::style::prefer_is_not_empty::PreferIsNotEmpty;
    use crate::lint::style::prefer_iterable_where_type::PreferIterableWhereType;
    use falcon_analyze::Rule;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;

    use std::path::Path;

    #[test]
    fn resolverless_context_preserves_no_diagnostics() {
        let source = "void f(String s, List<Object> xs) {
            print(s.length == 0); print(s.length != 0);
            xs.where((e) => e is String);
        }";
        let parsed = falcon_analyze::ParsedFile::parse(source);
        let parsed = std::sync::Arc::new(parsed);
        let program = parsed.program();
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        for (name, rule) in [
            (PreferIsEmpty::NAME, &PreferIsEmpty as &dyn Rule),
            (PreferIsNotEmpty::NAME, &PreferIsNotEmpty),
            (PreferIterableWhereType::NAME, &PreferIterableWhereType),
        ] {
            assert!(rule.analyze(name, program, &ctx).is_empty());
        }
    }
}
