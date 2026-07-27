//! Don't use constant patterns with type literals.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, IdentityIndex, LocalTypes, NameIdentity, Rule, StaticType};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_decl, walk_class_member, walk_enum_decl, walk_expr, walk_extension_decl,
    walk_mixin_decl, walk_pattern, walk_program, walk_stmt, walk_top_level_decl,
};

pub struct TypeLiteralInConstantPattern;

impl Rule for TypeLiteralInConstantPattern {
    fn name(&self) -> &'static str {
        "type-literal-in-constant-pattern"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            file_path: ctx.file_path,
            identities,
            locals: LocalTypes::new(),
            enclosing_members: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    file: String,
    file_path: &'a std::path::Path,
    identities: &'a IdentityIndex,
    locals: LocalTypes,
    enclosing_members: Vec<HashSet<String>>,
}

enum StmtAction<'a> {
    LocalVar(&'a LocalVarDecl),
    PatternDecl(&'a PatternDeclaration),
    Block,
    IfCase(&'a IfStmt),
    For(&'a ForStmt),
    Switch(&'a SwitchStmt),
    TryCatch(&'a TryCatchStmt),
    LocalFunc(&'a LocalFuncDecl),
    Other,
}

fn classify_stmt(stmt: &Stmt) -> StmtAction<'_> {
    match stmt {
        Stmt::LocalVar(declaration) => StmtAction::LocalVar(declaration),
        Stmt::PatternDecl(declaration) => StmtAction::PatternDecl(declaration),
        Stmt::Block(_) => StmtAction::Block,
        Stmt::If(statement) if matches!(statement.condition, IfCondition::Case(..)) => {
            StmtAction::IfCase(statement)
        }
        Stmt::For(statement) => StmtAction::For(statement),
        Stmt::Switch(statement) => StmtAction::Switch(statement),
        Stmt::TryCatch(statement) => StmtAction::TryCatch(statement),
        Stmt::LocalFunc(function) => StmtAction::LocalFunc(function),
        _ => StmtAction::Other,
    }
}

impl Collector<'_> {
    fn check_pattern(&mut self, pattern: &Pattern) {
        let Pattern::Const(constant) = pattern else {
            return;
        };
        if constant.expr.is_some() || constant.name.is_empty() {
            return;
        }
        let head = &constant.name[0].name;
        if self.locals.is_bound(head)
            || self
                .enclosing_members
                .last()
                .is_some_and(|members| members.contains(head))
        {
            return;
        }
        let segments: Vec<String> = constant
            .name
            .iter()
            .map(|identifier| identifier.name.clone())
            .collect();
        if self.identities.resolve(self.file_path, &segments) != NameIdentity::Type {
            return;
        }
        let span = &constant.name.last().expect("non-empty constant name").span;
        self.diags.push(Diagnostic::new(
            "type-literal-in-constant-pattern",
            Severity::Warning,
            "Use a type pattern or wrap the type literal in 'const (...)'.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn with_members(&mut self, members: &[ClassMember], visit: impl FnOnce(&mut Self)) {
        self.enclosing_members.push(member_names(members));
        visit(self);
        self.enclosing_members.pop();
    }

    fn body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(block) => self.statements(&block.stmts),
            FunctionBody::Arrow(expr, _) => self.visit_expr(expr),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            self.visit_stmt(statement);
        }
    }

    fn visit_params(&mut self, params: &FormalParamList) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            self.visit_formal_param(param);
        }
    }

    fn function(&mut self, params: &FormalParamList, body: Option<&FunctionBody>, fresh: bool) {
        let saved = fresh.then(|| std::mem::replace(&mut self.locals, LocalTypes::new()));
        if !fresh {
            self.locals.push_scope();
        }
        self.visit_params(params);
        self.locals.bind_params(params);
        if let Some(body) = body {
            self.body(body);
        }
        if let Some(saved) = saved {
            self.locals = saved;
        } else {
            self.locals.pop_scope();
        }
    }

    fn constructor_initializer(&mut self, initializer: &ConstructorInitializer) {
        match initializer {
            ConstructorInitializer::SuperCall { args, .. }
            | ConstructorInitializer::ThisCall { args, .. } => {
                for argument in &args.positional {
                    self.visit_expr(argument);
                }
                for argument in &args.named {
                    self.visit_expr(&argument.value);
                }
            }
            ConstructorInitializer::FieldInit { value, .. } => self.visit_expr(value),
            ConstructorInitializer::Assert {
                condition, message, ..
            } => {
                self.visit_expr(condition);
                if let Some(message) = message {
                    self.visit_expr(message);
                }
            }
        }
    }

    fn for_stmt(&mut self, node: &ForStmt) {
        self.locals.push_scope();
        if let Some(init) = &node.init {
            match init {
                ForInit::VarDecl(declaration) => {
                    self.visit_stmt(&Stmt::LocalVar(declaration.clone()));
                }
                ForInit::ForIn { iterable, .. } | ForInit::PatternForIn { iterable, .. } => {
                    self.visit_expr(iterable);
                    self.locals.bind_for_init(init);
                }
                ForInit::Exprs(expressions) => {
                    for expression in expressions {
                        self.visit_expr(expression);
                    }
                }
            }
        }
        if let Some(condition) = &node.condition {
            self.visit_expr(condition);
        }
        for update in &node.update {
            self.visit_expr(update);
        }
        self.visit_stmt(&node.body);
        self.locals.pop_scope();
    }

    fn try_catch(&mut self, node: &TryCatchStmt) {
        self.locals.push_scope();
        self.statements(&node.body.stmts);
        self.locals.pop_scope();
        for catch in &node.catches {
            self.locals.push_scope();
            self.locals.bind_catch(catch);
            self.statements(&catch.body.stmts);
            self.locals.pop_scope();
        }
        if let Some(finally) = &node.finally {
            self.locals.push_scope();
            self.statements(&finally.stmts);
            self.locals.pop_scope();
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        match node {
            TopLevelDecl::MixinClass(declaration) => {
                self.with_members(&declaration.members, |collector| {
                    walk_top_level_decl(collector, node);
                });
            }
            TopLevelDecl::ExtensionType(declaration) => {
                self.with_members(&declaration.members, |collector| {
                    walk_top_level_decl(collector, node);
                });
            }
            _ => walk_top_level_decl(self, node),
        }
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.with_members(&node.members, |collector| walk_class_decl(collector, node));
    }

    fn visit_mixin_decl(&mut self, node: &MixinDecl) {
        self.with_members(&node.members, |collector| walk_mixin_decl(collector, node));
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        let mut names = member_names(&node.members);
        names.extend(
            node.variants
                .iter()
                .map(|variant| variant.name.name.clone()),
        );
        self.enclosing_members.push(names);
        walk_enum_decl(self, node);
        self.enclosing_members.pop();
    }

    fn visit_extension_decl(&mut self, node: &ExtensionDecl) {
        self.with_members(&node.members, |collector| {
            walk_extension_decl(collector, node)
        });
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(&node.params, node.body.as_ref(), true);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(&node.params, node.body.as_ref(), true);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let saved = std::mem::replace(&mut self.locals, LocalTypes::new());
        self.visit_params(&node.params);
        self.locals.bind_params(&node.params);
        for initializer in &node.initializers {
            self.constructor_initializer(initializer);
        }
        if let Some(body) = &node.body {
            self.body(body);
        }
        self.locals = saved;
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        let saved = std::mem::replace(&mut self.locals, LocalTypes::new());
        if let Some(body) = &node.body {
            self.body(body);
        }
        self.locals = saved;
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let saved = std::mem::replace(&mut self.locals, LocalTypes::new());
        let type_ = node
            .param_type
            .as_ref()
            .map(StaticType::from_dart_type)
            .unwrap_or(StaticType::Unknown);
        self.locals.declare(node.param.name.clone(), type_);
        if let Some(body) = &node.body {
            self.body(body);
        }
        self.locals = saved;
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.function(&operator.params, operator.body.as_ref(), true);
        } else {
            walk_class_member(self, node);
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match classify_stmt(node) {
            StmtAction::LocalVar(declaration) => {
                walk_stmt(self, node);
                self.locals.declare_local(declaration);
            }
            StmtAction::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.visit_pattern(&declaration.pattern);
                self.locals.bind_pattern(&declaration.pattern);
            }
            StmtAction::Block => {
                self.locals.push_scope();
                walk_stmt(self, node);
                self.locals.pop_scope();
            }
            StmtAction::IfCase(if_statement) => {
                let IfCondition::Case(scrutinee, pattern, guard) = &if_statement.condition else {
                    unreachable!("classified as if-case")
                };
                self.visit_expr(scrutinee);
                self.locals.push_scope();
                self.visit_pattern(pattern);
                self.locals.bind_pattern(pattern);
                if let Some(guard) = guard {
                    self.visit_expr(guard);
                }
                self.visit_stmt(&if_statement.then_branch);
                self.locals.pop_scope();
                if let Some(else_branch) = &if_statement.else_branch {
                    self.visit_stmt(else_branch);
                }
            }
            StmtAction::For(for_statement) => self.for_stmt(for_statement),
            StmtAction::Switch(switch) => {
                self.visit_expr(&switch.subject);
                for case in &switch.cases {
                    self.locals.push_scope();
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                            self.visit_pattern(pattern);
                            self.locals.bind_pattern(pattern);
                            if let Some(guard) = guard.as_ref() {
                                self.visit_expr(guard);
                            }
                        }
                    }
                    self.statements(&case.body);
                    self.locals.pop_scope();
                }
            }
            StmtAction::TryCatch(try_catch) => self.try_catch(try_catch),
            StmtAction::LocalFunc(function) => {
                self.locals
                    .declare(function.name.name.clone(), StaticType::Unknown);
                self.function(&function.params, Some(&function.body), false);
            }
            StmtAction::Other => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::FuncExpr { params, body, .. } => self.function(params, Some(body), false),
            _ => walk_expr(self, node),
        }
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        self.check_pattern(node);
        walk_pattern(self, node);
    }
}

fn member_names(members: &[ClassMember]) -> HashSet<String> {
    let mut names = HashSet::new();
    for member in members {
        match member {
            ClassMember::Field(field) => {
                names.extend(
                    field
                        .declarators
                        .iter()
                        .map(|declarator| declarator.name.name.clone()),
                );
            }
            ClassMember::Method(method) => {
                names.insert(method.name.name.clone());
            }
            ClassMember::Getter(getter) => {
                names.insert(getter.name.name.clone());
            }
            ClassMember::Setter(setter) => {
                names.insert(setter.name.name.clone());
            }
            ClassMember::Constructor(_) | ClassMember::Operator(_) | ClassMember::Error(_) => {}
        }
    }
    names
}
