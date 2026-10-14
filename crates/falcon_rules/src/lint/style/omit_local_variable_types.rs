//! Flags local variable type annotations that repeat the initializer's type.
//!
//! When a local's declared type is exactly the static type of its
//! initializer, `var` or `final` says the same thing with less noise. The same
//! applies to `for` loop variables, both `for (int i = 0; ...)` and
//! `for (String s in strings)` when the iterable's element type matches.
//! Annotations that change the type (`num n = 1`, `int? x = 0`,
//! `Object o = ''`) are kept, as are those that drive inference of a generic
//! call (`int x = parse()` where `parse<T>()` returns `T`).
//!
//! ```dart
//! int count = items.length; // flagged
//! var count = items.length; // ok
//! ```
//!
//! falcon reports only when both types are fully known; types with `dynamic`
//! arguments or omitted type arguments are skipped because their inferred form
//! may differ from the analyzer's. An untyped collection literal takes its
//! type arguments from the declaration (`List<int> xs = []` is flagged), but a
//! generic constructor call without type arguments (`Box<int> b = Box(1)`) is
//! not, since falcon does not model its context-driven inference.
//!
//! Adopted from the official `omit_local_variable_types` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::style::always_specify_types::known_type_arity;

#[derive(Default)]
pub struct OmitLocalVariableTypes;

impl Rule for OmitLocalVariableTypes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Whether `ty` is fully determined: no unknown or `dynamic` component and
/// every generic type carries all of its type arguments.
fn settled(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    match ty {
        ResolvedType::Unknown | ResolvedType::Dynamic => false,
        ResolvedType::Void
        | ResolvedType::Never
        | ResolvedType::Null
        | ResolvedType::TypeParameter { .. } => true,
        ResolvedType::Interface {
            identity,
            arguments,
            ..
        } => {
            let arity = known_type_arity(identity)
                .or_else(|| state.signatures.project_type_parameter_count(identity));
            arity == Some(arguments.len()) && arguments.iter().all(|arg| settled(arg, state))
        }
        ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } => {
            settled(return_type, state)
                && positional.iter().all(|arg| settled(arg, state))
                && named.iter().all(|(_, arg)| settled(arg, state))
        }
        ResolvedType::Record {
            positional, named, ..
        } => {
            positional.iter().all(|field| settled(field, state))
                && named.iter().all(|(_, field)| settled(field, state))
        }
    }
}

/// The declared type, when it is a candidate for omission at all.
fn declared_type(ty: &DartType, state: &SemanticState<'_>) -> Option<ResolvedType> {
    let declared = state.model.resolve_type_in(ty, &state.type_parameters);
    (!matches!(declared, ResolvedType::Null) && settled(&declared, state)).then_some(declared)
}

/// `f()` of a generic top-level function returning its own type parameter:
/// the declared type is what instantiates it.
fn depends_on_declared_type(initializer: &Expr, state: &SemanticState<'_>) -> bool {
    let Expr::Call {
        callee, type_args, ..
    } = initializer
    else {
        return false;
    };
    let Expr::Ident(name) = callee.as_ref() else {
        return false;
    };
    type_args.is_empty()
        && !state.environment.is_bound(&name.name)
        && state
            .model
            .resolve_value(&name.name)
            .and_then(|identity| state.signatures.function(&identity))
            .is_some_and(|signature| {
                matches!(signature.return_type, ResolvedType::TypeParameter { .. })
            })
}

/// Whether `initializer`'s static type is exactly `declared`. A collection
/// literal without type arguments takes them from its context, so
/// `List<int> xs = []` is a `List<int>`, as the analyzer infers it; an empty
/// `{}` is a set when the context is a `Set`.
fn has_static_type(initializer: &Expr, declared: &ResolvedType, state: &SemanticState<'_>) -> bool {
    let literal = match initializer {
        Expr::List { type_arg: None, .. } => "List",
        Expr::Set { type_arg: None, .. } => "Set",
        Expr::Map {
            type_args,
            elements,
            ..
        } if type_args.is_empty() => {
            if elements.is_empty() && declared.interface("dart:core", "Set") {
                "Set"
            } else {
                "Map"
            }
        }
        _ => return state.infer(initializer) == *declared,
    };
    declared.interface("dart:core", literal) && !declared.nullable()
}

impl Collector<'_> {
    fn report(&mut self, ty: &DartType) {
        self.diags.push(self.ctx.diagnostic(
            "omit-local-variable-types",
            "Unnecessary type annotation on a local variable.",
            *ty.span(),
        ));
    }

    fn check_declaration(&mut self, declaration: &LocalVarDecl, state: &SemanticState<'_>) {
        let Some(ty) = &declaration.var_type else {
            return;
        };
        let Some(declared) = declared_type(ty, state) else {
            return;
        };
        let all_match = declaration.declarators.iter().all(|declarator| {
            declarator.initializer.as_ref().is_some_and(|initializer| {
                !depends_on_declared_type(initializer, state)
                    && has_static_type(initializer, &declared, state)
            })
        });
        if all_match {
            self.report(ty);
        }
    }

    fn check_for_in(&mut self, ty: &DartType, iterable: &Expr, state: &SemanticState<'_>) {
        let Some(declared) = declared_type(ty, state) else {
            return;
        };
        let element = state
            .signatures
            .instantiated_supertype(
                &state.infer(iterable),
                "dart:core",
                "Iterable",
                &state.model,
            )
            .and_then(|iterable| iterable.arguments().first().cloned());
        if element == Some(declared) {
            self.report(ty);
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        match node {
            Stmt::LocalVar(declaration) => self.check_declaration(declaration, state),
            Stmt::For(statement) => match &statement.header.init {
                Some(ForInit::VarDecl(declaration)) => self.check_declaration(declaration, state),
                Some(ForInit::ForIn {
                    var_type: Some(ty),
                    iterable,
                    ..
                }) => self.check_for_in(ty, iterable, state),
                _ => {}
            },
            _ => {}
        }
    }
}
