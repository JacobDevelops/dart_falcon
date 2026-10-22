//! Avoid releasing late State fields that may be unset or initialized only by cleanup.
//!
//! A direct `=` assignment, or a direct read of a lazily initialized field, in
//! synchronous initState is accepted in its initial straight-line prefix of
//! expression statements, local declarations and empty statements. Any other
//! statement ends that prefix, including assertions and pattern declarations.
//! Arrow bodies are supported. Reads cover bare fields, property receivers,
//! method receivers and cascade receivers, not nested arguments or callbacks.
//! Conditional, nested, asynchronous and helper-based initialization is not proven;
//! lazy field initializers alone still do not make cleanup safe.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::flutter_state_rules::is_state;

#[derive(Default)]
pub struct AvoidDisposingLateFields;

impl Rule for AvoidDisposingLateFields {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let mut owners = Vec::new();
        for declaration in program.declarations() {
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) => (class.name().name(), class.members()),
                TopLevelDecl::MixinClass(class) => (class.name().name(), class.members()),
                _ => continue,
            };
            if !state.model.resolve_name(name).is_some_and(|identity| {
                is_state(&ResolvedType::interface_identity(identity), &state)
            }) {
                continue;
            }
            let mut owner = Owner {
                fields: Vec::new(),
                methods: Vec::new(),
                initialization: Vec::new(),
            };
            for member in members {
                match member {
                    ClassMember::Field(field) if field.is_late() && !field.is_static() => {
                        owner
                            .fields
                            .extend(field.declarators().map(|field| LateField {
                                name: field.name().name(),
                                initialized: false,
                                lazy: field.initializer().is_some(),
                            }));
                    }
                    ClassMember::Method(method)
                        if method.name().name() == "dispose" && !method.is_static() =>
                    {
                        owner.methods.push(method.span());
                    }
                    ClassMember::Method(method)
                        if method.name().name() == "initState"
                            && !method.is_static()
                            && method.async_marker() == AsyncMarker::Sync
                            && method.params().params().next().is_none() =>
                    {
                        match method.body() {
                            Some(FunctionBody::Arrow(arrow)) => {
                                owner
                                    .initialization
                                    .push(arrow.expr().unparenthesized().span());
                            }
                            Some(FunctionBody::Block(block)) => {
                                for statement in block.stmts() {
                                    match statement {
                                        Stmt::Expr(expression) => owner
                                            .initialization
                                            .push(expression.expr().unparenthesized().span()),
                                        Stmt::LocalVar(_) | Stmt::LocalFunc(_) | Stmt::Empty(_) => {
                                        }
                                        _ => break,
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            if !owner.fields.is_empty() && !owner.methods.is_empty() {
                owners.push(owner);
            }
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            owners,
            diagnostics: Vec::new(),
        }))
    }
}

struct LateField<'a> {
    name: &'a str,
    initialized: bool,
    lazy: bool,
}

struct Owner<'a> {
    fields: Vec<LateField<'a>>,
    methods: Vec<Span>,
    initialization: Vec<Span>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    owners: Vec<Owner<'a>>,
    diagnostics: Vec<(usize, usize, Diagnostic)>,
}

fn own_field_name<'a>(expression: Expr<'a>, state: &SemanticState<'_>) -> Option<&'a str> {
    match expression.unparenthesized() {
        Expr::Ident(name) if state.is_own_field(name.name()) => Some(name.name()),
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
            Some(field.field().name())
        }
        _ => None,
    }
}

impl Collector<'_> {
    fn late_field(
        &self,
        receiver: Expr<'_>,
        span: Span,
        state: &SemanticState<'_>,
    ) -> Option<(usize, usize)> {
        let mut receiver = receiver.unparenthesized();
        while let Expr::NullAssert(assertion) = receiver {
            receiver = assertion.operand().unparenthesized();
        }
        let name = own_field_name(receiver, state)?;
        self.owners
            .iter()
            .enumerate()
            .find_map(|(owner_index, owner)| {
                if !owner
                    .methods
                    .iter()
                    .any(|method| method.start <= span.start && span.end <= method.end)
                {
                    return None;
                }
                owner
                    .fields
                    .iter()
                    .position(|field| field.name == name)
                    .map(|field_index| (owner_index, field_index))
            })
    }

    fn report(&mut self, span: Span, (owner, field): (usize, usize)) {
        self.diagnostics.push((
            owner,
            field,
            self.ctx.diagnostic("Initialize this `late` field unconditionally in `initState` before releasing it in `dispose`, or make it nullable.", span),
        ));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let assignment = match expression {
            Expr::Assign(assignment) if assignment.op() == AssignOp::Eq => {
                own_field_name(assignment.target(), state)
            }
            _ => None,
        };
        let read = match expression.unparenthesized() {
            Expr::Call(call) => match call.callee().unparenthesized() {
                Expr::Field(field) => own_field_name(field.object(), state),
                _ => None,
            },
            Expr::Cascade(cascade) => own_field_name(cascade.object(), state),
            Expr::Field(field) => {
                own_field_name(expression, state).or_else(|| own_field_name(field.object(), state))
            }
            Expr::Ident(_) => own_field_name(expression, state),
            _ => None,
        };
        for owner in &mut self.owners {
            if owner.initialization.contains(&expression.span()) {
                for field in &mut owner.fields {
                    if assignment == Some(field.name) || (field.lazy && read == Some(field.name)) {
                        field.initialized = true;
                    }
                }
            }
        }
        match expression {
            Expr::Call(call) => {
                if let Expr::Field(field) = call.callee().unparenthesized()
                    && matches!(field.field().name(), "dispose" | "close" | "cancel")
                    && let Some(late) = self.late_field(field.object(), expression.span(), state)
                {
                    self.report(expression.span(), late);
                }
            }
            Expr::Cascade(cascade) => {
                let Some(late) = self.late_field(cascade.object(), expression.span(), state) else {
                    return;
                };
                for section in cascade.sections() {
                    if let Some(CascadeOp::Call(call)) = section.ops().first()
                        && matches!(call.name().name(), "dispose" | "close" | "cancel")
                    {
                        self.report(call.span(), late);
                    }
                }
            }
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
            .into_iter()
            .filter(|(owner, field, _)| !self.owners[*owner].fields[*field].initialized)
            .map(|(_, _, diagnostic)| diagnostic)
            .collect()
    }
}
