//! Shared lexical boundaries and resolved Flutter API checks for State rules.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, RulePreparation, Truth,
    TypeRelation,
};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

pub(crate) fn is_state(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    state.signatures.relation(
        ty,
        TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::flutter("State")),
        &state.model,
    ) == Truth::Yes
}

pub(crate) fn is_context(ty: &ResolvedType) -> bool {
    matches!(ty.view(), ResolvedTypeView::Interface { identity, .. } if identity.is_package("flutter", "BuildContext"))
}

fn context_expression(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    if is_context(&state.infer(expression)) {
        return true;
    }
    let own_context = match expression {
        Expr::Ident(name) => {
            name.name() == "context"
                && !state.environment.is_bound("context")
                && !state.is_own_field("context")
        }
        Expr::Field(field) => {
            field.field().name() == "context" && matches!(field.object(), Expr::This(_))
        }
        _ => false,
    };
    own_context
        && state.enclosing_type.as_ref().is_some_and(|ty| {
            is_state(ty, state)
                && state
                    .signatures
                    .resolved_member_owner(ty, "context", &state.model)
                    .is_none_or(|owner| owner.is_package("flutter", "State"))
        })
}

pub(crate) fn is_setstate(call: CallExpr<'_>, state: &SemanticState<'_>) -> bool {
    let receiver = match call.callee().unparenthesized() {
        Expr::Ident(name)
            if name.name() == "setState" && !state.environment.is_bound("setState") =>
        {
            state.enclosing_type.as_ref()
        }
        Expr::Field(field)
            if field.field().name() == "setState" && matches!(field.object(), Expr::This(_)) =>
        {
            state.enclosing_type.as_ref()
        }
        _ => None,
    };
    receiver.is_some_and(|receiver| {
        is_state(receiver, state)
            && state
                .signatures
                .resolved_member_owner(receiver, "setState", &state.model)
                .is_none_or(|owner| owner.is_package("flutter", "State"))
    })
}

struct Method<'a> {
    declaration: MethodDecl<'a>,
    class: NodeId,
    state: bool,
    calls: Vec<(&'a str, Span)>,
    hazard: bool,
    super_dispose: bool,
}

pub(crate) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    rule: &'static str,
) -> RulePreparation<'a> {
    let Some(state) = SemanticState::new(program, ctx) else {
        return RulePreparation::Diagnostics(Vec::new());
    };
    let mut methods = Vec::new();
    let mut late = Vec::new();
    for declaration in program.declarations() {
        let TopLevelDecl::Class(class) = declaration else {
            continue;
        };
        let flutter_state = state
            .model
            .resolve_name(class.name().name())
            .is_some_and(|identity| is_state(&ResolvedType::interface_identity(identity), &state));
        for member in class.members() {
            match member {
                ClassMember::Method(declaration) => methods.push(Method {
                    declaration,
                    class: class.syntax().id(),
                    state: flutter_state && !declaration.is_static(),
                    calls: Vec::new(),
                    hazard: false,
                    super_dispose: false,
                }),
                ClassMember::Field(field) if field.is_late() && !field.is_static() => {
                    if field
                        .field_type()
                        .is_some_and(|ty| is_context(&state.model.resolve_type(ty)))
                    {
                        late.push((field.span(), true));
                    } else {
                        for declarator in field.declarators() {
                            if let Some(initializer) = declarator.initializer() {
                                late.push((initializer.span(), false));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    RulePreparation::Semantic(Box::new(Collector {
        program,
        ctx,
        rule,
        methods,
        late,
        closures: Vec::new(),
        contexts: Vec::new(),
        diags: Vec::new(),
    }))
}

struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    rule: &'static str,
    methods: Vec<Method<'a>>,
    late: Vec<(Span, bool)>,
    closures: Vec<Span>,
    contexts: Vec<(Span, &'a str)>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        let message = match self.rule {
            "require-super-dispose" => "Call `super.dispose()` in this `State.dispose` override.",
            "avoid-empty-setstate" => {
                "Avoid an empty `setState` callback; update state inside the callback."
            }
            "avoid-unnecessary-setstate" => {
                "Update state directly instead of calling `setState` during this lifecycle method."
            }
            "use-closest-build-context" => {
                "Use the closest `BuildContext` parameter instead of an outer context."
            }
            "avoid-late-context" => {
                "Avoid storing or accessing `BuildContext` in a `late` field initializer."
            }
            _ => "Move inherited-widget subscriptions from `initState` to `didChangeDependencies`.",
        };
        self.diags.push(self.ctx.diagnostic(message, span));
    }
    fn framework_subscription(&self, receiver: Expr<'_>, state: &SemanticState<'_>) -> bool {
        let Some(parts) = falcon_analyze::expression_segments(receiver, false) else {
            return false;
        };
        let Some(name) = parts.last().copied() else {
            return false;
        };
        if !matches!(
            name,
            "Theme"
                | "MediaQuery"
                | "Directionality"
                | "DefaultTextStyle"
                | "IconTheme"
                | "Localizations"
        ) || state.environment.is_bound(parts[0])
        {
            return false;
        }
        if let Some(identity) = state.model.resolve_dotted(&parts) {
            return identity.is_package("flutter", identity.name());
        }
        if state.model.resolve_value(parts[0]).is_some() {
            return false;
        }
        self.program.imports().any(|import| {
            import.uri().value().starts_with("package:flutter/")
                && match (parts.as_slice(), import.as_name()) {
                    ([_], None) => true,
                    ([prefix, _], Some(alias)) => *prefix == alias.name(),
                    _ => false,
                }
                && import.combinators().all(|combinator| match combinator {
                    ImportCombinator::Show(names) => {
                        names.names().any(|shown| shown.name() == name)
                    }
                    ImportCombinator::Hide(names) => {
                        names.names().all(|hidden| hidden.name() != name)
                    }
                })
        })
    }
    fn check_context_reference(&mut self, expression: Expr<'_>, state: &SemanticState<'_>) {
        if self.rule == "avoid-late-context" {
            if matches!(expression, Expr::Ident(_) | Expr::Field(_))
                && !self.late.is_empty()
                && self
                    .late
                    .iter()
                    .any(|(span, _)| span.contains(expression.span()))
                && !self
                    .closures
                    .iter()
                    .any(|span| span.contains(expression.span()))
                && context_expression(expression, state)
            {
                self.report(expression.span());
            }
            return;
        }
        if self.rule == "use-closest-build-context"
            && let Expr::Ident(identifier) = expression
            && let Some((scope, closest)) = self.contexts.iter().rev().find(|(span, name)| {
                span.contains(expression.span())
                    && is_context(&state.environment.declared_type(name))
            })
            && identifier.name() != *closest
            && !self
                .contexts
                .iter()
                .any(|(span, name)| span == scope && *name == identifier.name())
            && is_context(&state.infer(expression))
        {
            self.report(expression.span());
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>, _state: &SemanticState<'a>) {
        if let Stmt::LocalFunc(function) = statement {
            self.closures.push(statement.span());
            for parameter in function.params().params() {
                self.contexts
                    .push((statement.span(), parameter.name().name()));
            }
        }
    }
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::FuncExpr(function) = expression {
            self.closures.push(expression.span());
            for parameter in function.params().params() {
                self.contexts
                    .push((expression.span(), parameter.name().name()));
            }
        }
        if matches!(
            self.rule,
            "avoid-late-context" | "use-closest-build-context"
        ) {
            self.check_context_reference(expression, state);
            return;
        }
        let Expr::Call(call) = expression else {
            return;
        };
        let setstate = is_setstate(call, state);
        if self.rule == "avoid-empty-setstate" {
            if setstate
                && let Some(Expr::FuncExpr(callback)) =
                    call.args().positional().next().map(Expr::unparenthesized)
                && let FunctionBody::Block(block) = callback.body()
                && block
                    .stmts()
                    .all(|statement| matches!(statement, Stmt::Empty(_)))
            {
                self.report(call.callee().span());
            }
            return;
        }
        let Some(index) = self
            .methods
            .iter()
            .position(|method| method.declaration.span().contains(expression.span()))
        else {
            return;
        };
        if !self.methods[index].state
            || self.closures.iter().any(|span| {
                span.contains(expression.span())
                    && self.methods[index].declaration.span().contains(*span)
            })
        {
            return;
        }
        let name = match call.callee().unparenthesized() {
            Expr::Ident(name) if !state.environment.is_bound(name.name()) => Some(name.name()),
            Expr::Field(field) if matches!(field.object(), Expr::This(_)) => {
                Some(field.field().name())
            }
            Expr::Field(field)
                if matches!(field.object(), Expr::Super(_))
                    && field.field().name() == "dispose" =>
            {
                self.methods[index].super_dispose = true;
                None
            }
            _ => None,
        };
        if let Some(name) = name {
            self.methods[index].calls.push((name, call.callee().span()));
        }
        if self.rule == "avoid-unnecessary-setstate" && setstate {
            self.methods[index].hazard = true;
            if matches!(
                self.methods[index].declaration.name().name(),
                "build" | "initState" | "didUpdateWidget"
            ) {
                self.report(call.callee().span());
            }
        }
        if self.rule == "avoid-inherited-widget-in-initstate" {
            let callee = match call.callee().unparenthesized() {
                Expr::GenericInstantiation(generic) => generic.target(),
                callee => callee,
            };
            if let Expr::Field(field) = callee {
                let inherited = field.field().name() == "dependOnInheritedWidgetOfExactType"
                    && context_expression(field.object(), state);
                let framework_of = matches!(field.field().name(), "of" | "maybeOf")
                    && self.framework_subscription(field.object(), state);
                if inherited || framework_of {
                    self.methods[index].hazard = true;
                    if self.methods[index].declaration.name().name() == "initState" {
                        self.report(call.callee().span());
                    }
                }
            }
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        if self.rule == "require-super-dispose" {
            let spans: Vec<_> = self
                .methods
                .iter()
                .filter(|method| {
                    method.state
                        && method.declaration.name().name() == "dispose"
                        && method.declaration.body().is_some()
                        && !method.super_dispose
                })
                .map(|method| method.declaration.name().span())
                .collect();
            for span in spans {
                self.report(span);
            }
        }
        if self.rule == "avoid-late-context" {
            let spans: Vec<_> = self
                .late
                .iter()
                .filter(|(_, typed)| *typed)
                .map(|(span, _)| *span)
                .collect();
            for span in spans {
                if !self.diags.iter().any(|diagnostic| {
                    span.contains(Span::new(diagnostic.span.start, diagnostic.span.end))
                }) {
                    self.report(span);
                }
            }
        }
        if matches!(
            self.rule,
            "avoid-unnecessary-setstate" | "avoid-inherited-widget-in-initstate"
        ) {
            loop {
                let mut changed = false;
                for index in 0..self.methods.len() {
                    if self.methods[index].hazard {
                        continue;
                    }
                    let hazard = self.methods[index].calls.iter().any(|(name, _)| {
                        self.methods.iter().any(|target| {
                            target.class == self.methods[index].class
                                && target.declaration.name().name() == *name
                                && target.declaration.async_marker() == AsyncMarker::Sync
                                && target.hazard
                        })
                    });
                    if hazard {
                        self.methods[index].hazard = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut spans = Vec::new();
            for method in &self.methods {
                let lifecycle = if self.rule == "avoid-unnecessary-setstate" {
                    matches!(
                        method.declaration.name().name(),
                        "initState" | "build" | "didUpdateWidget"
                    )
                } else {
                    method.declaration.name().name() == "initState"
                };
                if !method.state || !lifecycle {
                    continue;
                }
                for (name, span) in &method.calls {
                    if *name != "setState"
                        && self.methods.iter().any(|target| {
                            target.class == method.class
                                && target.declaration.name().name() == *name
                                && target.declaration.async_marker() == AsyncMarker::Sync
                                && target.hazard
                        })
                    {
                        spans.push(*span);
                    }
                }
            }
            for span in spans {
                self.report(span);
            }
        }
        self.diags.sort_by_key(|diagnostic| diagnostic.span.start);
        self.diags
    }
}
