use falcon_analyze::{AnalyzeContext, DeclarationIdentity, MemberResult, ResolvedType, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

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

#[derive(Clone, Copy)]
pub(super) enum Emptiness {
    Empty,
    NotEmpty,
}

impl Emptiness {
    fn details(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Empty => (
                "prefer-is-empty",
                "isEmpty",
                "Use 'isEmpty' instead of comparing 'length' to 0.",
            ),
            Self::NotEmpty => (
                "prefer-is-not-empty",
                "isNotEmpty",
                "Use 'isNotEmpty' instead of comparing 'length' to 0.",
            ),
        }
    }

    fn comparison_receiver<'a>(
        self,
        op: BinaryOp,
        left: &'a Expr,
        right: &'a Expr,
    ) -> Option<&'a Expr> {
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
            let Expr::Field {
                object,
                field,
                is_null_safe: false,
                ..
            } = length
            else {
                return None;
            };
            if field.name == "length"
                && matches!(literal, Expr::IntLit { value: text, .. }
                    if text.replace('_', "").parse::<i64>().ok() == Some(value))
            {
                Some(object.as_ref())
            } else {
                None
            }
        })
    }
}

pub(super) fn analyze_emptiness(
    program: &Program,
    ctx: &AnalyzeContext<'_>,
    emptiness: Emptiness,
) -> Vec<Diagnostic> {
    struct Collector<'a> {
        ctx: &'a AnalyzeContext<'a>,
        emptiness: Emptiness,
        diags: Vec<Diagnostic>,
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
            let (rule, member, message) = self.emptiness.details();
            if let Some(receiver) = self.emptiness.comparison_receiver(*op, left, right)
                && !lacks_collection_member(
                    self.ctx,
                    receiver,
                    state,
                    member,
                    &["Iterable", "String", "Map"],
                )
            {
                self.diags.push(self.ctx.diagnostic(rule, message, *span));
            }
        }
    }
    let Some(state) = SemanticState::new(program, ctx) else {
        return Vec::new();
    };
    let mut collector = Collector {
        ctx,
        emptiness,
        diags: Vec::new(),
    };
    visit_program(&mut collector, program, state);
    collector.diags
}

#[cfg(test)]
mod tests {
    use super::super::prefer_is_empty::PreferIsEmpty;
    use super::super::prefer_is_not_empty::PreferIsNotEmpty;
    use super::super::prefer_iterable_where_type::PreferIterableWhereType;
    use super::*;
    use falcon_analyze::Rule;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn resolverless_context_preserves_no_diagnostics() {
        let source = "void f(String s, List<Object> xs) {
            print(s.length == 0); print(s.length != 0);
            xs.where((e) => e is String);
        }";
        let program = parse(source).0;
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config);
        for rule in [
            &PreferIsEmpty as &dyn Rule,
            &PreferIsNotEmpty,
            &PreferIterableWhereType,
        ] {
            assert!(rule.analyze(&program, &ctx).is_empty());
        }
    }
}
