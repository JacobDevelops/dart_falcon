//! Avoid `Type.toString()` in production code, where type names may be minified.
//!
//! Web and AOT builds may minify or strip type names, so a string built from a
//! `Type` can differ between debug and release:
//!
//! ```dart
//! final name = value.runtimeType.toString(); // BAD
//! if (value is Widget) {} // GOOD: compare types, not their names
//! ```
//!
//! Reported for `toString()` calls and `toString` tear-offs passed as
//! arguments whose receiver (or implicit `this`, inside a class, mixin, or
//! extension) is provably a subtype of `dart:core` `Type` and still uses
//! `Object.toString`. `runtimeType` getters are typed as `Type`; a project type
//! implementing `Type` is only reported when its whole supertype chain resolves
//! and no project declaration in it overrides `toString`.
//!
//! Adopted from the official `avoid_type_to_string` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::function_bodies::contains;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidTypeToString;

impl Rule for AvoidTypeToString {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let extensions = program
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                TopLevelDecl::Extension(extension) => Some((extension.span, &extension.on_type)),
                _ => None,
            })
            .collect();
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            extensions,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a, 'p> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Extension declarations and their `on` types, for implicit `this`.
    extensions: Vec<(Span, &'p DartType)>,
}

impl Collector<'_, '_> {
    /// The type of implicit `this` at `span`.
    fn this_type(&self, span: &Span, state: &SemanticState<'_>) -> ResolvedType {
        match &state.enclosing_type {
            Some(ty @ ResolvedType::Interface { .. }) => ty.clone(),
            _ => self
                .extensions
                .iter()
                .find(|(extension, _)| contains(extension, span))
                .map_or(ResolvedType::Unknown, |(_, on_type)| {
                    state.model.resolve_type_in(on_type, &state.type_parameters)
                }),
        }
    }

    /// The static type of an explicit receiver; `runtimeType` reads are `Type`.
    fn receiver_type(&self, receiver: &Expr, state: &SemanticState<'_>) -> ResolvedType {
        match receiver {
            Expr::This { span } | Expr::Super { span } => self.this_type(span, state),
            Expr::Field { field, .. } if field.name == "runtimeType" => {
                state.model.core_type("Type", false)
            }
            Expr::Ident(identifier)
                if identifier.name == "runtimeType"
                    && !state.environment.is_bound(&identifier.name) =>
            {
                state.model.core_type("Type", false)
            }
            other => state.infer(other),
        }
    }

    fn check(&mut self, target: &ResolvedType, name: &Identifier, state: &SemanticState<'_>) {
        if name.name == "toString" && is_type_using_object_to_string(target, state) {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-type-to-string",
                "Using 'toString' on a 'Type' is not safe in production code.",
                name.span,
            ));
        }
    }

    fn check_arguments(&mut self, args: &ArgList, state: &SemanticState<'_>) {
        for argument in args
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value))
        {
            match argument {
                Expr::Field { object, field, .. } => {
                    let target = self.receiver_type(object, state);
                    self.check(&target, field, state);
                }
                Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                    let target = self.this_type(&identifier.span, state);
                    self.check(&target, identifier, state);
                }
                _ => {}
            }
        }
    }
}

impl SemanticRuleVisitor for Collector<'_, '_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Call { callee, args, .. } => {
                match callee.as_ref() {
                    Expr::Field { object, field, .. } => {
                        let target = self.receiver_type(object, state);
                        self.check(&target, field, state);
                    }
                    Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                        let target = self.this_type(&identifier.span, state);
                        self.check(&target, identifier, state);
                    }
                    _ => {}
                }
                self.check_arguments(args, state);
            }
            Expr::New { args, .. } => self.check_arguments(args, state),
            _ => {}
        }
    }
}

/// Whether `ty` is provably a `Type` whose `toString` is `Object.toString`.
fn is_type_using_object_to_string(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    let ResolvedType::Interface { identity, .. } = ty else {
        return false;
    };
    if state.signatures.implements_sdk(ty, "dart:core", "Type") != Truth::Yes {
        return false;
    }
    if identity.is_sdk("dart:core", "Type") {
        return true;
    }
    let owner_is_object = state
        .signatures
        .resolved_member_owner(ty, "toString", &state.model)
        .is_some_and(|owner| owner == DeclarationIdentity::OBJECT);
    // Abstract redeclarations (such as `Type.toString`) leave the concrete
    // implementation, which is what the official rule looks up, unchanged.
    owner_is_object
        && state
            .signatures
            .inherited_member_facts(identity, "toString")
            .is_some_and(|facts| {
                facts
                    .iter()
                    .filter(|fact| !fact.is_abstract)
                    .all(|fact| fact.declaring_type == DeclarationIdentity::OBJECT)
            })
}
