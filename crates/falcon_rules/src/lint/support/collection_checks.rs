use falcon_analyze::{AnalyzeContext, DeclarationIdentity, MemberResult, ResolvedType, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

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
            state.signatures.is_subtype_of(
                &ty,
                &DeclarationIdentity::sdk("dart:core", name),
                &state.model,
            ) == Truth::Yes
        })
    {
        return false;
    }
    let (Some(types), ResolvedType::Interface { identity, .. }) = (ctx.types(), ty) else {
        return false;
    };
    // The identity index proves positive facts; TypeIndex still supplies negative proofs.
    types.member_lookup_identity(&identity, member) == MemberResult::Absent
        && targets.iter().all(|target| {
            types.is_subtype_identity(&identity, &DeclarationIdentity::sdk("dart:core", target))
                == Truth::No
        })
}

#[derive(Clone, Copy)]
pub(crate) enum Emptiness {
    Empty,
    NotEmpty,
}

impl Emptiness {
    fn details(self) -> (&'static str, &'static str) {
        match self {
            Self::Empty => (
                "isEmpty",
                "Use `isEmpty` instead of comparing `length` to 0.",
            ),
            Self::NotEmpty => (
                "isNotEmpty",
                "Use `isNotEmpty` instead of comparing `length` to 0.",
            ),
        }
    }

    fn comparison_receiver<'a>(
        self,
        op: BinaryOp,
        left: Expr<'a>,
        right: Expr<'a>,
    ) -> Option<Expr<'a>> {
        let forms = match self {
            Self::Empty => &[
                (BinaryOp::EqEq, 0, false),
                (BinaryOp::EqEq, 0, true),
                (BinaryOp::Lt, 1, false),
                (BinaryOp::Gt, 1, true),
                (BinaryOp::LtEq, 0, false),
                (BinaryOp::GtEq, 0, true),
            ],
            Self::NotEmpty => &[
                (BinaryOp::NotEq, 0, false),
                (BinaryOp::NotEq, 0, true),
                (BinaryOp::Gt, 0, false),
                (BinaryOp::Lt, 0, true),
                (BinaryOp::GtEq, 1, false),
                (BinaryOp::LtEq, 1, true),
            ],
        };
        forms.iter().find_map(|&(comparison, value, swapped)| {
            if op != comparison {
                return None;
            }
            let (length, literal) = if swapped {
                (right, left)
            } else {
                (left, right)
            };
            let Expr::Field(field) = length.unparenthesized() else {
                return None;
            };
            if !field.is_null_safe()
                && field.field().name() == "length"
                && matches!(literal.unparenthesized(), Expr::IntLit(text)
                    if text.value().replace('_', "").parse::<i64>().ok() == Some(value))
            {
                Some(field.object())
            } else {
                None
            }
        })
    }
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
            let Expr::Binary(binary) = node else {
                return;
            };
            let (member, message) = self.emptiness.details();
            if let Some(receiver) =
                self.emptiness
                    .comparison_receiver(binary.op(), binary.left(), binary.right())
                && !lacks_collection_member(
                    self.ctx,
                    receiver,
                    state,
                    member,
                    &["Iterable", "String", "Map"],
                )
            {
                self.diags.push(self.ctx.diagnostic(message, binary.span()));
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
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn resolverless_context_preserves_no_diagnostics() {
        let source = "void f(String s, List<Object> xs) {
            print(s.length == 0); print(s.length != 0);
            xs.where((e) => e is String);
        }";
        let parsed = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.tree.program(source);
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
