//! Flags empty blocks (`{}`).
//!
//! An empty function, method, loop, or switch-case body is frequently an
//! unfinished implementation or a leftover after code was deleted, and it hides
//! whether the emptiness is deliberate. Fill in the intended behavior, or, if a
//! no-op is genuinely correct, add a comment saying so. A block containing only
//! a comment is treated as intentional and left alone. The report lands on the
//! closing brace.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_function_decl,
    walk_getter_decl, walk_method_decl, walk_setter_decl, walk_stmt,
};

pub struct NoEmptyBlock;

impl Rule for NoEmptyBlock {
    fn name(&self) -> &'static str {
        "no-empty-block"
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

impl Collector<'_, '_> {
    fn check_body(&mut self, body: Option<&FunctionBody>) {
        if let Some(FunctionBody::Block(block)) = body {
            flag_if_empty(block, &mut self.diags, self.ctx);
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.check_body(node.body.as_ref());
        walk_function_decl(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.check_body(node.body.as_ref());
        walk_constructor_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check_body(node.body.as_ref());
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.check_body(node.body.as_ref());
        walk_getter_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.check_body(node.body.as_ref());
        walk_setter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check_body(operator.body.as_ref());
        }
        walk_class_member(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => flag_if_empty(block, &mut self.diags, self.ctx),
            Stmt::TryCatch(try_catch) => {
                flag_if_empty(&try_catch.body, &mut self.diags, self.ctx);
                for catch in &try_catch.catches {
                    flag_if_empty(&catch.body, &mut self.diags, self.ctx);
                }
                if let Some(finally) = &try_catch.finally {
                    flag_if_empty(finally, &mut self.diags, self.ctx);
                }
            }
            Stmt::LocalFunc(local) => self.check_body(Some(&local.body)),
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { body, .. } = node {
            self.check_body(Some(body));
        }
        walk_expr(self, node);
    }
}

fn flag_if_empty(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !block.stmts.is_empty() {
        return;
    }
    let end = block.span.end.min(ctx.source.len());
    let src_full = &ctx.source[block.span.start..end];
    // span_from() includes content up to the next token, so rfind('}') finds
    // the actual closing brace rather than trusting span.end directly.
    let Some(close_pos) = src_full.rfind('}') else {
        return;
    };
    let inner = &src_full[..=close_pos];
    if inner.contains("//") || inner.contains("/*") {
        return;
    }
    let close_byte = block.span.start + close_pos;
    diags.push(Diagnostic::new(
        "no-empty-block",
        Severity::Warning,
        "Avoid empty blocks — add a comment explaining the intent or remove the block",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: close_byte,
            end: close_byte + 1,
        },
    ));
}
