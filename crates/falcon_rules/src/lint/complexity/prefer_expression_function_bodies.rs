//! Flags block function bodies that consist of a single `return` of a value.
//!
//! A body like `{ return expr; }` reads better as `=> expr`. Every function
//! body is checked: top-level functions, methods, getters, operators, local
//! functions, and closures.
//!
//! ```dart
//! int get width { return _width; } // flagged
//! int get width => _width;         // ok
//! ```
//!
//! Adopted from the official `prefer_expression_function_bodies` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_expr, walk_function_decl, walk_stmt,
};

#[derive(Default)]
pub struct PreferExpressionFunctionBodies;

impl Rule for PreferExpressionFunctionBodies {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check(&mut self, body: Option<&FunctionBody>) {
        if let Some(FunctionBody::Block(block)) = body
            && let [Stmt::Return(ReturnStmt { value: Some(_), .. })] = block.stmts.as_slice()
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-expression-function-bodies",
                "Unnecessary use of a block function body.",
                block.span,
            ));
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.check(node.body.as_ref());
        walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let body = match node {
            ClassMember::Method(member) => member.body.as_ref(),
            ClassMember::Getter(member) => member.body.as_ref(),
            ClassMember::Setter(member) => member.body.as_ref(),
            ClassMember::Operator(member) => member.body.as_ref(),
            ClassMember::Constructor(member) => member.body.as_ref(),
            ClassMember::PrimaryConstructorBody(member) => member.body.as_ref(),
            ClassMember::Field(_) | ClassMember::Error(_) => None,
        };
        self.check(body);
        walk_class_member(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            self.check(Some(&function.body));
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { body, .. } = node {
            self.check(Some(body));
        }
        walk_expr(self, node);
    }
}
