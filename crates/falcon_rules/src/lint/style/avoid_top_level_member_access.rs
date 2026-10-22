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
use falcon_syntax::cst::visit::bound_names;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidTopLevelMemberAccess;

impl Rule for AvoidTopLevelMemberAccess {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let top_level_vars = program
            .declarations()
            .filter_map(|decl| match decl {
                TopLevelDecl::Variable(var) if var.keyword() == VarKeyword::Var => Some(var),
                _ => None,
            })
            .flat_map(|var| var.declarators().map(|decl| decl.name().name()))
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

impl<'a> Collector<'a> {
    fn bind(&mut self, name: Identifier<'a>, span: Span) {
        if let Some(&name) = self.top_level_vars.get(name.name()) {
            self.bindings.entry(name).or_default().push(span);
        }
    }

    fn bind_params(&mut self, params: FormalParamList<'a>, span: Span) {
        for param in params.params() {
            self.bind(param.name(), span);
        }
    }

    fn bind_pattern(&mut self, pattern: Pattern<'a>, span: Span) {
        for name in bound_names(pattern) {
            self.bind(name, span);
        }
    }

    fn enter_statements(&mut self, stmts: AstChildren<'a, Stmt<'a>>, span: Span) {
        for stmt in stmts {
            if let Stmt::LocalFunc(local) = stmt {
                self.bind(local.name(), span);
            }
        }
        self.block_ends.push(span.end);
    }

    fn bind_for_init(&mut self, init: ForInit<'a>, statement: ForStmt<'a>) {
        match init {
            ForInit::VarDecl(declaration) => {
                for declarator in declaration.declarators() {
                    self.bind(
                        declarator.name(),
                        Span::new(declarator.name().span().end, statement.span().end),
                    );
                }
                self.suppress_next_local_var_binding = true;
            }
            ForInit::ForIn(init) if init.declares_variable() => {
                self.bind(init.name(), statement.body().span());
            }
            ForInit::PatternForIn(init) => {
                self.bind_pattern(init.pattern(), statement.body().span());
            }
            ForInit::ForIn(_) | ForInit::Exprs(_) => {}
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.events.iter().filter(|(span, access)| {
            access.is_none_or(|name| !self.bindings.get(name).is_some_and(|scopes| {
                scopes.iter().any(|scope| scope.contains_offset(span.start))
            }))
        }).map(|(span, _)| self.ctx.diagnostic(
            "Avoid using non-const top-level members.",
            *span,
        )).collect()
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(var) = node {
            if var.keyword() == VarKeyword::Var {
                self.events.push((var.span(), None));
            }
            self.top_level_initializer = true;
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _: TopLevelDecl<'a>) {
        self.top_level_initializer = false;
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.is_static() && node.keyword() != VarKeyword::Final && node.keyword() != VarKeyword::Const {
            self.events.push((node.span(), None));
        }
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if let (Some(body), Some(params)) = (node.body(), node.params()) {
            self.bind_params(params, body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) -> StructuralTraversal {
        self.bind_params(node.params(), Span::new(node.params().span().end, node.span().end));
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        if let Some(body) = node.body() {
            self.bind_params(node.params(), body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        if let Some(body) = node.body() {
            self.bind(node.param(), body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_operator_decl(&mut self, node: OperatorDecl<'a>) -> StructuralTraversal {
        if let Some(body) = node.body() {
            self.bind_params(node.params(), body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        if !std::mem::take(&mut self.suppress_next_local_var_binding) {
            for declaration in node.declarators() {
                if let Some(end) = self.block_ends.last().copied() {
                    self.bind(declaration.name(), Span::new(declaration.name().span().end, end));
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_primary_constructor_body(&mut self, node: PrimaryConstructorBody<'a>) -> StructuralTraversal {
        self.primary_body = match node.body() {
            Some(FunctionBody::Block(block)) => Some(block.span()),
            _ => None,
        };
        StructuralTraversal::Continue
    }

    fn exit_primary_constructor_body(&mut self, _: PrimaryConstructorBody<'a>) {
        self.primary_body = None;
    }

    fn visit_block(&mut self, node: Block<'a>) -> StructuralTraversal {
        if self.primary_body != Some(node.span()) {
            self.enter_statements(node.stmts(), node.span());
        }
        StructuralTraversal::Continue
    }

    fn exit_block(&mut self, node: Block<'a>) {
        if self.primary_body != Some(node.span()) {
            self.block_ends.pop();
        }
    }

    fn visit_switch_case(&mut self, node: SwitchCase<'a>) -> StructuralTraversal {
        for kind in node.cases() {
            if let SwitchCaseKind::Pattern(label) = kind {
                let pattern = label.pattern();
                self.bind_pattern(pattern, Span::new(pattern.span().end, node.span().end));
            }
        }
        self.enter_statements(node.body(), node.span());
        StructuralTraversal::Continue
    }

    fn exit_switch_case(&mut self, _: SwitchCase<'a>) {
        self.block_ends.pop();
    }

    fn visit_catch_clause(&mut self, node: CatchClause<'a>) -> StructuralTraversal {
        if let Some(name) = node.exception_var() {
            self.bind(name, node.body().span());
        }
        if let Some(name) = node.stack_trace_var() {
            self.bind(name, node.body().span());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        match node {
            Stmt::PatternDecl(decl) => {
                if let Some(end) = self.block_ends.last().copied() {
                    self.bind_pattern(decl.pattern(), Span::new(decl.span().end, end));
                }
            }
            Stmt::For(statement) => {
                if let Some(init) = statement.header().init() {
                    self.bind_for_init(init, statement);
                }
            }
            Stmt::If(statement) => {
                if let IfCondition::Case(condition) = statement.condition() {
                    let pattern = condition.pattern();
                    self.bind_pattern(pattern, Span::new(pattern.span().end, statement.then_branch().span().end));
                }
            }
            Stmt::LocalFunc(local) => self.bind_params(local.params(), local.body().span()),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(closure) = node {
            self.bind_params(closure.params(), closure.body().span());
        }
        if !self.top_level_initializer
            && let Expr::Ident(id) = node
            && let Some(&name) = self.top_level_vars.get(id.name())
        {
            self.events.push((id.span(), Some(name)));
        }
        StructuralTraversal::Continue
    }
}
}
