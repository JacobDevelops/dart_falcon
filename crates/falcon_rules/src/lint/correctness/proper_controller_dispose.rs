//! Requires Flutter controllers owned by a `State` field to be disposed.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex,
    TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, bound_names, walk_expr, walk_pattern, walk_stmt};

pub struct ProperControllerDispose;

impl Rule for ProperControllerDispose {
    fn name(&self) -> &'static str {
        "proper-controller-dispose"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diagnostics = Vec::new();
        for declaration in &program.declarations {
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) => (&class.name.name, class.members.as_slice()),
                TopLevelDecl::MixinClass(class) => (&class.name.name, class.members.as_slice()),
                _ => continue,
            };
            if !is_flutter_state(name, signatures, &model) {
                continue;
            }
            check_class(members, &model, ctx, &mut diagnostics);
        }
        diagnostics
    }
}

fn is_flutter_state(name: &str, signatures: &SignatureIndex, model: &SemanticModel<'_>) -> bool {
    let Some(identity) = model.resolve_name(&[name.to_string()]) else {
        return false;
    };
    let ty = ResolvedType::Interface {
        identity,
        arguments: Vec::new(),
        nullable: false,
        extension_type: false,
    };
    let state = DeclarationIdentity::Package {
        package: "flutter".to_string(),
        name: "State".to_string(),
    };
    signatures.is_subtype_of(&ty, &state, model) == TypeTruth::Yes
}

fn check_class(
    members: &[ClassMember],
    model: &SemanticModel<'_>,
    ctx: &AnalyzeContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let fields: HashSet<String> = members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Field(field) if !field.is_static => Some(field),
            _ => None,
        })
        .flat_map(|field| {
            field
                .declarators
                .iter()
                .map(|declarator| declarator.name.name.clone())
        })
        .collect();
    let mut owned = HashSet::new();
    let mut disposed = HashSet::new();

    for member in members {
        if let ClassMember::Field(field) = member {
            for declarator in &field.declarators {
                if declarator
                    .initializer
                    .as_ref()
                    .is_some_and(|initializer| constructs_controller(initializer, model))
                {
                    owned.insert(declarator.name.name.clone());
                }
            }
        }
    }

    for member in members {
        let (params, body, in_dispose) = match member {
            ClassMember::Method(method) => (
                Some(&method.params),
                method.body.as_ref(),
                method.name.name == "dispose",
            ),
            ClassMember::Constructor(constructor) => {
                (Some(&constructor.params), constructor.body.as_ref(), false)
            }
            ClassMember::Getter(getter) => (None, getter.body.as_ref(), false),
            ClassMember::Setter(setter) => {
                let Some(body) = setter.body.as_ref() else {
                    continue;
                };
                let mut scan = Scan::new(&fields, model, &mut owned, &mut disposed, false);
                scan.declare(&setter.param.name);
                scan.body(body);
                continue;
            }
            ClassMember::Operator(operator) => {
                (Some(&operator.params), operator.body.as_ref(), false)
            }
            _ => continue,
        };
        let Some(body) = body else {
            continue;
        };
        let mut scan = Scan::new(&fields, model, &mut owned, &mut disposed, in_dispose);
        if let Some(params) = params {
            scan.params(params);
        }
        scan.body(body);
    }

    for member in members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        for declarator in &field.declarators {
            if owned.contains(&declarator.name.name) && !disposed.contains(&declarator.name.name) {
                diagnostics.push(Diagnostic::new(
                    "proper-controller-dispose",
                    Severity::Warning,
                    "This controller is never disposed; dispose it in the State's dispose() method.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: declarator.name.span.start,
                        end: declarator.name.span.end,
                    },
                ));
            }
        }
    }
}

struct Scan<'a, 'model> {
    fields: &'a HashSet<String>,
    model: &'model SemanticModel<'model>,
    owned: &'a mut HashSet<String>,
    disposed: &'a mut HashSet<String>,
    in_dispose: bool,
    scopes: Vec<HashSet<String>>,
}

impl<'a, 'model> Scan<'a, 'model> {
    fn new(
        fields: &'a HashSet<String>,
        model: &'model SemanticModel<'model>,
        owned: &'a mut HashSet<String>,
        disposed: &'a mut HashSet<String>,
        in_dispose: bool,
    ) -> Self {
        Self {
            fields,
            model,
            owned,
            disposed,
            in_dispose,
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

    fn pattern(&mut self, pattern: &Pattern) {
        walk_pattern(self, pattern);
        for name in bound_names(pattern) {
            self.declare(&name.name);
        }
    }

    fn field_target<'b>(&self, expression: &'b Expr) -> Option<&'b str> {
        match expression {
            Expr::Field { object, field, .. }
                if matches!(object.as_ref(), Expr::This { .. })
                    && self.fields.contains(&field.name) =>
            {
                Some(&field.name)
            }
            Expr::Ident(identifier)
                if self.fields.contains(&identifier.name) && !self.bound(&identifier.name) =>
            {
                Some(&identifier.name)
            }
            _ => None,
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

impl Visitor for Scan<'_, '_> {
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
                    self.visit_stmt(&statement.then_branch);
                    if let Some(branch) = &statement.else_branch {
                        self.visit_stmt(branch);
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
                        self.visit_stmt(branch);
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
            Stmt::TryCatch(statement) => {
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
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
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Assign { target, value, .. }
                if constructs_controller(value, self.model) =>
            {
                if let Some(field) = self.field_target(target) {
                    self.owned.insert(field.to_string());
                }
            }
            Expr::Call { callee, .. } if self.in_dispose => {
                if let Expr::Field { object, field, .. } = callee.as_ref()
                    && field.name == "dispose"
                    && let Some(controller) = self.field_target(object)
                {
                    self.disposed.insert(controller.to_string());
                }
            }
            Expr::Cascade {
                object, sections, ..
            } if self.in_dispose
                && sections.iter().any(|section| {
                    matches!(section.ops.first(), Some(CascadeOp::Call(name, _, _)) if name.name == "dispose")
                }) =>
            {
                if let Some(controller) = self.field_target(object) {
                    self.disposed.insert(controller.to_string());
                }
            }
            Expr::FuncExpr { params, body, .. } => {
                self.nested_function(params, body);
                return;
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

fn constructor_identity(callee: &Expr, model: &SemanticModel<'_>) -> Option<DeclarationIdentity> {
    match callee {
        Expr::Ident(identifier) => model.resolve_name(std::slice::from_ref(&identifier.name)),
        Expr::Field { object, field, .. } => match object.as_ref() {
            Expr::Ident(identifier) => model
                .resolve_name(&[identifier.name.clone(), field.name.clone()])
                .or_else(|| model.resolve_name(std::slice::from_ref(&identifier.name))),
            Expr::Field {
                object: prefix,
                field: type_name,
                ..
            } => match prefix.as_ref() {
                Expr::Ident(prefix) => {
                    model.resolve_name(&[prefix.name.clone(), type_name.name.clone()])
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn constructs_controller(expression: &Expr, model: &SemanticModel<'_>) -> bool {
    let identity = match expression {
        Expr::New { dart_type, .. } => match model.resolve_type(dart_type) {
            ResolvedType::Interface { identity, .. } => Some(identity),
            _ => None,
        },
        Expr::Call { callee, .. } => constructor_identity(callee, model),
        _ => None,
    };
    matches!(identity, Some(DeclarationIdentity::Package { package, name })
        if package == "flutter" && matches!(name.as_str(),
            "TextEditingController" | "ScrollController" | "PageController" |
            "TabController" | "AnimationController"))
}
