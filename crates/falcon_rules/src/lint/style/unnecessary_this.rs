//! Flags a redundant `this.` qualifier on member access.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, bound_names, walk_expr, walk_pattern, walk_stmt};

pub struct UnnecessaryThis;

impl Rule for UnnecessaryThis {
    fn name(&self) -> &'static str {
        "unnecessary-this"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for declaration in &program.declarations {
            let Some(members) = members_of(declaration) else {
                continue;
            };
            for member in members {
                check_member(member, ctx, &mut diagnostics);
            }
        }
        diagnostics
    }
}

fn members_of(declaration: &TopLevelDecl) -> Option<&[ClassMember]> {
    match declaration {
        TopLevelDecl::Class(class) => Some(&class.members),
        TopLevelDecl::Mixin(mixin) => Some(&mixin.members),
        TopLevelDecl::MixinClass(class) => Some(&class.members),
        TopLevelDecl::Enum(enumeration) => Some(&enumeration.members),
        TopLevelDecl::Extension(extension) => Some(&extension.members),
        TopLevelDecl::ExtensionType(extension) => Some(&extension.members),
        _ => None,
    }
}

fn check_member(member: &ClassMember, ctx: &AnalyzeContext, diagnostics: &mut Vec<Diagnostic>) {
    let (params, body) = match member {
        ClassMember::Method(method) if !method.is_static => (&method.params, method.body.as_ref()),
        ClassMember::Getter(getter) if !getter.is_static => {
            return check_body(None, getter.body.as_ref(), ctx, diagnostics);
        }
        ClassMember::Setter(setter) if !setter.is_static => {
            let mut collector = Collector::new(ctx, diagnostics);
            collector.declare(&setter.param.name);
            if let Some(body) = &setter.body {
                collector.body(body);
            }
            return;
        }
        ClassMember::Operator(operator) => (&operator.params, operator.body.as_ref()),
        // Constructor initializers deliberately keep `this.` for disambiguation.
        ClassMember::Constructor(constructor) => (&constructor.params, constructor.body.as_ref()),
        _ => return,
    };
    check_body(Some(params), body, ctx, diagnostics);
}

fn check_body(
    params: Option<&FormalParamList>,
    body: Option<&FunctionBody>,
    ctx: &AnalyzeContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(body) = body else {
        return;
    };
    let mut collector = Collector::new(ctx, diagnostics);
    if let Some(params) = params {
        collector.params(params);
    }
    collector.body(body);
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: &'a mut Vec<Diagnostic>,
    scopes: Vec<HashSet<String>>,
}

impl<'a, 'ctx> Collector<'a, 'ctx> {
    fn new(ctx: &'a AnalyzeContext<'ctx>, diagnostics: &'a mut Vec<Diagnostic>) -> Self {
        Self {
            ctx,
            diagnostics,
            scopes: vec![HashSet::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str) {
        self.scopes
            .last_mut()
            .expect("lexical scope")
            .insert(name.to_string());
    }

    fn bound(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn params(&mut self, params: &FormalParamList) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            self.declare(&param.name.name);
        }
    }

    fn pattern(&mut self, pattern: &Pattern) {
        walk_pattern(self, pattern);
        for name in bound_names(pattern) {
            self.declare(&name.name);
        }
    }

    fn body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(block) => {
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
            }
            FunctionBody::Arrow(expression, _) => self.visit_expr(expression),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn nested_function(&mut self, params: &FormalParamList, body: &FunctionBody) {
        self.push();
        self.params(params);
        self.body(body);
        self.pop();
    }

    fn local(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
            self.declare(&declarator.name.name);
        }
    }

    fn for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.local(declaration),
            ForInit::ForIn { name, iterable, .. } => {
                self.visit_expr(iterable);
                self.declare(&name.name);
            }
            ForInit::PatternForIn { pattern, iterable } => {
                self.visit_expr(iterable);
                self.pattern(pattern);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => {
                self.push();
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
                self.pop();
            }
            Stmt::LocalVar(declaration) => self.local(declaration),
            Stmt::LocalFunc(function) => {
                self.declare(&function.name.name);
                self.nested_function(&function.params, &function.body);
            }
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.pattern(&declaration.pattern);
            }
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => {
                    self.visit_expr(condition);
                    self.push();
                    self.visit_stmt(&statement.then_branch);
                    self.pop();
                    if let Some(branch) = &statement.else_branch {
                        self.push();
                        self.visit_stmt(branch);
                        self.pop();
                    }
                }
                IfCondition::Case(value, pattern, guard) => {
                    self.visit_expr(value);
                    self.push();
                    self.pattern(pattern);
                    if let Some(guard) = guard {
                        self.visit_expr(guard);
                    }
                    self.visit_stmt(&statement.then_branch);
                    self.pop();
                    if let Some(branch) = &statement.else_branch {
                        self.push();
                        self.visit_stmt(branch);
                        self.pop();
                    }
                }
            },
            Stmt::For(statement) => {
                self.push();
                if let Some(init) = &statement.init {
                    self.for_init(init);
                }
                if let Some(condition) = &statement.condition {
                    self.visit_expr(condition);
                }
                for update in &statement.update {
                    self.visit_expr(update);
                }
                self.visit_stmt(&statement.body);
                self.pop();
            }
            Stmt::While(statement) => {
                self.visit_expr(&statement.condition);
                self.push();
                self.visit_stmt(&statement.body);
                self.pop();
            }
            Stmt::DoWhile(statement) => {
                self.push();
                self.visit_stmt(&statement.body);
                self.visit_expr(&statement.condition);
                self.pop();
            }
            Stmt::Switch(statement) => {
                self.visit_expr(&statement.subject);
                for case in &statement.cases {
                    self.push();
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                            self.pattern(pattern);
                            if let Some(guard) = guard.as_ref() {
                                self.visit_expr(guard);
                            }
                        }
                    }
                    for statement in &case.body {
                        self.visit_stmt(statement);
                    }
                    self.pop();
                }
            }
            Stmt::TryCatch(statement) => {
                self.push();
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
                self.pop();
                for catch in &statement.catches {
                    self.push();
                    if let Some(name) = &catch.exception_var {
                        self.declare(&name.name);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.declare(&name.name);
                    }
                    for statement in &catch.body.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop();
                }
                if let Some(finally) = &statement.finally {
                    self.push();
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop();
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Field { object, field, .. } = node
            && matches!(object.as_ref(), Expr::This { .. })
            && !self.bound(&field.name)
        {
            let span = object.span();
            self.diagnostics.push(Diagnostic::new(
                "unnecessary-this",
                Severity::Warning,
                "Unnecessary 'this.' qualifier.",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        if let Expr::FuncExpr { params, body, .. } = node {
            self.nested_function(params, body);
        } else {
            walk_expr(self, node);
        }
    }
}
