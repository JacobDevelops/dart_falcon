//! Flags `.where((e) => e is T)`, which `.whereType<T>()` expresses directly.
//! Adopted from package:lints `prefer_iterable_whereType`.
//!
//! Filtering an iterable with an `is` test is exactly what `Iterable.whereType`
//! does, but the getter also narrows the element type, yielding an
//! `Iterable<T>` instead of leaving the original element type in place. Using
//! `whereType<T>()` is shorter, avoids the throwaway closure, and removes the
//! casts or checks that a bare `where` filter would still require downstream.
//! The rule matches a `.where` call carrying exactly one positional argument and
//! no named arguments, where that argument is a one-parameter arrow closure of
//! the form `param is T` — the test must be non-negated and applied to the
//! closure's own parameter.
//!
//! Type knowledge only ever *suppresses*. When a [`TypeIndex`] is on the context
//! and the receiver's static type is *positively proven* to be a concrete type
//! that (a) is definitely not an `Iterable` (`is_subtype … No`) and (b)
//! definitely has no `whereType` member (`member_lookup … Absent`), then
//! `.whereType<T>()` does not exist on the receiver and rewriting would be wrong,
//! so the diagnostic is withheld. A user type carrying its own `whereType` keeps
//! firing (the rewrite is valid there), and an `Unknown` receiver — the common
//! case, and every receiver when no type index is attached — keeps firing.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use super::prefer_is_empty::lacks_collection_member;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct PreferIterableWhereType;
impl Rule for PreferIterableWhereType {
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
        let Expr::Call { callee, args, .. } = node else {
            return;
        };
        let Expr::Field { object, field, .. } = callee.as_ref() else {
            return;
        };
        if field.name == "where"
            && args.named.is_empty()
            && args.positional.len() == 1
            && is_is_check_closure(&args.positional[0])
            && !lacks_collection_member(self.ctx, object, state, "whereType", &["Iterable"])
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-iterable-where-type",
                "Use 'whereType<T>()' instead of 'where' with an 'is' check.",
                field.span,
            ));
        }
    }
}

fn is_is_check_closure(expr: &Expr) -> bool {
    let Expr::FuncExpr { params, body, .. } = expr else {
        return false;
    };
    let [param] = params.params.as_slice() else {
        return false;
    };
    if param.kind != falcon_syntax::ast::ParamKind::Positional {
        return false;
    }
    let param_name = &param.name.name;
    if let FunctionBody::Arrow(inner, _) = &**body
        && let Expr::Is {
            expr: tested,
            negated: false,
            ..
        } = &**inner
        && let Expr::Ident(id) = &**tested
    {
        return &id.name == param_name;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, SemanticModel, SignatureIndex, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

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
        PreferIterableWhereType.analyze(&program, &ctx).len()
    }

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
        PreferIterableWhereType.analyze(&program, &ctx).len()
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        // Bare identifier — Unknown type — must fire either way (the baseline).
        let src = "void f() { var a = items.where((e) => e is String); }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_iterable_param_suppresses_only_with_types() {
        // `Query` is not an Iterable and has no `whereType` → the rewrite is bogus.
        let src = "class Query { Query where(bool Function(dynamic) f) => this; } \
                   void f(Query q) { q.where((e) => e is String); }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no whereType → suppress");
    }

    #[test]
    fn proven_non_iterable_this_and_constructor_suppress() {
        let src = "class Query { \
                     Query where(bool Function(dynamic) f) => this; \
                     void run() { this.where((e) => e is int); Query().where((e) => e is num); } \
                   }";
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn iterable_receiver_still_fires() {
        // A `List` receiver IS an Iterable → is_subtype is Yes, not No → fire.
        let src = "void f(List<int> xs) { xs.where((e) => e is int); }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn user_type_with_where_type_still_fires() {
        // `whereType` present → member_lookup Found, not Absent → fire.
        let src = "class Custom { \
                     Custom where(bool Function(dynamic) f) => this; \
                     Iterable<T> whereType<T>() => throw ''; \
                     void run() { this.where((e) => e is String); } \
                   }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn negated_and_multi_param_never_match() {
        assert_eq!(
            count_no_types("void f() { a.where((e) => e is! String); }"),
            0
        );
        assert_eq!(
            count_no_types("void f() { a.where((x, y) => x is int); }"),
            0
        );
    }
}
