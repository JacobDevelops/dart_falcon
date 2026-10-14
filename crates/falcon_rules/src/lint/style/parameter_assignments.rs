//! Don't reassign parameters of functions, methods, or constructors.
//!
//! Rebinding a parameter makes the body harder to follow: the name no longer
//! means what the caller passed. Assignments (`x = ...`, `x += ...`),
//! increments and decrements, and pattern assignments (`(x, y) = ...`) that
//! write a parameter are flagged, including writes from closures inside the
//! body. One exception mirrors the analyzer: an optional parameter without a
//! default value (so it defaults to `null`) may be given a default once with
//! `x ??= value`; a second `??=` or any other assignment is still flagged.
//! The analyzer also exempts increments/decrements of optional parameters
//! without defaults, and this rule retains that exception.
//!
//! ```dart
//! void f(int x) { x = 2; } // flagged
//! void g([int? x]) { x ??= 2; } // ok
//! ```
//!
//! Adopted from the official `parameter_assignments` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_function_decl,
    walk_method_decl, walk_setter_decl, walk_stmt,
};

use crate::lint::lexical_usage::{LocalWrite, LocalWriteKind, local_writes};

const RULE: &str = "parameter-assignments";

#[derive(Default)]
pub struct ParameterAssignments;

impl Rule for ParameterAssignments {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut writes: HashMap<usize, Vec<LocalWrite>> = HashMap::new();
        for write in local_writes(program, ctx.source) {
            writes.entry(write.declaration).or_default().push(write);
        }
        if writes.is_empty() {
            return Vec::new();
        }
        let mut collector = Collector {
            ctx,
            writes,
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    writes: HashMap<usize, Vec<LocalWrite>>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check_param(&mut self, name: &Identifier, defaults_to_null: bool) {
        let Some(writes) = self.writes.remove(&name.span.start) else {
            return;
        };
        let mut defaulted = false;
        for write in writes {
            let report = match write.kind {
                LocalWriteKind::Assign(AssignOp::NullCoalesceEq) if defaults_to_null => {
                    std::mem::replace(&mut defaulted, true)
                }
                LocalWriteKind::Assign(_) => true,
                LocalWriteKind::IncDec => !defaults_to_null,
                LocalWriteKind::Pattern { direct } => direct,
                // The analyzer's rule does not visit for-in loop variables.
                LocalWriteKind::ForIn => false,
            };
            if report {
                self.diagnostics.push(self.ctx.diagnostic(
                    RULE,
                    format!("Invalid assignment to the parameter '{}'.", name.name),
                    write.span,
                ));
            }
        }
    }

    fn check(&mut self, params: &FormalParamList) {
        for param in &params.params {
            let optional = match param.kind {
                ParamKind::Positional | ParamKind::Named { required: true } => false,
                ParamKind::OptionalPositional | ParamKind::Named { required: false } => true,
            };
            self.check_param(&param.name, optional && param.default_value.is_none());
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if let Some(params) = node.params() {
            self.check(params);
        }
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check(&node.params);
        walk_method_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.check_param(&node.param, false);
        walk_setter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check(&operator.params);
        }
        walk_class_member(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.check(&node.params);
        walk_constructor_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            self.check(&function.params);
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { params, .. } = node {
            self.check(params);
        }
        walk_expr(self, node);
    }
}
