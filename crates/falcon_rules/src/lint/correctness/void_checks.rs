//! Validate values flowing into generic types instantiated with `void`.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ResolvedSignature, ResolvedType, Rule, TypeParameterId, TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{
    SemanticRuleVisitor, SemanticState, returned_expressions, visit_program,
};

pub struct VoidChecks;

impl Rule for VoidChecks {
    fn name(&self) -> &'static str {
        "void-checks"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector {
    diagnostics: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn report(&mut self, expression: &Expr) {
        let span = expression.span();
        self.diagnostics.push(Diagnostic::new(
            "void-checks",
            Severity::Warning,
            "This value is not valid for a generic type instantiated with void.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn check_value(
        &mut self,
        original: &ResolvedType,
        expected: &ResolvedType,
        value: &Expr,
        state: &SemanticState<'_>,
    ) {
        if !contains_type_parameter(original) {
            return;
        }
        if let (ResolvedType::Function { return_type, .. }, Expr::FuncExpr { body, .. }) =
            (expected, value)
            && state.model.void_context(return_type)
        {
            for expression in returned_expressions(body) {
                let actual = state.infer(&expression);
                if state
                    .model
                    .acceptable_for_void_context(return_type, &actual)
                    == TypeTruth::No
                {
                    self.report(&expression);
                }
            }
            return;
        }
        if matches!(expected, ResolvedType::Function { .. }) || !state.model.void_context(expected)
        {
            return;
        }
        let actual = state.infer(value);
        if state.model.acceptable_for_void_context(expected, &actual) == TypeTruth::No {
            self.report(value);
        }
    }

    fn substitutions(
        &self,
        signature: &ResolvedSignature,
        owner_arguments: &[ResolvedType],
        call_arguments: &[DartType],
        state: &SemanticState<'_>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
        let mut substitutions = HashMap::new();
        for (parameter, argument) in signature.owner_parameters.iter().zip(owner_arguments) {
            substitutions.insert(parameter.clone(), argument.clone());
        }
        for (parameter, argument) in signature.call_parameters.iter().zip(call_arguments) {
            substitutions.insert(
                parameter.clone(),
                state
                    .model
                    .resolve_type_in(argument, &state.type_parameters),
            );
        }
        substitutions
    }

    fn instantiated_type_literal(
        &self,
        expression: &Expr,
        state: &SemanticState<'_>,
    ) -> Option<ResolvedType> {
        let Expr::GenericInstantiation {
            target, type_args, ..
        } = expression
        else {
            return None;
        };
        let segments = match target.as_ref() {
            Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                vec![identifier.name.clone()]
            }
            Expr::Field { object, field, .. } => {
                let Expr::Ident(prefix) = object.as_ref() else {
                    return None;
                };
                if state.environment.is_bound(&prefix.name) {
                    return None;
                }
                vec![prefix.name.clone(), field.name.clone()]
            }
            _ => return None,
        };
        let identity = state.model.resolve_name(&segments)?;
        Some(ResolvedType::Interface {
            identity,
            arguments: type_args
                .iter()
                .map(|argument| {
                    state
                        .model
                        .resolve_type_in(argument, &state.type_parameters)
                })
                .collect(),
            nullable: false,
            extension_type: false,
        })
    }

    fn check_arguments(
        &mut self,
        signature: &ResolvedSignature,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        args: &ArgList,
        state: &SemanticState<'_>,
    ) {
        for (argument, original) in args.positional.iter().zip(&signature.positional) {
            let expected = state.model.substitute(original, substitutions);
            self.check_value(original, &expected, argument, state);
        }
        for argument in &args.named {
            if let Some(original) = signature.named.get(&argument.name.name) {
                let expected = state.model.substitute(original, substitutions);
                self.check_value(original, &expected, &argument.value, state);
            }
        }
    }

    fn check_call(
        &mut self,
        callee: &Expr,
        type_args: &[DartType],
        args: &ArgList,
        state: &SemanticState<'_>,
    ) {
        match callee {
            Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                if let Some(identity) = state
                    .model
                    .resolve_value(std::slice::from_ref(&identifier.name))
                    && let Some(signature) = state.signatures.function(&identity).cloned()
                {
                    let substitutions = self.substitutions(&signature, &[], type_args, state);
                    self.check_arguments(&signature, &substitutions, args, state);
                    return;
                }
                if let Some(identity) = state
                    .model
                    .resolve_name(std::slice::from_ref(&identifier.name))
                    && let Some(signature) = state.signatures.constructor(&identity, "new").cloned()
                {
                    let owner_arguments: Vec<_> = type_args
                        .iter()
                        .map(|argument| {
                            state
                                .model
                                .resolve_type_in(argument, &state.type_parameters)
                        })
                        .collect();
                    let substitutions =
                        self.substitutions(&signature, &owner_arguments, &[], state);
                    self.check_arguments(&signature, &substitutions, args, state);
                }
            }
            Expr::Field { object, field, .. } => {
                if let Some(receiver) = self.instantiated_type_literal(object, state)
                    && let Some((signature, substitutions)) = state
                        .signatures
                        .resolved_constructor(&receiver, &field.name)
                {
                    self.check_arguments(&signature, &substitutions, args, state);
                    return;
                }
                let receiver = state.infer(object);
                if let Some((signature, mut substitutions)) =
                    state
                        .signatures
                        .resolved_member(&receiver, &field.name, &state.model)
                {
                    substitutions.extend(signature.call_parameters.iter().zip(type_args).map(
                        |(parameter, argument)| {
                            (
                                parameter.clone(),
                                state
                                    .model
                                    .resolve_type_in(argument, &state.type_parameters),
                            )
                        },
                    ));
                    self.check_arguments(&signature, &substitutions, args, state);
                }
            }
            _ => {}
        }
    }
}

impl SemanticRuleVisitor for Collector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if !matches!(node, Expr::FuncExpr { .. })
            && let Some(ResolvedType::Function { return_type, .. }) = state.expected()
            && state.model.void_context(return_type)
            && let ResolvedType::Function {
                return_type: actual_return,
                ..
            } = state.infer(node)
            && state
                .model
                .acceptable_for_void_context(return_type, &actual_return)
                == TypeTruth::No
        {
            self.report(node);
        }
        match node {
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                if let Expr::GenericInstantiation {
                    target,
                    type_args: instantiated,
                    ..
                } = callee.as_ref()
                {
                    self.check_call(target, instantiated, args, state);
                } else {
                    self.check_call(callee, type_args, args, state);
                }
            }
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                let resolved = state
                    .model
                    .resolve_type_in(dart_type, &state.type_parameters);
                if let ResolvedType::Interface {
                    identity,
                    arguments,
                    ..
                } = resolved
                    && let Some(signature) = state
                        .signatures
                        .constructor(
                            &identity,
                            constructor_name
                                .as_ref()
                                .map(|name| name.name.as_str())
                                .unwrap_or("new"),
                        )
                        .cloned()
                {
                    let substitutions = self.substitutions(&signature, &arguments, &[], state);
                    self.check_arguments(&signature, &substitutions, args, state);
                }
            }
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } => match target.as_ref() {
                Expr::Field { object, field, .. } => {
                    let receiver = state.infer(object);
                    if let Some((original, substitutions)) =
                        state
                            .signatures
                            .resolved_field(&receiver, &field.name, &state.model)
                    {
                        let expected = state.model.substitute(&original, &substitutions);
                        self.check_value(&original, &expected, value, state);
                    }
                }
                Expr::Index { object, .. } => {
                    let receiver = state.infer(object);
                    if let Some((signature, substitutions)) =
                        state
                            .signatures
                            .resolved_member(&receiver, "[]=", &state.model)
                        && let Some(original) = signature.positional.get(1)
                    {
                        let expected = state.model.substitute(original, &substitutions);
                        self.check_value(original, &expected, value, state);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn contains_type_parameter(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::TypeParameter { .. } => true,
        ResolvedType::Interface { arguments, .. } => arguments.iter().any(contains_type_parameter),
        ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } => {
            contains_type_parameter(return_type)
                || positional.iter().any(contains_type_parameter)
                || named.iter().any(|(_, ty)| contains_type_parameter(ty))
        }
        ResolvedType::Record {
            positional, named, ..
        } => {
            positional.iter().any(contains_type_parameter)
                || named.iter().any(|(_, ty)| contains_type_parameter(ty))
        }
        _ => false,
    }
}
