//! Flags mutable global state: non-`const`, non-`final` top-level variables and
//! static fields, plus every read of mutable top-level variables.
//!
//! Mutable top-level and static members are shared, unscoped state that any
//! code can read or write, which makes data flow hard to follow and tests hard
//! to isolate. Prefer `const` or `final` values, or pass state explicitly
//! through constructors and parameters.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, bound_names, walk_class_member, walk_constructor_decl, walk_expr, walk_field_decl,
    walk_function_decl, walk_getter_decl, walk_method_decl, walk_setter_decl, walk_stmt,
    walk_top_level_decl,
};

#[derive(Default)]
pub struct AvoidTopLevelMemberAccess;

impl Rule for AvoidTopLevelMemberAccess {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let top_level_vars = program
            .declarations
            .iter()
            .filter_map(|decl| match decl {
                TopLevelDecl::Variable(var)
                    if var.keyword == falcon_syntax::ast::VarKeyword::Var =>
                {
                    Some(var)
                }
                _ => None,
            })
            .flat_map(|var| var.declarators.iter().map(|decl| decl.name.name.clone()))
            .collect();
        let mut scopes = ScopeCollector::default();
        scopes.visit_program(program);

        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            top_level_vars,
            bindings: scopes.bindings,
        };
        collector.visit_program(program);
        collector.diags
    }
}

#[derive(Default)]
struct ScopeCollector {
    bindings: HashMap<String, Vec<Span>>,
    block_ends: Vec<usize>,
    suppress_next_local_var_binding: bool,
}

impl ScopeCollector {
    fn bind(&mut self, name: &Identifier, span: &Span) {
        self.bindings
            .entry(name.name.clone())
            .or_default()
            .push(*span);
    }

    fn bind_params(&mut self, params: &FormalParamList, span: &Span) {
        for param in params.params.iter() {
            self.bind(&param.name, span);
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern, span: &Span) {
        for name in bound_names(pattern) {
            self.bind(name, span);
        }
    }

    fn current_tail(&self, start: usize) -> Option<Span> {
        self.block_ends.last().map(|end| Span::new(start, *end))
    }

    fn bind_local_functions(&mut self, block: &Block) {
        self.bind_local_functions_in(&block.stmts, &block.span);
    }

    fn bind_local_functions_in(&mut self, stmts: &[Stmt], span: &Span) {
        for stmt in stmts {
            if let Stmt::LocalFunc(local) = stmt {
                self.bind(&local.name, span);
            }
        }
    }

    fn with_statement_list(&mut self, stmts: &[Stmt], span: &Span) {
        self.bind_local_functions_in(stmts, span);
        self.block_ends.push(span.end);
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
        self.block_ends.pop();
    }

    fn with_body(&mut self, body: &FunctionBody, walk: impl FnOnce(&mut Self)) {
        if let FunctionBody::Block(block) = body {
            self.bind_local_functions(block);
            self.block_ends.push(block.span.end);
            walk(self);
            self.block_ends.pop();
        } else {
            walk(self);
        }
    }
    fn bind_for_init(&mut self, init: &ForInit, statement: &ForStmt) {
        match init {
            ForInit::VarDecl(declaration) => {
                for declarator in &declaration.declarators {
                    let scope = Span::new(declarator.name.span.end, statement.span.end);
                    self.bind(&declarator.name, &scope);
                }
                self.suppress_next_local_var_binding = true;
            }
            ForInit::ForIn { name, .. } => self.bind(name, statement.body.span()),
            ForInit::PatternForIn { pattern, .. } => {
                self.bind_pattern(pattern, statement.body.span());
            }
            ForInit::Exprs(_) => {}
        }
    }
}

/// Statement kinds that introduce bindings. `Block`, `Switch`, and `TryCatch` are
/// absent: `visit_stmt` handles those before it classifies.
enum StmtScope<'a> {
    PatternDecl(&'a PatternDeclaration),
    For(&'a ForStmt),
    IfCase(&'a IfStmt, &'a Pattern),
    LocalFunc(&'a LocalFuncDecl),
    Other,
}

fn classify_stmt_scope(stmt: &Stmt) -> StmtScope<'_> {
    match stmt {
        Stmt::PatternDecl(pattern) => StmtScope::PatternDecl(pattern),
        Stmt::For(for_stmt) => StmtScope::For(for_stmt),
        Stmt::If(if_stmt) => match &if_stmt.condition {
            IfCondition::Case(_, pattern, _) => StmtScope::IfCase(if_stmt, pattern),
            IfCondition::Expr(_) => StmtScope::Other,
        },
        Stmt::LocalFunc(local) => StmtScope::LocalFunc(local),
        _ => StmtScope::Other,
    }
}

impl Visitor for ScopeCollector {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if let Some(body) = &node.body {
            if let Some(params) = node.params() {
                self.bind_params(params, body.span());
            }
            self.with_body(body, |this| walk_function_decl(this, node));
        } else {
            walk_function_decl(self, node);
        }
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let scope = Span::new(node.params.span.end, node.span.end);
        self.bind_params(&node.params, &scope);
        if let Some(body) = &node.body {
            self.with_body(body, |this| walk_constructor_decl(this, node));
        } else {
            walk_constructor_decl(self, node);
        }
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        if let Some(body) = &node.body {
            self.bind_params(&node.params, body.span());
            self.with_body(body, |this| walk_method_decl(this, node));
        } else {
            walk_method_decl(self, node);
        }
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        if let Some(body) = &node.body {
            self.with_body(body, |this| walk_getter_decl(this, node));
        } else {
            walk_getter_decl(self, node);
        }
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        if let Some(body) = &node.body {
            self.bind(&node.param, body.span());
            self.with_body(body, |this| walk_setter_decl(this, node));
        } else {
            walk_setter_decl(self, node);
        }
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node
            && let Some(body) = &operator.body
        {
            self.bind_params(&operator.params, body.span());
            self.with_body(body, |this| walk_class_member(this, node));
        } else {
            walk_class_member(self, node);
        }
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        if !std::mem::take(&mut self.suppress_next_local_var_binding) {
            for declaration in &node.declarators {
                if let Some(scope) = self.current_tail(declaration.name.span.end) {
                    self.bind(&declaration.name, &scope);
                }
            }
        }
        falcon_syntax::visitor::walk_local_var(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => {
                self.with_statement_list(&block.stmts, &block.span);
                return;
            }
            Stmt::Switch(switch) => {
                self.visit_expr(&switch.subject);
                for case in &switch.cases {
                    for label in &case.labels {
                        self.visit_identifier(label);
                    }
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                            self.visit_pattern(pattern);
                            if let Some(guard) = guard {
                                self.visit_expr(guard);
                            }
                            let scope = Span::new(pattern.span().end, case.span.end);
                            self.bind_pattern(pattern, &scope);
                        }
                    }
                    self.with_statement_list(&case.body, &case.span);
                }
                return;
            }
            Stmt::TryCatch(try_catch) => {
                self.with_statement_list(&try_catch.body.stmts, &try_catch.body.span);
                for catch in &try_catch.catches {
                    if let Some(name) = &catch.exception_var {
                        self.bind(name, &catch.body.span);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.bind(name, &catch.body.span);
                    }
                    self.with_statement_list(&catch.body.stmts, &catch.body.span);
                }
                if let Some(finally) = &try_catch.finally {
                    self.with_statement_list(&finally.stmts, &finally.span);
                }
                return;
            }
            _ => {}
        }

        let mut pushed_end = None;
        match classify_stmt_scope(node) {
            StmtScope::PatternDecl(decl) => {
                if let Some(scope) = self.current_tail(decl.span.end) {
                    self.bind_pattern(&decl.pattern, &scope);
                }
            }
            StmtScope::For(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    self.bind_for_init(init, for_stmt);
                }
            }
            StmtScope::IfCase(if_stmt, pattern) => {
                let scope = Span::new(pattern.span().end, if_stmt.then_branch.span().end);
                self.bind_pattern(pattern, &scope);
            }
            StmtScope::LocalFunc(local) => {
                self.bind_params(&local.params, local.body.span());
                if let FunctionBody::Block(block) = &local.body {
                    self.bind_local_functions(block);
                    self.block_ends.push(block.span.end);
                    pushed_end = Some(block.span.end);
                }
            }
            StmtScope::Other => {}
        }
        walk_stmt(self, node);
        if pushed_end.is_some() {
            self.block_ends.pop();
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { params, body, .. } = node {
            self.bind_params(params, body.span());
            self.with_body(body, |this| walk_expr(this, node));
        } else {
            walk_expr(self, node);
        }
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    top_level_vars: HashSet<String>,
    bindings: HashMap<String, Vec<Span>>,
}

impl Collector<'_, '_> {
    fn is_bound(&self, name: &str, offset: usize) -> bool {
        self.bindings.get(name).is_some_and(|scopes| {
            scopes
                .iter()
                .any(|scope| scope.start <= offset && offset < scope.end)
        })
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(var) = node {
            if var.keyword == falcon_syntax::ast::VarKeyword::Var {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-top-level-member-access",
                    "Avoid using non-const top-level members",
                    var.span,
                ));
            }
            // A top-level initializer is part of the declaration itself, not an
            // access from executable code.
            return;
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.is_static
            && node.keyword != falcon_syntax::ast::VarKeyword::Final
            && node.keyword != falcon_syntax::ast::VarKeyword::Const
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-top-level-member-access",
                "Avoid using non-const top-level members",
                node.span,
            ));
        }
        walk_field_decl(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Ident(id) = node
            && self.top_level_vars.contains(&id.name)
            && !self.is_bound(&id.name, id.span.start)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-top-level-member-access",
                "Avoid using non-const top-level members",
                id.span,
            ));
        }
        walk_expr(self, node);
    }
}
