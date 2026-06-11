use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidRedundantAsync;

impl Rule for AvoidRedundantAsync {
    fn name(&self) -> &'static str {
        "avoid-redundant-async"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func_decl) => {
                    if let Some(diag) = check_function_async(func_decl, ctx) {
                        diags.push(diag);
                    }
                }
                TopLevelDecl::Class(class_decl) => {
                    for member in &class_decl.members {
                        if let ClassMember::Method(method_decl) = member
                            && let Some(diag) = check_method_async(method_decl, ctx)
                        {
                            diags.push(diag);
                        }
                    }
                }
                _ => {}
            }
        }

        diags
    }
}

fn check_function_async(func_decl: &FunctionDecl, ctx: &AnalyzeContext) -> Option<Diagnostic> {
    if !func_decl.is_async {
        return None;
    }

    if is_redundant_async_body(&func_decl.body) {
        return Some(Diagnostic::new(
            "avoid-redundant-async",
            Severity::Warning,
            "Unnecessary 'async' modifier — remove 'async' and 'await' to return the Future directly",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: func_decl.name.span.start,
                end: func_decl.name.span.end,
            },
        ));
    }

    None
}

fn check_method_async(method_decl: &MethodDecl, ctx: &AnalyzeContext) -> Option<Diagnostic> {
    if !method_decl.is_async {
        return None;
    }

    if is_redundant_async_body(&method_decl.body) {
        return Some(Diagnostic::new(
            "avoid-redundant-async",
            Severity::Warning,
            "Unnecessary 'async' modifier — remove 'async' and 'await' to return the Future directly",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: method_decl.name.span.start,
                end: method_decl.name.span.end,
            },
        ));
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
