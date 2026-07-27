//! Disallow nullable return types on functions that never return null.
//!
//! Flags a function or method whose return type is nullable (`T?`) even though
//! every `return` in its body yields a provably non-null value. A needless `?`
//! forces callers to null-check a result that can never be null, spreading
//! defensive code that adds no safety. Drop the `?` from the return type. Only
//! the outer nullability counts, so `Future<T?>` and `List<int?>` are left
//! alone. Without full type resolution the check is conservative.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_function_decl, walk_getter_decl, walk_method_decl, walk_stmt,
};

pub struct UnnecessaryNullableReturnType;

impl Rule for UnnecessaryNullableReturnType {
    fn name(&self) -> &'static str {
        "unnecessary-nullable-return-type"
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

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        check(
            node.return_type.as_ref(),
            node.body.as_ref(),
            &mut self.diags,
            self.ctx,
        );
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        check(
            node.return_type.as_ref(),
            node.body.as_ref(),
            &mut self.diags,
            self.ctx,
        );
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        check(
            node.return_type.as_ref(),
            node.body.as_ref(),
            &mut self.diags,
            self.ctx,
        );
        walk_getter_decl(self, node);
    }
}

fn check(
    return_type: Option<&DartType>,
    body: Option<&FunctionBody>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(return_type) = return_type
        && is_outer_nullable(return_type)
        && let Some(body) = body
        && all_returns_provably_non_null(body)
    {
        flag_return_type(return_type, diags, ctx);
    }
}

fn is_outer_nullable(ty: &DartType) -> bool {
    match ty {
        DartType::Named(nt) => nt.is_nullable,
        DartType::Function(ft) => ft.is_nullable,
        DartType::Record(rt) => rt.is_nullable,
        _ => false,
    }
}

fn all_returns_provably_non_null(body: &FunctionBody) -> bool {
    match body {
        FunctionBody::Block(block) => {
            let mut scan = ReturnScan {
                count: 0,
                all_non_null: true,
            };
            for stmt in &block.stmts {
                scan.visit_stmt(stmt);
            }
            scan.count > 0 && scan.all_non_null
        }
        FunctionBody::Arrow(expr, _) => is_provably_non_null(expr),
        FunctionBody::Native(_, _) => false,
    }
}

struct ReturnScan {
    count: usize,
    all_non_null: bool,
}

impl Visitor for ReturnScan {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Return(ret) => {
                self.count += 1;
                if !ret.value.as_ref().is_some_and(is_provably_non_null) {
                    self.all_non_null = false;
                }
            }
            // Returns in delayed nested functions do not contribute to the outer
            // function's return contract.
            Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if !matches!(node, Expr::FuncExpr { .. }) {
            walk_expr(self, node);
        }
    }
}

fn is_provably_non_null(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::StringLit(_)
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. }
            | Expr::New { .. }
    )
}

fn flag_return_type(ty: &DartType, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let span = ty.span();
    diags.push(Diagnostic::new(
        "unnecessary-nullable-return-type",
        Severity::Warning,
        "Function return type is unnecessarily nullable",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
