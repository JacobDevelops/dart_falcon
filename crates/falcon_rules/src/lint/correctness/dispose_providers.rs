//! Requires cleanup callbacks for disposable values created by package:provider's Provider.
//!
//! The created value's inferred type must declare or inherit an instance method
//! named `dispose`, `close`, or `cancel`. Each such method must be called on the
//! dispose callback's value parameter, not another object or a shadowing local.
//! Value constructors do not own their values. Creation is proven only for direct
//! generative constructors or simple same-file function tear-offs returning one.
//! Unknown factories, non-inline cleanup, and path-sensitive guarantees are outside
//! this rule's scope.

use std::collections::HashSet;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, MemberKind, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::{constructor_span, scoped_constructor_invocation};
use crate::lint::function_bodies::FunctionBodies;

const CLEANUP: [&str; 3] = ["dispose", "close", "cancel"];

#[derive(Default)]
pub struct DisposeProviders;

impl Rule for DisposeProviders {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            program,
            bodies: FunctionBodies::new(program),
            mutated: ctx.mutated_declarations(program),
            providers: Vec::new(),
        }))
    }
}

struct Provider<'a> {
    span: Span,
    callback: Option<(Span, Identifier<'a>)>,
    required: [bool; 3],
    disposed: [bool; 3],
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    bodies: FunctionBodies<'a>,
    mutated: &'a HashSet<NodeId>,
    providers: Vec<Provider<'a>>,
}

impl<'a> Collector<'a> {
    fn provider(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, constructor, args)) = scoped_constructor_invocation(expression, state)
        else {
            return;
        };
        if constructor != "new"
            || identity.name() != "Provider"
            || !(identity.is_package("provider", "Provider")
                || state
                    .model
                    .declaration_uri(&identity)
                    .is_some_and(|uri| uri.starts_with("package:provider/")))
        {
            return;
        }
        let Some(create) = args.named().find(|arg| arg.name().name() == "create") else {
            return;
        };
        let return_type = created_type(create.value(), state, self.program).with_nullable(false);
        let required = CLEANUP.map(|name| {
            state
                .signatures
                .resolved_member_facts(&return_type, name, &state.model)
                .is_some_and(|facts| {
                    facts
                        .iter()
                        .any(|fact| !fact.is_static && fact.kind == MemberKind::Method)
                })
        });
        if !required.iter().any(|required| *required) {
            return;
        }
        let callback = match args.named().find(|arg| arg.name().name() == "dispose") {
            None => None,
            Some(arg) => match arg.value().unparenthesized() {
                Expr::NullLit(_) => None,
                Expr::FuncExpr(function) => {
                    let mut params = function.params().positional();
                    let (Some(_context), Some(value), None) =
                        (params.next(), params.next(), params.next())
                    else {
                        return;
                    };
                    let name = value.name();
                    if self.mutated.contains(&name.syntax().id()) {
                        return;
                    }
                    Some((function.body().span(), name))
                }
                _ => return,
            },
        };
        self.providers.push(Provider {
            span: constructor_span(expression),
            callback,
            required,
            disposed: [false; 3],
        });
    }

    fn cleanup(
        &mut self,
        receiver: Expr<'a>,
        method: &str,
        call_span: Span,
        state: &SemanticState<'a>,
    ) {
        let mut receiver = receiver;
        receiver = loop {
            receiver = match receiver.unparenthesized() {
                Expr::NullAssert(assertion) => assertion.operand(),
                Expr::As(cast) => cast.expr(),
                other => break other,
            };
        };
        let Some(index) = CLEANUP.iter().position(|name| *name == method) else {
            return;
        };
        let Expr::Ident(receiver) = receiver.unparenthesized() else {
            return;
        };
        let Some(declaration) = state.environment.binding_declaration(receiver.name()) else {
            return;
        };
        for provider in &mut self.providers {
            let Some((body, value)) = provider.callback else {
                continue;
            };
            if declaration == value.span()
                && body.contains(call_span)
                && self
                    .bodies
                    .enclosing(&call_span)
                    .is_some_and(|info| info.span == body)
            {
                provider.disposed[index] = true;
            }
        }
    }
}

fn returned_expression(body: FunctionBody<'_>) -> Option<Expr<'_>> {
    match body {
        FunctionBody::Arrow(arrow) => Some(arrow.expr()),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            match (statements.next(), statements.next()) {
                (Some(Stmt::Return(statement)), None) => statement.value(),
                _ => None,
            }
        }
        FunctionBody::Native(_) => None,
    }
}

fn created_type<'a>(
    create: Expr<'a>,
    state: &SemanticState<'a>,
    program: Program<'a>,
) -> ResolvedType {
    let returned = match create.unparenthesized() {
        Expr::FuncExpr(function) => {
            let Some(returned) = returned_expression(function.body()) else {
                return ResolvedType::Unknown;
            };
            let head = match returned.unparenthesized() {
                Expr::Call(call) => falcon_analyze::expression_segments(call.callee(), true)
                    .and_then(|segments| segments.first().copied()),
                _ => None,
            };
            if head.is_some_and(|head| function.params().params().any(|p| p.name().name() == head))
            {
                return ResolvedType::Unknown;
            }
            returned
        }
        Expr::Ident(identifier) if !state.environment.is_bound(identifier.name()) => {
            let Some(function) = program
                .declarations()
                .find_map(|declaration| match declaration {
                    TopLevelDecl::Function(function)
                        if function.name().name() == identifier.name() =>
                    {
                        Some(function)
                    }
                    _ => None,
                })
            else {
                return ResolvedType::Unknown;
            };
            let Some(returned) = function.body().and_then(returned_expression) else {
                return ResolvedType::Unknown;
            };
            let head = match returned.unparenthesized() {
                Expr::Call(call) => falcon_analyze::expression_segments(call.callee(), true)
                    .and_then(|segments| segments.first().copied()),
                _ => None,
            };
            if head.is_some_and(|head| {
                function
                    .params()
                    .is_some_and(|params| params.params().any(|p| p.name().name() == head))
            }) {
                return ResolvedType::Unknown;
            }
            returned
        }
        _ => return ResolvedType::Unknown,
    };
    let Some((identity, constructor, _)) =
        scoped_constructor_invocation(returned.unparenthesized(), state)
    else {
        return ResolvedType::Unknown;
    };
    if !state.signatures.has_constructor(&identity, constructor)
        || state
            .signatures
            .declaration(&identity)
            .is_some_and(|facts| {
                facts
                    .constructors
                    .iter()
                    .any(|fact| fact.name == constructor && fact.is_factory)
            })
    {
        return ResolvedType::Unknown;
    }
    match state.infer(create) {
        ResolvedType::Function { return_type, .. } => (*return_type).clone(),
        _ => ResolvedType::Unknown,
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.providers
            .iter()
            .filter(|provider| {
                provider
                    .required
                    .iter()
                    .zip(provider.disposed)
                    .any(|(required, disposed)| *required && !disposed)
            })
            .map(|provider| {
                self.ctx.diagnostic("Dispose the created value in this `Provider`'s `dispose` callback.", provider.span)
            })
            .collect()
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        self.provider(node, state);
        match node {
            Expr::Call(call) => {
                if let Expr::Field(field) = call.callee().unparenthesized() {
                    self.cleanup(field.object(), field.field().name(), call.span(), state);
                }
            }
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    if let Some(CascadeOp::Call(call)) = section.ops().next() {
                        self.cleanup(cascade.object(), call.name().name(), node.span(), state);
                    }
                }
            }
            _ => {}
        }
    }
}
