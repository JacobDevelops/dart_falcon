//! Flags `.where((e) => e is T)`, which `.whereType<T>()` expresses directly.
//! Adopted from package:lints `prefer_iterable_whereType`.
//!
//! Filtering an iterable with an `is` test is exactly what `Iterable.whereType`
//! does, but the method also narrows the element type, yielding an
//! `Iterable<T>` instead of leaving the original element type in place. Using
//! `whereType<T>()` is shorter, avoids the throwaway closure, and removes the
//! casts or checks that a bare `where` filter would still require downstream.
//! The rule matches a `.where` call carrying exactly one positional argument and
//! no named arguments, where that argument is a one-parameter arrow closure of
//! the form `param is T` — the test must be non-negated and applied to the
//! closure's own parameter.
//!
//! Semantic analysis requires declaration identities. Once available, an
//! unknown receiver still triggers the lint. A `TypeIndex` can suppress it only
//! when the receiver is proven not to be an `Iterable` and to have no
//! `whereType` member; source-resolved members and subtypes take precedence.
//! A user type carrying its own `whereType` keeps firing. Without a type index,
//! negative proofs are unavailable, but source-derived types are still inferred.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::support::collection_checks::lacks_collection_member;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferIterableWhereType;
impl Rule for PreferIterableWhereType {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
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

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return;
        };
        let mut arguments = call.args().positional();
        if field.field().name() == "where"
            && call.args().named().is_empty()
            && let (Some(predicate), None) = (arguments.next(), arguments.next())
            && is_is_check_closure(predicate)
            && !lacks_collection_member(self.ctx, field.object(), state, "whereType", &["Iterable"])
        {
            self.diags.push(self.ctx.diagnostic("Use `whereType<T>()` instead of `where` with an `is` check.", field.field().span()));
        }
    }
}

fn is_is_check_closure(expr: Expr<'_>) -> bool {
    let Expr::FuncExpr(function) = expr.unparenthesized() else {
        return false;
    };
    let mut params = function.params().params();
    let (Some(param), None) = (params.next(), params.next()) else {
        return false;
    };
    if param.kind() != ParamKind::Positional {
        return false;
    }
    let param_name = param.name().name();
    if let FunctionBody::Arrow(body) = function.body()
        && let Expr::Is(test) = body.expr().unparenthesized()
        && !test.negated()
        && let Expr::Ident(id) = test.expr().unparenthesized()
    {
        return id.name() == param_name;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::test_support::count;

    fn count_no_types(source: &str) -> usize {
        count("prefer-iterable-where-type", &PreferIterableWhereType, source, false)
    }

    fn count_with_types(source: &str) -> usize {
        count("prefer-iterable-where-type", &PreferIterableWhereType, source, true)
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
