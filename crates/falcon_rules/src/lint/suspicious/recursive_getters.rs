//! Flags a getter that unconditionally returns itself.
//!
//! A body of `=> x` (or `=> this.x`, or `return x;`) inside `get x` calls the
//! getter again with no base case, recursing until the stack overflows at
//! runtime. It almost always means a backing field was intended — typically an
//! underscore-prefixed `_x` — but was misspelled as the getter's own name.
//! Return the backing field instead. Only a direct, unconditional self-reference
//! is reported; a getter that merely mentions its own name inside a larger
//! expression is left alone.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct RecursiveGetters;

impl Rule for RecursiveGetters {
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
    // Getter declarations only occur at the top level or directly in a member
    // container. Bodies, expressions and types cannot introduce another getter.
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Function(function) = node {
            self.visit_function_decl(function);
        } else if let Some(members) = crate::lint::declarations::members_of(node) {
            for member in members {
                if let ClassMember::Getter(getter) = member {
                    self.visit_getter_decl(getter);
                }
            }
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if matches!(node.kind, FunctionKind::Getter) {
            check(&node.body, &node.name.name, &mut self.diags, self.ctx);
        }
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        check(&node.body, &node.name.name, &mut self.diags, self.ctx);
    }
}

fn check(
    body: &Option<FunctionBody>,
    name: &str,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let expr = match body.as_ref() {
        Some(FunctionBody::Arrow(e, _)) => Some(&**e),
        Some(FunctionBody::Block(b)) => match b.stmts.as_slice() {
            [Stmt::Return(r)] => r.value.as_ref(),
            _ => None,
        },
        _ => None,
    };
    let Some(expr) = expr else { return };
    if let Some(span) = self_reference(expr, name) {
        diags.push(ctx.diagnostic(
            "recursive-getters",
            "Recursive getter — this getter unconditionally returns itself",
            *span,
        ));
    }
}

/// The span of a direct self-reference (`x` or `this.x`), if `expr` is one.
fn self_reference<'a>(expr: &'a Expr, name: &str) -> Option<&'a Span> {
    match expr {
        Expr::Ident(id) if id.name == name => Some(&id.span),
        Expr::Field {
            object,
            field,
            span,
            ..
        } if matches!(&**object, Expr::This { .. }) && field.name == name => Some(span),
        _ => None,
    }
}
