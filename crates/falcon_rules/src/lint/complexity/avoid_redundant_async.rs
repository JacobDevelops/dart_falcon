//! Flags an `async` function whose body is a single `return await ...;`.
//!
//! When a function does nothing with the awaited value but return it, the
//! `async`/`await` round-trip is redundant: returning the `Future` directly is
//! equivalent and skips an extra microtask hop and stack frame. The rule fires
//! on a function or method marked `async` whose block body is exactly one
//! statement of the form `return await <expr>;`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_top_level_decl};

#[derive(Default)]
pub struct AvoidRedundantAsync;

impl Rule for AvoidRedundantAsync {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        AsyncDeclarations {
            diags: &mut diags,
            ctx,
        }
        .visit_program(program);

        diags
    }
}

struct AsyncDeclarations<'a, 'ctx> {
    diags: &'a mut Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for AsyncDeclarations<'_, '_> {
    fn visit_expr(&mut self, _node: &Expr) {}

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if !matches!(node, TopLevelDecl::Variable(_)) {
            walk_top_level_decl(self, node);
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if let Some(diag) = check_function_async(node, self.ctx) {
            self.diags.push(diag);
        }
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Method(method) = node
            && let Some(diag) = check_method_async(method, self.ctx)
        {
            self.diags.push(diag);
        }
    }
}

fn check_function_async(func_decl: &FunctionDecl, ctx: &AnalyzeContext) -> Option<Diagnostic> {
    if !func_decl.async_marker.is_async() {
        return None;
    }

    if is_redundant_async_body(&func_decl.body) {
        return Some(ctx.diagnostic("avoid-redundant-async", "Unnecessary 'async' modifier — remove 'async' and 'await' to return the Future directly", func_decl.name.span));
    }

    None
}

fn check_method_async(method_decl: &MethodDecl, ctx: &AnalyzeContext) -> Option<Diagnostic> {
    if !method_decl.async_marker.is_async() {
        return None;
    }

    if is_redundant_async_body(&method_decl.body) {
        return Some(ctx.diagnostic("avoid-redundant-async", "Unnecessary 'async' modifier — remove 'async' and 'await' to return the Future directly", method_decl.name.span));
    }

    None
}

fn is_redundant_async_body(body: &Option<FunctionBody>) -> bool {
    match body {
        Some(FunctionBody::Block(block)) => {
            block.stmts.len() == 1
                && matches!(
                    &block.stmts[0],
                    Stmt::Return(ReturnStmt {
                        value: Some(Expr::Await { .. }),
                        ..
                    })
                )
        }
        _ => false,
    }
}
