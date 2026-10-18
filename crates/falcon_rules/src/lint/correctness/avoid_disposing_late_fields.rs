//! Avoid releasing late State fields from dispose: they may be unset or lazily
//! initialized solely to release them. Nullable fields permit conditional cleanup.

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
            };
            for member in members {
                match member {
                    ClassMember::Field(field) if field.is_late() && !field.is_static() => {
                        owner
                            .fields
                            .extend(field.declarators().map(|field| field.name().name()));
                    }
                    ClassMember::Method(method)
                        if method.name().name() == "dispose" && !method.is_static() =>
                    {
                        owner.methods.push(method.span());
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

struct Owner<'a> {
    fields: Vec<&'a str>,
    methods: Vec<Span>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    owners: Vec<Owner<'a>>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn late_field(&self, receiver: Expr<'_>, span: Span, state: &SemanticState<'_>) -> bool {
        let mut receiver = receiver.unparenthesized();
        while let Expr::NullAssert(assertion) = receiver {
            receiver = assertion.operand().unparenthesized();
        }
        let name = match receiver {
            Expr::Ident(name) if state.is_own_field(name.name()) => name.name(),
            Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
                field.field().name()
            }
            _ => return false,
        };
        self.owners.iter().any(|owner| {
            owner.fields.contains(&name)
                && owner
                    .methods
                    .iter()
                    .any(|method| method.start <= span.start && span.end <= method.end)
        })
    }

    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-disposing-late-fields",
            "Avoid releasing a late field in dispose; make the field nullable instead.",
            span,
        ));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        match expression {
            Expr::Call(call) => {
                if let Expr::Field(field) = call.callee().unparenthesized()
                    && matches!(field.field().name(), "dispose" | "close" | "cancel")
                    && self.late_field(field.object(), expression.span(), state)
                {
                    self.report(expression.span());
                }
            }
            Expr::Cascade(cascade)
                if self.late_field(cascade.object(), expression.span(), state) =>
            {
                for section in cascade.sections() {
                    if let Some(CascadeOp::Call(call)) = section.ops().first()
                        && matches!(call.name().name(), "dispose" | "close" | "cancel")
                    {
                        self.report(call.span());
                    }
                }
            }
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
