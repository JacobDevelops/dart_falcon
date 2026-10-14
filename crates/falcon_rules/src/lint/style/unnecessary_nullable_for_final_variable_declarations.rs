//! Flags a `final` or `const` variable given a nullable type but initialized to a
//! provably non-null value.
//!
//! A `final int? x = 3;` can never hold null: it is initialized once, at the
//! declaration, to a value that is obviously non-null. The `?` widens the static
//! type for no reason, forcing needless null checks at every use. The rule is
//! deliberately conservative about "provably non-null" — only literals (numbers,
//! strings, booleans, collections, records), constructor invocations, and function
//! expressions qualify; anything whose value flows from another binding is left
//! alone. It applies to final/const top-level variables, static fields, and local
//! variables. Drop the `?` from the declared type.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_top_level_decl};

pub struct UnnecessaryNullableForFinalVariableDeclarations;

impl Rule for UnnecessaryNullableForFinalVariableDeclarations {
    fn name(&self) -> &'static str {
        "unnecessary-nullable-for-final-variable-declarations"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Only literals and constructor invocations are treated as provably non-null;
/// anything whose value depends on another binding is left alone.
fn is_provably_non_null(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::StringLit(_)
            | Expr::BoolLit { .. }
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. }
            | Expr::New { .. }
            | Expr::FuncExpr { .. }
    )
}

impl Collector<'_> {
    fn check(
        &mut self,
        keyword: VarKeyword,
        var_type: &Option<DartType>,
        declarators: &[VarDeclarator],
    ) {
        if keyword == VarKeyword::Var {
            return;
        }
        let Some(t) = var_type else { return };
        if !t.is_nullable() {
            return;
        }
        // The type is shared by every declarator, so it is only unnecessarily
        // nullable when *all* of them are provably non-null.
        if !declarators
            .iter()
            .all(|d| d.initializer.as_ref().is_some_and(is_provably_non_null))
        {
            return;
        }
        for d in declarators {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-nullable-for-final-variable-declarations",
                "Unnecessary nullable type for a final variable declaration.",
                d.span,
            ));
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node {
            self.check(v.keyword, &v.var_type, &v.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.is_static {
            self.check(node.keyword, &node.field_type, &node.declarators);
        }
    }

    fn visit_local_var(&mut self, lv: &LocalVarDecl) {
        self.check(lv.keyword, &lv.var_type, &lv.declarators);
        falcon_syntax::visitor::walk_local_var(self, lv);
    }
}
