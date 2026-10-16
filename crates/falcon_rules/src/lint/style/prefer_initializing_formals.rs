//! Prefer initializing formals when a parameter is copied directly into its field.
//!
//! An initializing formal makes field initialization explicit in the constructor
//! signature: replace `Example(int value) : value = value;` with
//! `Example(this.value);`. The rule also recognizes direct assignments in a
//! constructor body, including primary constructors.
//!
//! Suggestions preserve the parameter's name and require its type to be a subtype
//! of the field's type. Inherited fields, setters, static fields, and parameters
//! referenced elsewhere in the constructor are excluded. Private fields are
//! suggested only for named parameters when private named parameters are available.

use std::collections::HashMap;

use falcon_analyze::resolve::MemberKind;
use falcon_analyze::{AnalyzeContext, Feature, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferInitializingFormals;

impl Rule for PreferInitializingFormals {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            private_named: ctx
                .language_version()
                .supports(Feature::PrivateNamedParameters),
            references: HashMap::new(),
            candidates: Vec::new(),
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    private_named: bool,
    references: HashMap<Span, Vec<usize>>,
    candidates: Vec<Candidate>,
}

struct Candidate {
    parameter: Span,
    reference: usize,
    anchor: Span,
    regions: [Option<Span>; 2],
}

impl Collector<'_> {
    fn check(
        &mut self,
        field: &Identifier,
        value: &Expr,
        anchor: Span,
        ctor: ConstructorRef<'_>,
        state: &SemanticState<'_>,
    ) {
        let Expr::Ident(value) = value else {
            return;
        };
        let Some(parameter) = ctor.params.params.iter().find(|parameter| {
            parameter.name.name == value.name
                && parameter.initializing.is_none()
                && !(ctor.is_primary && (parameter.is_final || parameter.is_var))
        }) else {
            return;
        };
        if field.name.starts_with('_') {
            if !self.private_named
                || !matches!(parameter.kind, ParamKind::Named { .. })
                || (field.name != parameter.name.name
                    && field.name.strip_prefix('_') != Some(parameter.name.name.as_str()))
            {
                return;
            }
        } else if field.name != parameter.name.name {
            return;
        }
        let Some(receiver @ ResolvedType::Interface { identity, .. }) = &state.enclosing_type
        else {
            return;
        };
        if !state
            .signatures
            .declared_member_facts(identity, &field.name)
            .iter()
            .any(|fact| !fact.is_static && matches!(fact.kind, MemberKind::Field { .. }))
        {
            return;
        }
        let Some((field_type, substitutions)) =
            state
                .signatures
                .resolved_field(receiver, &field.name, &state.model)
        else {
            return;
        };
        let field_type = state.model.substitute(&field_type, &substitutions);
        let parameter_type = state.environment.declared_type(&parameter.name.name);
        if state
            .signatures
            .subtype(&parameter_type, &field_type, &state.model)
            != Truth::Yes
        {
            return;
        }
        let initializer_span = |initializer: &ConstructorInitializer| match initializer {
            ConstructorInitializer::FieldInit { span, .. }
            | ConstructorInitializer::SuperCall { span, .. }
            | ConstructorInitializer::ThisCall { span, .. }
            | ConstructorInitializer::Assert { span, .. } => *span,
        };
        let initializers =
            ctor.initializers
                .first()
                .zip(ctor.initializers.last())
                .map(|(first, last)| {
                    Span::new(initializer_span(first).start, initializer_span(last).end)
                });
        let regions = [initializers, ctor.body.map(|body| *body.span())];
        self.references.entry(parameter.name.span).or_default();
        self.candidates.push(Candidate {
            parameter: parameter.name.span,
            reference: value.span.start,
            anchor,
            regions,
        });
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let collector = *self;
        let ctx = collector.ctx;
        collector
            .candidates
            .into_iter()
            .filter_map(|candidate| {
                let mut references = collector
                    .references
                    .get(&candidate.parameter)?
                    .iter()
                    .filter(|offset| {
                        candidate
                            .regions
                            .iter()
                            .flatten()
                            .any(|region| region.start <= **offset && **offset < region.end)
                    });
                (references.next() == Some(&candidate.reference) && references.next().is_none())
                    .then(|| {
                        ctx.diagnostic(
                            "prefer-initializing-formals",
                            "Use an initializing formal to assign a parameter to a field.",
                            candidate.anchor,
                        )
                    })
            })
            .collect()
    }
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Ident(identifier) = node
            && let Some(binding) = state.environment.binding_declaration(&identifier.name)
            && let Some(references) = self.references.get_mut(&binding)
        {
            references.push(identifier.span.start);
        }
    }

    fn visit_constructor_ref(&mut self, ctor: ConstructorRef<'_>, state: &SemanticState<'_>) {
        if ctor.is_factory {
            return;
        }
        for initializer in ctor.initializers {
            if let ConstructorInitializer::FieldInit {
                field, value, span, ..
            } = initializer
            {
                self.check(field, value, *span, ctor, state);
            }
        }
        if let Some(FunctionBody::Block(block)) = ctor.body {
            for statement in &block.stmts {
                if let Stmt::Expr(statement) = statement
                    && let Expr::Assign {
                        target,
                        value,
                        span,
                        ..
                    } = &statement.expr
                    && let Expr::Field { object, field, .. } = target.as_ref()
                    && matches!(object.as_ref(), Expr::This { .. })
                {
                    self.check(field, value.unparenthesized(), *span, ctor, state);
                }
            }
        }
    }
}
