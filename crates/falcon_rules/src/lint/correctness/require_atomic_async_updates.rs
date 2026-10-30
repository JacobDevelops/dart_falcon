//! Warns when an assignment reads its target before an `await` and writes it afterwards.
//!
//! Uncaptured local variables and parameters are excluded. Shared targets,
//! including lazy initialization with `??=`, should be read after awaiting.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, FunctionBodies, MemberSelector, Rule, RulePreparation,
    SemanticRuleVisitor, SemanticState,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, bound_names, walk_element, walk_expr_chain};
use falcon_syntax::cst::*;

use crate::lint::support::element_identity::same_resolved_element;
use crate::lint::support::syntax::{same_syntax, scalar_literal};

#[derive(Default)]
pub struct RequireAtomicAsyncUpdates;

impl Rule for RequireAtomicAsyncUpdates {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            bodies: ctx.function_bodies(program),
            locals: ctx.local_declarations(program),
            program,
            captured: OnceCell::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    bodies: &'a FunctionBodies<'a>,
    locals: &'a HashMap<usize, NodeId>,
    program: Program<'a>,
    captured: OnceCell<HashSet<NodeId>>,
    diagnostics: Vec<Diagnostic>,
}

fn executable_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDecl
            | NodeKind::MethodDecl
            | NodeKind::GetterDecl
            | NodeKind::SetterDecl
            | NodeKind::ConstructorDecl
            | NodeKind::PrimaryConstructor
            | NodeKind::PrimaryConstructorBody
            | NodeKind::OperatorDecl
            | NodeKind::LocalFuncDecl
            | NodeKind::FuncExpr
    )
}

fn captured_bindings<'a>(program: Program<'a>, ctx: &AnalyzeContext<'a>) -> HashSet<NodeId> {
    let nodes = program.syntax().descendants();
    let base = program.syntax().id().0;
    let locals = ctx.local_declarations(program);
    let writes = ctx.local_writes(program);
    let mut owners = Vec::with_capacity(nodes.len());
    let mut scopes: HashMap<_, _> = locals
        .keys()
        .copied()
        .chain(writes.iter().map(|write| write.span.start))
        .map(|start| (start, None))
        .collect();
    for node in nodes {
        let owner = if executable_kind(node.kind()) {
            Some(node.id())
        } else {
            node.parent().and_then(|parent| {
                owners
                    .get((parent.id().0 - base) as usize)
                    .copied()
                    .flatten()
            })
        };
        owners.push(owner);
        if let Some(scope) = scopes.get_mut(&node.span().start) {
            *scope = owner;
        }
    }
    locals
        .iter()
        .map(|(&start, &declaration)| (start, declaration))
        .chain(
            writes
                .iter()
                .map(|write| (write.span.start, write.declaration)),
        )
        .filter_map(|(start, declaration)| {
            (scopes.get(&start) != owners.get((declaration.0 - base) as usize))
                .then_some(declaration)
        })
        .collect()
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Assign(assignment) = node else {
            return;
        };
        if !self
            .bodies
            .enclosing(&assignment.span())
            .is_some_and(|body| body.async_marker.is_async())
        {
            return;
        }
        let target = assignment.target().unparenthesized();
        if !matches!(target, Expr::Ident(_) | Expr::Field(_) | Expr::Index(_)) {
            return;
        }
        if let Expr::Ident(identifier) = target
            && self.locals.get(&identifier.span().start).is_some_and(|id| {
                !self
                    .captured
                    .get_or_init(|| captured_bindings(self.program, self.ctx))
                    .contains(id)
            })
        {
            return;
        }
        let target_declaration = state
            .referenced_declaration(target, false)
            .map(|declaration| (declaration.owner, declaration.selector));
        let mut analysis = AwaitedRead {
            target,
            target_declaration,
            state,
            read: assignment.op() != AssignOp::Eq,
            stale: false,
            suppress_reads: false,
        };
        analysis.visit_expr(assignment.value());
        if analysis.stale {
            self.diagnostics.push(self.ctx.diagnostic(
                "This reassignment may use an outdated value after `await`.",
                target.span(),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

struct AwaitedRead<'s, 'a> {
    target: Expr<'a>,
    target_declaration: Option<(DeclarationIdentity, MemberSelector<'a>)>,
    state: &'s SemanticState<'a>,
    read: bool,
    stale: bool,
    // SemanticState has the assignment's scope, not bindings introduced inside
    // its RHS. Suppress new reads only where such a binding shadows the target;
    // incoming reads and suspensions still propagate through that scope.
    suppress_reads: bool,
}

impl<'a> AwaitedRead<'_, 'a> {
    fn is_target(&self, expression: Expr<'a>) -> bool {
        let expression = expression.unparenthesized();
        let implicit_receiver = |node| match node {
            Expr::Ident(_) => true,
            Expr::Field(field) => matches!(field.object().unparenthesized(), Expr::This(_)),
            _ => false,
        };
        if implicit_receiver(self.target)
            && implicit_receiver(expression)
            && let (Some((owner, selector)), Some(right)) = (
                &self.target_declaration,
                self.state.referenced_declaration(expression, false),
            )
        {
            return *owner == right.owner && *selector == right.selector;
        }
        match (self.target, expression) {
            (Expr::Index(left), Expr::Index(right)) => {
                same_resolved_element(left.object(), false, right.object(), false, self.state)
                    && (same_resolved_element(
                        left.index(),
                        false,
                        right.index(),
                        false,
                        self.state,
                    ) || (scalar_literal(left.index())
                        && scalar_literal(right.index())
                        && same_syntax(left.index().syntax(), right.index().syntax())))
            }
            _ => same_resolved_element(self.target, false, expression, false, self.state),
        }
    }

    fn target_mentions(&self, name: &str) -> bool {
        struct UsesName<'n> {
            name: &'n str,
            found: bool,
        }
        impl<'a> Visitor<'a> for UsesName<'_> {
            fn visit_expr(&mut self, node: Expr<'a>) {
                walk_expr_chain(self, node, |visitor, node| {
                    if visitor.found {
                        return false;
                    }
                    visitor.found = match node {
                        Expr::Ident(identifier) => identifier.name() == visitor.name,
                        Expr::Field(field)
                            if matches!(field.object().unparenthesized(), Expr::This(_)) =>
                        {
                            field.field().name() == visitor.name
                        }
                        _ => false,
                    };
                    !visitor.found
                });
            }
        }
        let mut uses = UsesName { name, found: false };
        uses.visit_expr(self.target);
        uses.found
    }

    fn pattern_shadows(&self, pattern: Pattern<'a>) -> bool {
        bound_names(pattern)
            .into_iter()
            .any(|name| self.target_mentions(name.name()))
    }

    fn visit_switch(&mut self, node: SwitchExpr<'a>) {
        self.visit_expr(node.subject());
        let mut completed_read = self.read;
        for arm in node.arms() {
            let suppressed = self.suppress_reads;
            self.suppress_reads |= self.pattern_shadows(arm.pattern());
            if let Some(guard) = arm.guard() {
                self.visit_expr(guard);
            }
            // A failed guard can precede a later arm, but an arm body cannot.
            let guard_read = self.read;
            self.visit_expr(arm.body());
            completed_read |= self.read;
            self.read = guard_read;
            self.suppress_reads = suppressed;
            if self.stale {
                return;
            }
        }
        self.read |= completed_read;
    }

    fn visit_collection_if<E: Element<'a>>(
        &mut self,
        condition: IfCondition<'a>,
        then_element: E,
        else_element: Option<E>,
    ) {
        let suppressed = self.suppress_reads;
        match condition {
            IfCondition::Expr(expression) => self.visit_expr(expression),
            IfCondition::Case(condition) => {
                self.visit_expr(condition.expr());
                self.suppress_reads |= self.pattern_shadows(condition.pattern());
                if let Some(guard) = condition.guard() {
                    self.visit_expr(guard);
                }
            }
        }
        let before = self.read;
        self.visit_element(then_element);
        let then_read = self.read;
        self.read = before;
        self.suppress_reads = suppressed;
        if let Some(element) = else_element {
            self.visit_element(element);
        }
        self.read |= then_read;
    }

    fn visit_collection_for<E: Element<'a>>(
        &mut self,
        header: ForHeader<'a>,
        body: E,
        is_await: bool,
    ) {
        let suppressed = self.suppress_reads;
        if let Some(init) = header.init() {
            match init {
                ForInit::ForIn(init) => {
                    self.visit_expr(init.iterable());
                    self.suppress_reads |=
                        init.declares_variable() && self.target_mentions(init.name().name());
                }
                ForInit::PatternForIn(init) => {
                    self.visit_expr(init.iterable());
                    self.suppress_reads |= self.pattern_shadows(init.pattern());
                }
                ForInit::VarDecl(init) => {
                    self.suppress_reads |= init
                        .declarators()
                        .any(|declaration| self.target_mentions(declaration.name().name()));
                    self.visit_local_var(init);
                }
                ForInit::Exprs(init) => {
                    for expression in init.exprs() {
                        self.visit_expr(expression);
                    }
                }
            }
        }
        if let Some(condition) = header.condition() {
            self.visit_expr(condition);
        }
        // Read state is monotone: two iterations also cover a read in one
        // iteration followed by suspension in the next.
        for _ in 0..2 {
            let before = self.read;
            if is_await {
                self.stale |= self.read;
            }
            self.visit_element(body);
            for update in header.updates() {
                self.visit_expr(update);
            }
            if let Some(condition) = header.condition() {
                self.visit_expr(condition);
            }
            if self.stale || self.read == before {
                break;
            }
        }
        self.suppress_reads = suppressed;
    }
}

impl<'a> AwaitedRead<'_, 'a> {
    fn visit_evaluation(visitor: &mut Self, node: Expr<'a>) -> bool {
        if visitor.stale {
            return false;
        }
        match node {
            Expr::FuncExpr(_) => false,
            Expr::Await(awaited) => {
                visitor.visit_expr(awaited.expr());
                visitor.stale |= visitor.read;
                false
            }
            Expr::Conditional(conditional) => {
                visitor.visit_expr(conditional.condition());
                let before = visitor.read;
                visitor.visit_expr(conditional.then_expr());
                let then_read = visitor.read;
                visitor.read = before;
                visitor.visit_expr(conditional.else_expr());
                visitor.read |= then_read;
                false
            }
            Expr::Switch(switch) => {
                visitor.visit_switch(switch);
                false
            }
            Expr::Assign(assignment) => {
                if assignment.op() == AssignOp::Eq {
                    // A write does not read its target, but computing its
                    // receiver and index does evaluate those expressions.
                    match assignment.target().unparenthesized() {
                        Expr::Field(field) => visitor.visit_expr(field.object()),
                        Expr::Index(index) => {
                            visitor.visit_expr(index.object());
                            visitor.visit_expr(index.index());
                        }
                        _ => {}
                    }
                } else {
                    visitor.visit_expr(assignment.target());
                }
                visitor.visit_expr(assignment.value());
                false
            }
            _ => {
                if !visitor.read && !visitor.suppress_reads {
                    visitor.read = visitor.is_target(node);
                }
                true
            }
        }
    }
}

impl<'a> Visitor<'a> for AwaitedRead<'_, 'a> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        walk_expr_chain(self, node, Self::visit_evaluation);
    }

    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        if self.stale {
            return;
        }
        match element.kind() {
            ElementKind::If {
                condition,
                then_element,
                else_element,
            } => {
                self.visit_collection_if(condition, then_element, else_element);
            }
            ElementKind::For { header, body } => {
                self.visit_collection_for(header, body, element.is_await());
            }
            kind => walk_element(self, kind),
        }
    }
}
