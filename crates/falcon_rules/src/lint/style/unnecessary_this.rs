//! Flags a redundant `this.` qualifier on member access.

use crate::lint::lexical_usage::NameScopes;
use falcon_analyze::{AnalyzeContext, DocName, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_block, walk_expr, walk_function_body, walk_pattern, walk_stmt,
};

#[derive(Default)]
pub struct UnnecessaryThis;

impl Rule for UnnecessaryThis {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let model = ctx
            .identities
            .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types));
        for declaration in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(declaration) else {
                continue;
            };
            let mut names = crate::lint::declarations::member_names(members);
            let effective = match declaration {
                TopLevelDecl::Class(node) => node.element_members(),
                TopLevelDecl::MixinClass(node) => node.element_members(),
                TopLevelDecl::Enum(node) => node.element_members(),
                TopLevelDecl::ExtensionType(node) => node.element_members(),
                _ => EffectiveMembers::declared(members),
            };
            names.extend(
                effective
                    .declaring_params()
                    .map(|param| param.name.name.clone()),
            );
            // Direct members stop lookup before the library; inherited and
            // extension-provided members can still be shadowed there.
            let library_shadows = |name: &str| {
                !names.contains(name) && library_contains_name(program, model.as_ref(), name)
            };
            for member in members {
                check_member(member, ctx, &library_shadows, &mut diagnostics);
            }
        }
        diagnostics
    }
}

fn library_contains_name(program: &Program, model: Option<&SemanticModel<'_>>, name: &str) -> bool {
    if program.imports.iter().any(|import| {
        import
            .as_name
            .as_ref()
            .is_some_and(|prefix| prefix.name == name)
    }) {
        return true;
    }
    if let Some(model) = model {
        return matches!(
            model.resolve_library_name(name),
            DocName::Declaration { .. } | DocName::Prefix | DocName::Extension
        );
    }
    program
        .declarations
        .iter()
        .any(|declaration| match declaration {
            TopLevelDecl::Function(function) => function.name.name == name,
            TopLevelDecl::Variable(variable) => variable
                .declarators
                .iter()
                .any(|declarator| declarator.name.name == name),
            _ => false,
        })
}

fn check_member(
    member: &ClassMember,
    ctx: &AnalyzeContext,
    library_shadows: &dyn Fn(&str) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (params, body) = match member {
        ClassMember::Method(method) if !method.is_static => (&method.params, method.body.as_ref()),
        ClassMember::Getter(getter) if !getter.is_static => {
            return check_body(
                None,
                getter.body.as_ref(),
                ctx,
                library_shadows,
                diagnostics,
            );
        }
        ClassMember::Setter(setter) if !setter.is_static => {
            let mut collector = Collector::new(ctx, library_shadows, diagnostics);
            collector.scopes.declare(&setter.param.name);
            if let Some(body) = &setter.body {
                walk_function_body(&mut collector, body);
            }
            return;
        }
        ClassMember::Operator(operator) => (&operator.params, operator.body.as_ref()),
        // Constructor initializers deliberately keep `this.` for disambiguation.
        ClassMember::Constructor(constructor) => (&constructor.params, constructor.body.as_ref()),
        _ => return,
    };
    check_body(Some(params), body, ctx, library_shadows, diagnostics);
}

fn check_body(
    params: Option<&FormalParamList>,
    body: Option<&FunctionBody>,
    ctx: &AnalyzeContext,
    library_shadows: &dyn Fn(&str) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(body) = body else {
        return;
    };
    let mut collector = Collector::new(ctx, library_shadows, diagnostics);
    if let Some(params) = params {
        collector.scopes.params(params);
    }
    walk_function_body(&mut collector, body);
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: &'a mut Vec<Diagnostic>,
    scopes: NameScopes,
    library_shadows: &'a dyn Fn(&str) -> bool,
}

impl<'a, 'ctx> Collector<'a, 'ctx> {
    fn new(
        ctx: &'a AnalyzeContext<'ctx>,
        library_shadows: &'a dyn Fn(&str) -> bool,
        diagnostics: &'a mut Vec<Diagnostic>,
    ) -> Self {
        Self {
            ctx,
            diagnostics,
            scopes: NameScopes::default(),
            library_shadows,
        }
    }

    fn pattern(&mut self, pattern: &Pattern) {
        walk_pattern(self, pattern);
        self.scopes.pattern(pattern);
    }

    fn nested_function(&mut self, params: &FormalParamList, body: &FunctionBody) {
        self.scopes.push();
        self.scopes.params(params);
        walk_function_body(self, body);
        self.scopes.pop();
    }

    fn scoped_stmt(&mut self, statement: &Stmt) {
        self.scopes.push();
        self.visit_stmt(statement);
        self.scopes.pop();
    }

    fn local(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            self.scopes.declare(&declarator.name.name);
        }
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
        }
    }

    fn prebind(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::LocalVar(declaration) => {
                    for declarator in &declaration.declarators {
                        self.scopes.declare(&declarator.name.name);
                    }
                }
                Stmt::LocalFunc(function) => self.scopes.declare(&function.name.name),
                Stmt::PatternDecl(declaration) => self.scopes.pattern(&declaration.pattern),
                _ => {}
            }
        }
    }

    fn for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.local(declaration),
            ForInit::ForIn {
                declares_variable,
                name,
                iterable,
                ..
            } => {
                if *declares_variable {
                    self.scopes.declare(&name.name);
                }
                self.visit_expr(iterable);
            }
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
                self.pattern(pattern);
                self.visit_expr(iterable);
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
    fn visit_block(&mut self, node: &Block) {
        // Dart's block scope includes declarations following the reference,
        // including references in the declaration's own initializer.
        self.prebind(&node.stmts);
        walk_block(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.local(node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(_) => {
                self.scopes.push();
                walk_stmt(self, node);
                self.scopes.pop();
            }
            Stmt::LocalVar(declaration) => self.visit_local_var(declaration),
            Stmt::LocalFunc(function) => {
                self.scopes.declare(&function.name.name);
                self.nested_function(&function.params, &function.body);
            }
            Stmt::PatternDecl(declaration) => {
                self.scopes.pattern(&declaration.pattern);
                self.visit_expr(&declaration.init);
                walk_pattern(self, &declaration.pattern);
            }
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => {
                    self.visit_expr(condition);
                    self.scoped_stmt(&statement.then_branch);
                    if let Some(branch) = &statement.else_branch {
                        self.scoped_stmt(branch);
                    }
                }
                IfCondition::Case(value, pattern, guard) => {
                    self.visit_expr(value);
                    self.scopes.push();
                    self.pattern(pattern);
                    if let Some(guard) = guard {
                        self.visit_expr(guard);
                    }
                    self.visit_stmt(&statement.then_branch);
                    self.scopes.pop();
                    if let Some(branch) = &statement.else_branch {
                        self.scoped_stmt(branch);
                    }
                }
            },
            Stmt::For(statement) => {
                self.scopes.push();
                if let Some(init) = &statement.header.init {
                    self.for_init(init);
                }
                if let Some(condition) = &statement.header.condition {
                    self.visit_expr(condition);
                }
                for update in &statement.header.updates {
                    self.visit_expr(update);
                }
                self.visit_stmt(&statement.body);
                self.scopes.pop();
            }
            Stmt::While(statement) => {
                self.visit_expr(&statement.condition);
                self.scoped_stmt(&statement.body);
            }
            Stmt::DoWhile(statement) => {
                self.scopes.push();
                self.visit_stmt(&statement.body);
                self.visit_expr(&statement.condition);
                self.scopes.pop();
            }
            Stmt::Switch(statement) => {
                self.visit_expr(&statement.subject);
                for case in &statement.cases {
                    self.scopes.push();
                    self.prebind(&case.body);
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                            self.pattern(pattern);
                            if let Some(guard) = guard.as_ref() {
                                self.visit_expr(guard);
                            }
                        } else if let SwitchCaseKind::Expr(expression) = kind {
                            self.visit_expr(expression);
                        }
                    }
                    for statement in &case.body {
                        self.visit_stmt(statement);
                    }
                    self.scopes.pop();
                }
            }
            Stmt::TryCatch(statement) => {
                self.scopes.push();
                self.visit_block(&statement.body);
                self.scopes.pop();
                for catch in &statement.catches {
                    self.scopes.push();
                    if let Some(name) = &catch.exception_var {
                        self.scopes.declare(&name.name);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.scopes.declare(&name.name);
                    }
                    self.visit_block(&catch.body);
                    self.scopes.pop();
                }
                if let Some(finally) = &statement.finally {
                    self.scopes.push();
                    self.visit_block(finally);
                    self.scopes.pop();
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Field { object, field, .. } = node
            && matches!(object.unparenthesized(), Expr::This { .. })
            && !self.scopes.bound(&field.name)
            && !(self.library_shadows)(&field.name)
        {
            let span = object.span();
            self.diagnostics.push(self.ctx.diagnostic(
                "unnecessary-this",
                "Unnecessary 'this.' qualifier.",
                *span,
            ));
        }
        if let Expr::FuncExpr { params, body, .. } = node {
            self.nested_function(params, body);
        } else {
            walk_expr(self, node);
        }
    }
}
