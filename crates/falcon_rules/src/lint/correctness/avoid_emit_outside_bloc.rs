//! Flags external calls and tear-offs of a Bloc or Cubit's `emit` method.
//!
//! External code should dispatch Bloc events or call a Cubit's public domain
//! methods instead of bypassing its state-transition logic. Receivers must be
//! confirmed through `bloc`/`flutter_bloc` package identity or an imported API
//! superclass. Ordinary and null-aware method calls, tear-offs and cascade
//! sections are checked. Emission inside the owning class through `this`,
//! `super`, or a receiver of that owner type is allowed; extensions are outside
//! the owner. Unknown/dynamic receivers and unrelated `emit` methods are not
//! reported; no class-name suffix is assumed.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::bloc::BlocTypes;

#[derive(Default)]
pub struct AvoidEmitOutsideBloc;

impl Rule for AvoidEmitOutsideBloc {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            types: BlocTypes::new(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    types: BlocTypes<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn check(
        &mut self,
        receiver: &ResolvedType,
        name: &str,
        span: Span,
        package_parameter: bool,
        state: &SemanticState<'_>,
    ) {
        if name != "emit"
            || !(package_parameter
                || self
                    .types
                    .is_bloc_type(receiver, &state.model, Some(&state.signatures)))
        {
            return;
        }
        if let ResolvedType::Interface { identity, .. } = receiver
            && state.constant_owner.as_ref() == Some(identity)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("Do not emit state outside its Bloc or Cubit; use its events or public methods.", span));
    }

    fn package_extension(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
        expression
            .syntax()
            .ancestors()
            .find_map(ExtensionDecl::cast)
            .is_some_and(|extension| {
                self.types
                    .is_api_syntax(extension.on_type(), &state.model, "Bloc")
                    || self
                        .types
                        .is_api_syntax(extension.on_type(), &state.model, "Cubit")
            })
    }

    fn package_receiver(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
        if matches!(expression.unparenthesized(), Expr::This(_)) {
            self.package_extension(expression, state)
        } else {
            self.types
                .binding_has_api_syntax(expression, state, &["Bloc", "Cubit"])
        }
    }

    fn extension_emit(expression: Expr<'a>) -> bool {
        expression
            .syntax()
            .ancestors()
            .find_map(ExtensionDecl::cast)
            .is_some_and(|extension| {
                extension.members().any(|member| match member {
                    ClassMember::Method(method) => method.name().name() == "emit",
                    ClassMember::Getter(getter) => getter.name().name() == "emit",
                    ClassMember::Setter(setter) => setter.name().name() == "emit",
                    ClassMember::Field(field) => field
                        .declarators()
                        .any(|field| field.name().name() == "emit"),
                    _ => false,
                })
            })
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        match expression {
            Expr::Field(field)
                if field.field().name() == "emit"
                    && !matches!(field.object().unparenthesized(), Expr::Super(_)) =>
            {
                self.check(
                    &state.infer_receiver(field.object()),
                    field.field().name(),
                    field.field().span(),
                    self.package_receiver(field.object(), state),
                    state,
                );
            }
            Expr::Ident(name)
                if name.name() == "emit"
                    && !state.environment.is_bound(name.name())
                    && state.model.resolve_value(name.name()).is_none()
                    && !Self::extension_emit(expression) =>
            {
                if let Some(receiver) = &state.enclosing_type {
                    self.check(
                        receiver,
                        name.name(),
                        name.span(),
                        self.package_extension(expression, state),
                        state,
                    );
                }
            }
            _ => {}
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        let package_receiver = operation
            .syntax()
            .ancestors()
            .find_map(CascadeSection::cast)
            .filter(|section| {
                section
                    .ops()
                    .next()
                    .is_some_and(|first| first.syntax() == operation.syntax())
            })
            .and_then(|section| section.syntax().ancestors().find_map(CascadeExpr::cast))
            .is_some_and(|cascade| self.package_receiver(cascade.object(), state));
        match operation {
            CascadeOp::Call(call) => {
                self.check(
                    receiver,
                    call.name().name(),
                    call.name().span(),
                    package_receiver,
                    state,
                );
            }
            CascadeOp::Field(field) => {
                self.check(
                    receiver,
                    field.name().name(),
                    field.name().span(),
                    package_receiver,
                    state,
                );
            }
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
