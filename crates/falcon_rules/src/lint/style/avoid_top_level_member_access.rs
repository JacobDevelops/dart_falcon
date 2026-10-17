//! Flags mutable global state: non-`const`, non-`final` top-level variables and
//! static fields, plus every read of mutable top-level variables.
//!
//! Mutable top-level and static members are shared, unscoped state that any
//! code can read or write, which makes data flow hard to follow and tests hard
//! to isolate. Prefer `const` or `final` values, or pass state explicitly
//! through constructors and parameters.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::bound_names;

#[derive(Default)]
pub struct AvoidTopLevelMemberAccess;

impl Rule for AvoidTopLevelMemberAccess {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let top_level_vars = program
            .declarations
            .iter()
            .filter_map(|decl| match decl {
                TopLevelDecl::Variable(var) if var.keyword == VarKeyword::Var => Some(var),
                _ => None,
            })
            .flat_map(|var| var.declarators.iter().map(|decl| decl.name.name.as_str()))
            .collect();
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            top_level_vars,
            bindings: HashMap::new(),
            block_ends: Vec::new(),
            suppress_next_local_var_binding: false,
            top_level_initializer: false,
            primary_body: None,
            events: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    top_level_vars: HashSet<&'a str>,
    bindings: HashMap<&'a str, Vec<Span>>,
    block_ends: Vec<usize>,
    suppress_next_local_var_binding: bool,
    top_level_initializer: bool,
    events: Vec<(Span, Option<&'a str>)>,
    primary_body: Option<Span>,
}

impl Collector<'_> {
    fn bind(&mut self, name: &Identifier, span: Span) {
        if let Some(&name) = self.top_level_vars.get(name.name.as_str()) {
            self.bindings.entry(name).or_default().push(span);
        }
    }

    fn bind_params(&mut self, params: &FormalParamList, span: Span) {
        for param in &params.params {
            self.bind(&param.name, span);
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern, span: Span) {
        for name in bound_names(pattern) {
            self.bind(name, span);
        }
    }

    fn enter_statements(&mut self, stmts: &[Stmt], span: Span) {
        for stmt in stmts {
            if let Stmt::LocalFunc(local) = stmt {
                self.bind(&local.name, span);
            }
        }
        self.block_ends.push(span.end);
    }

    fn bind_for_init(&mut self, init: &ForInit, statement: &ForStmt) {
        match init {
            ForInit::VarDecl(declaration) => {
                for declarator in &declaration.declarators {
                    self.bind(
                        &declarator.name,
                        Span::new(declarator.name.span.end, statement.span.end),
                    );
                }
                self.suppress_next_local_var_binding = true;
            }
            ForInit::ForIn {
                declares_variable: true,
                name,
                ..
            } => self.bind(name, *statement.body.span()),
            ForInit::PatternForIn { pattern, .. } => {
                self.bind_pattern(pattern, *statement.body.span())
            }
            ForInit::ForIn {
                declares_variable: false,
                ..
            }
            | ForInit::Exprs(_) => {}
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.events.iter().filter(|(span, access)| {
            access.is_none_or(|name| !self.bindings.get(name).is_some_and(|scopes| {
                scopes.iter().any(|scope| scope.start <= span.start && span.start < scope.end)
            }))
        }).map(|(span, _)| self.ctx.diagnostic(
            "avoid-top-level-member-access",
            "Avoid using non-const top-level members",
            *span,
        )).collect()
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        if let TopLevelDecl::Variable(var) = node {
            if var.keyword == VarKeyword::Var {
                self.events.push((var.span, None));
            }
            self.top_level_initializer = true;
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _: &TopLevelDecl) {
        self.top_level_initializer = false;
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        if node.is_static && node.keyword != VarKeyword::Final && node.keyword != VarKeyword::Const {
            self.events.push((node.span, None));
        }
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if let (Some(body), Some(params)) = (&node.body, node.params()) {
            self.bind_params(params, *body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) -> StructuralTraversal {
        self.bind_params(&node.params, Span::new(node.params.span.end, node.span.end));
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        if let Some(body) = &node.body {
            self.bind_params(&node.params, *body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        if let Some(body) = &node.body {
            self.bind(&node.param, *body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_operator_decl(&mut self, node: &OperatorDecl) -> StructuralTraversal {
        if let Some(body) = &node.body {
            self.bind_params(&node.params, *body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        if !std::mem::take(&mut self.suppress_next_local_var_binding) {
            for declaration in &node.declarators {
                if let Some(end) = self.block_ends.last().copied() {
                    self.bind(&declaration.name, Span::new(declaration.name.span.end, end));
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_primary_constructor_body(&mut self, node: &PrimaryConstructorBody) -> StructuralTraversal {
        self.primary_body = match &node.body {
            Some(FunctionBody::Block(block)) => Some(block.span),
            _ => None,
        };
        StructuralTraversal::Continue
    }

    fn exit_primary_constructor_body(&mut self, _: &PrimaryConstructorBody) {
        self.primary_body = None;
    }

    fn visit_block(&mut self, node: &Block) -> StructuralTraversal {
        if self.primary_body != Some(node.span) {
            self.enter_statements(&node.stmts, node.span);
        }
        StructuralTraversal::Continue
    }

    fn exit_block(&mut self, node: &Block) {
        if self.primary_body != Some(node.span) {
            self.block_ends.pop();
        }
    }

    fn visit_switch_case(&mut self, node: &SwitchCase) -> StructuralTraversal {
        for kind in &node.cases {
            if let SwitchCaseKind::Pattern(pattern, _) = kind {
                self.bind_pattern(pattern, Span::new(pattern.span().end, node.span.end));
            }
        }
        self.enter_statements(&node.body, node.span);
        StructuralTraversal::Continue
    }

    fn exit_switch_case(&mut self, _: &SwitchCase) {
        self.block_ends.pop();
    }

    fn visit_catch_clause(&mut self, node: &CatchClause) -> StructuralTraversal {
        if let Some(name) = &node.exception_var {
            self.bind(name, node.body.span);
        }
        if let Some(name) = &node.stack_trace_var {
            self.bind(name, node.body.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        match node {
            Stmt::PatternDecl(decl) => {
                if let Some(end) = self.block_ends.last().copied() {
                    self.bind_pattern(&decl.pattern, Span::new(decl.span.end, end));
                }
            }
            Stmt::For(statement) => {
                if let Some(init) = &statement.header.init {
                    self.bind_for_init(init, statement);
                }
            }
            Stmt::If(statement) => {
                if let IfCondition::Case(_, pattern, _) = &statement.condition {
                    self.bind_pattern(pattern, Span::new(pattern.span().end, statement.then_branch.span().end));
                }
            }
            Stmt::LocalFunc(local) => self.bind_params(&local.params, *local.body.span()),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::FuncExpr { params, body, .. } = node {
            self.bind_params(params, *body.span());
        }
        if !self.top_level_initializer
            && let Expr::Ident(id) = node
            && let Some(&name) = self.top_level_vars.get(id.name.as_str())
        {
            self.events.push((id.span, Some(name)));
        }
        StructuralTraversal::Continue
    }
}
}
