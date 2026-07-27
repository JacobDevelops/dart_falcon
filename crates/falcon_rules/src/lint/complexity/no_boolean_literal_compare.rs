//! Flags a comparison between a boolean value and a boolean literal, e.g. `x == true`.
//!
//! Comparing a known boolean to `true`/`false` is redundant: `x == true` is
//! just `x`, and `x == false` is `!x`. The rule flags an `==`/`!=` where one
//! side is a boolean literal and the other is provably a non-nullable boolean —
//! either syntactically (a literal, `!`, an `is` check, or a comparison/logical
//! operator) or a local or parameter whose inferred static type is a
//! non-nullable `bool`. A `bool?` operand is deliberately left alone, because
//! `x == true` is the correct null-safe way to test a nullable boolean.

use falcon_analyze::{AnalyzeContext, LocalTypes, Rule, StaticType};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_function_decl,
    walk_getter_decl, walk_method_decl, walk_setter_decl, walk_stmt,
};

pub struct NoBooleanLiteralCompare;

impl Rule for NoBooleanLiteralCompare {
    fn name(&self) -> &'static str {
        "no-boolean-literal-compare"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            locals: LocalTypes::new(),
        };
        collector.visit_program(program);
        collector
            .diags
            .sort_unstable_by_key(|diagnostic| diagnostic.span.start);
        collector
            .diags
            .dedup_by_key(|diagnostic| diagnostic.span.start);
        collector.diags
    }
}

fn is_known_bool(expr: &Expr) -> bool {
    match expr {
        Expr::BoolLit { .. } => true,
        Expr::Unary {
            op: UnaryOp::Bang, ..
        } => true,
        Expr::Is { .. } => true,
        Expr::Binary { op, .. } => matches!(
            op,
            BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::LtEq
                | BinaryOp::GtEq
                | BinaryOp::And
                | BinaryOp::Or
        ),
        _ => false,
    }
}

enum StmtAction<'a> {
    Block,
    LocalVar(&'a LocalVarDecl),
    PatternDecl(&'a PatternDeclaration),
    For(&'a ForStmt),
    IfCase(&'a IfStmt),
    TryCatch(&'a TryCatchStmt),
    LocalFunc(&'a LocalFuncDecl),
    Other,
}

fn classify_stmt(stmt: &Stmt) -> StmtAction<'_> {
    match stmt {
        Stmt::Block(_) => StmtAction::Block,
        Stmt::LocalVar(local) => StmtAction::LocalVar(local),
        Stmt::PatternDecl(pattern) => StmtAction::PatternDecl(pattern),
        Stmt::For(for_stmt) => StmtAction::For(for_stmt),
        Stmt::If(if_stmt) if matches!(if_stmt.condition, IfCondition::Case(..)) => {
            StmtAction::IfCase(if_stmt)
        }
        Stmt::TryCatch(try_catch) => StmtAction::TryCatch(try_catch),
        Stmt::LocalFunc(local) => StmtAction::LocalFunc(local),
        _ => StmtAction::Other,
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    locals: LocalTypes,
}

impl Collector<'_, '_> {
    fn with_fresh_locals(
        &mut self,
        bind: impl FnOnce(&mut LocalTypes),
        walk: impl FnOnce(&mut Self),
    ) {
        let previous = std::mem::replace(&mut self.locals, LocalTypes::new());
        bind(&mut self.locals);
        walk(self);
        self.locals = previous;
    }

    fn with_nested_locals(
        &mut self,
        bind: impl FnOnce(&mut LocalTypes),
        walk: impl FnOnce(&mut Self),
    ) {
        let previous = self.locals.clone();
        self.locals.push_scope();
        bind(&mut self.locals);
        walk(self);
        self.locals = previous;
    }

    fn check_comparison(&mut self, expr: &Expr) {
        let Expr::Binary {
            op,
            left,
            right,
            span,
        } = expr
        else {
            return;
        };
        if !matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) {
            return;
        }
        let other = if matches!(left.as_ref(), Expr::BoolLit { .. }) {
            Some(right.as_ref())
        } else if matches!(right.as_ref(), Expr::BoolLit { .. }) {
            Some(left.as_ref())
        } else {
            None
        };
        if other.is_some_and(|operand| {
            is_known_bool(operand) || self.locals.of_expr(operand).is_non_nullable_bool()
        }) {
            self.diags.push(Diagnostic::new(
                "no-boolean-literal-compare",
                Severity::Warning,
                "Avoid comparing boolean values to boolean literals",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.with_fresh_locals(
            |locals| locals.bind_params(&node.params),
            |this| walk_function_decl(this, node),
        );
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.with_fresh_locals(
            |locals| locals.bind_params(&node.params),
            |this| walk_constructor_decl(this, node),
        );
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.with_fresh_locals(
            |locals| locals.bind_params(&node.params),
            |this| walk_method_decl(this, node),
        );
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.with_fresh_locals(|_| {}, |this| walk_getter_decl(this, node));
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.with_fresh_locals(
            |locals| {
                let ty = node
                    .param_type
                    .as_ref()
                    .map(StaticType::from_dart_type)
                    .unwrap_or(StaticType::Unknown);
                locals.declare(node.param.name.clone(), ty);
            },
            |this| walk_setter_decl(this, node),
        );
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.with_fresh_locals(
                |locals| locals.bind_params(&operator.params),
                |this| walk_class_member(this, node),
            );
        } else {
            walk_class_member(self, node);
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match classify_stmt(node) {
            StmtAction::Block => {
                self.locals.push_scope();
                walk_stmt(self, node);
                self.locals.pop_scope();
            }
            StmtAction::LocalVar(local) => {
                walk_stmt(self, node);
                self.locals.declare_local(local);
            }
            StmtAction::PatternDecl(pattern) => {
                walk_stmt(self, node);
                self.locals.bind_pattern(&pattern.pattern);
            }
            StmtAction::For(for_stmt) => {
                self.locals.push_scope();
                if let Some(init) = &for_stmt.init {
                    self.locals.bind_for_init(init);
                }
                walk_stmt(self, node);
                self.locals.pop_scope();
            }
            StmtAction::IfCase(if_stmt) => {
                let IfCondition::Case(value, pattern, guard) = &if_stmt.condition else {
                    unreachable!();
                };
                self.visit_expr(value);
                self.visit_pattern(pattern);
                self.with_nested_locals(
                    |locals| locals.bind_pattern(pattern),
                    |this| {
                        if let Some(guard) = guard {
                            this.visit_expr(guard);
                        }
                        this.visit_stmt(&if_stmt.then_branch);
                    },
                );
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.visit_stmt(else_branch);
                }
            }
            StmtAction::TryCatch(try_catch) => {
                self.with_nested_locals(
                    |_| {},
                    |this| {
                        for stmt in &try_catch.body.stmts {
                            this.visit_stmt(stmt);
                        }
                    },
                );
                for catch in &try_catch.catches {
                    self.with_nested_locals(
                        |locals| locals.bind_catch(catch),
                        |this| {
                            for stmt in &catch.body.stmts {
                                this.visit_stmt(stmt);
                            }
                        },
                    );
                }
                if let Some(finally) = &try_catch.finally {
                    self.with_nested_locals(
                        |_| {},
                        |this| {
                            for stmt in &finally.stmts {
                                this.visit_stmt(stmt);
                            }
                        },
                    );
                }
            }
            StmtAction::LocalFunc(local) => {
                self.with_nested_locals(
                    |locals| locals.bind_params(&local.params),
                    |this| walk_stmt(this, node),
                );
            }
            StmtAction::Other => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        self.check_comparison(node);
        match node {
            Expr::Assign { target, value, .. } => {
                walk_expr(self, node);
                if let Expr::Ident(id) = target.as_ref() {
                    let ty = self.locals.of_expr(value);
                    self.locals.reassign(&id.name, ty);
                }
            }
            Expr::FuncExpr { params, .. } => {
                self.with_nested_locals(
                    |locals| locals.bind_params(params),
                    |this| walk_expr(this, node),
                );
            }
            _ => walk_expr(self, node),
        }
    }
}
