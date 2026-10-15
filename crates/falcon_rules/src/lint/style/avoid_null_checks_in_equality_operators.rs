//! Flags null checks of the parameter inside a custom `operator ==`.
//!
//! Dart never calls a user-defined `==` with `null`: `a == null` is answered by
//! the language before the operator runs. Declaring the parameter as nullable
//! (`Object? other`) and then comparing it with `null`, applying `??`, or using
//! `?.` on it is dead code.
//!
//! ```dart
//! class Person {
//!   final String? name;
//!   @override
//!   bool operator ==(Object? other) =>
//!       other != null && other is Person && name == other.name; // flagged
//! }
//! ```
//!
//! Only operators whose single parameter is explicitly declared nullable are
//! inspected; `dynamic` is excluded, matching the analyzer's nullability suffix
//! check. An operator body that rebinds the parameter name is skipped.
//!
//! Adopted from the official `avoid_null_checks_in_equality_operators` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body, walk_pattern, walk_stmt};

const MESSAGE: &str = "Unnecessary null comparison in implementation of '=='.";

#[derive(Default)]
pub struct AvoidNullChecksInEqualityOperators;

impl Rule for AvoidNullChecksInEqualityOperators {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                continue;
            };
            for member in members {
                if let ClassMember::Operator(op) = member {
                    check_operator(op, ctx, &mut diags);
                }
            }
        }
        diags
    }
}

fn check_operator(op: &OperatorDecl, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let (Some(body), [param]) = (&op.body, op.params.params.as_slice()) else {
        return;
    };
    let nullable_param = param.param_type.as_ref().is_some_and(DartType::is_nullable);
    if op.op != "==" || !nullable_param {
        return;
    }
    let name = param.name.name.as_str();
    let mut rebinds = Rebinds { name, found: false };
    walk_function_body(&mut rebinds, body);
    if rebinds.found {
        return;
    }
    let mut finder = NullChecks {
        name,
        spans: Vec::new(),
    };
    walk_function_body(&mut finder, body);
    diags.extend(
        finder
            .spans
            .into_iter()
            .map(|span| ctx.diagnostic("avoid-null-checks-in-equality-operators", MESSAGE, span)),
    );
}

fn is_param(expr: &Expr, name: &str) -> bool {
    matches!(expr.unparenthesized(), Expr::Ident(identifier) if identifier.name == name)
}

struct NullChecks<'a> {
    name: &'a str,
    spans: Vec<Span>,
}

impl Visitor for NullChecks<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        let flagged = match node {
            Expr::Binary {
                op: BinaryOp::EqEq | BinaryOp::NotEq,
                left,
                right,
                ..
            } => {
                (matches!((**left).unparenthesized(), Expr::NullLit { .. })
                    && is_param(right, self.name))
                    || (matches!((**right).unparenthesized(), Expr::NullLit { .. })
                        && is_param(left, self.name))
            }
            Expr::Binary {
                op: BinaryOp::NullCoalesce,
                left,
                ..
            } => is_param(left, self.name),
            // Covers both `other?.property` and `other?.method()`.
            Expr::Field {
                object,
                is_null_safe: true,
                ..
            } => is_param(object, self.name),
            _ => false,
        };
        if flagged {
            self.spans.push(*node.span());
        }
        walk_expr(self, node);
    }
}

/// Whether the body declares anything with the parameter's name, which would
/// make a later reference ambiguous without scope tracking.
struct Rebinds<'a> {
    name: &'a str,
    found: bool,
}

impl Rebinds<'_> {
    fn check(&mut self, identifier: &Identifier) {
        self.found |= identifier.name == self.name;
    }

    fn check_params(&mut self, params: &FormalParamList) {
        for param in &params.params {
            self.check(&param.name);
        }
    }
}

impl Visitor for Rebinds<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(declaration) => {
                for declarator in &declaration.declarators {
                    self.check(&declarator.name);
                }
            }
            Stmt::LocalFunc(function) => {
                self.check(&function.name);
                self.check_params(&function.params);
            }
            Stmt::For(ForStmt {
                header: ForHeader {
                    init: Some(init), ..
                },
                ..
            }) => match init {
                ForInit::VarDecl(declaration) => {
                    for declarator in &declaration.declarators {
                        self.check(&declarator.name);
                    }
                }
                ForInit::ForIn { name, .. } => self.check(name),
                ForInit::PatternForIn { .. } | ForInit::Exprs(_) => {}
            },
            Stmt::TryCatch(statement) => {
                for clause in &statement.catches {
                    for variable in [&clause.exception_var, &clause.stack_trace_var]
                        .into_iter()
                        .flatten()
                    {
                        self.check(variable);
                    }
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { params, .. } = node {
            self.check_params(params);
        }
        walk_expr(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, .. } = node {
            self.check(name);
        }
        walk_pattern(self, node);
    }
}
