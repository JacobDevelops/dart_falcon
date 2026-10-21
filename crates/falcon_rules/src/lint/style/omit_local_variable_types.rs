//! Flags local variable type annotations that repeat the initializer's type.
//!
//! When a local's declared type is exactly the static type of its
//! initializer, `var` or `final` says the same thing with less noise. The same
//! applies to `for` loop variables, both `for (int i = 0; ...)` and
//! `for (String s in strings)` when the iterable's element type matches.
//! For-in checks require an interface-typed iterable; a type parameter's bound
//! supplies iteration semantics but is not the iterable's own static type.
//! Enum `values` lists are source-derived static members, so their indexing,
//! method calls, and inferred `for-in` bindings participate without SDK sources.
//! Annotations that change the type (`num n = 1`, `int? x = 0`,
//! `Object o = ''`) are kept, as are those that drive inference of a generic
//! call (`int x = parse()` where `parse<T>()` returns `T`).
//!
//! ```dart
//! int count = items.length; // flagged
//! var count = items.length; // ok
//! ```
//!
//! falcon reports only when both types are fully known; unknown components and
//! omitted type arguments are skipped. Explicit `dynamic` type arguments are
//! known types and participate in the exact comparison. The initializer is
//! inferred with the declared type as its context, as the analyzer does, so `List<int> xs = []`,
//! `Box<int> b = Box()` and `double d = c ? 0 : 1.5` are flagged, while a
//! coerced integer literal (`double d = 1`) keeps its annotation.
//! Diagnostic spans stop at the annotation's last token, excluding trailing trivia.
//!
//! Adopted from the official `omit_local_variable_types` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::style::always_specify_types::known_type_arity;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct OmitLocalVariableTypes;

impl Rule for OmitLocalVariableTypes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Whether `ty` is fully determined: no unknown component and every generic
/// type carries all of its type arguments.
fn settled(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    match ty {
        ResolvedType::Unknown => false,
        ResolvedType::Dynamic
        | ResolvedType::Void
        | ResolvedType::Never
        | ResolvedType::Null
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::NonNullTypeParameter { .. } => true,
        ResolvedType::Interface {
            identity,
            arguments,
            ..
        } => {
            let arity = state
                .signatures
                .type_parameter_count(identity)
                .or_else(|| known_type_arity(identity));
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

/// The declared type, when it is a candidate for omission at all. Raw generic
/// types denote their instantiation to bounds.
fn declared_type(ty: DartType<'_>, state: &SemanticState<'_>) -> Option<ResolvedType> {
    let declared = state
        .signatures
        .instantiate_to_bounds(&state.model.resolve_type_in(ty, &state.type_parameters));
    (!matches!(declared, ResolvedType::Null | ResolvedType::Dynamic) && settled(&declared, state))
        .then_some(declared)
}

/// Whether `initializer`'s static type, inferred under the declared type as
/// its context, is exactly `declared`.
fn has_static_type(
    initializer: Expr<'_>,
    declared: &ResolvedType,
    state: &SemanticState<'_>,
) -> bool {
    state
        .signatures
        .instantiate_to_bounds(&state.infer_with_context(initializer, Some(declared)))
        == *declared
}

/// Whether a promoted type parameter acquired an intersection `T & S` that
/// no written type spells. Removing `?` alone still leaves the spellable `T`.
fn promoted_type_parameter(initializer: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let Expr::Ident(identifier) = initializer.unparenthesized() else {
        return false;
    };
    let environment = &state.environment;
    let ResolvedType::TypeParameter { id, .. } = environment.declared_type(identifier.name())
    else {
        return false;
    };
    environment.promotion_chain(identifier.name()).iter().any(
        |promotion| !matches!(promotion, ResolvedType::TypeParameter { id: promoted, .. } if *promoted == id),
    )
}

impl Collector<'_> {
    fn report(&mut self, ty: DartType<'_>) {
        // Parser type spans already end at the last consumed token.
        self.diags.push(self.ctx.diagnostic("Unnecessary type annotation on a local variable.", ty.span()));
    }

    fn check_declaration(&mut self, declaration: LocalVarDecl<'_>, state: &SemanticState<'_>) {
        let Some(ty) = declaration.var_type() else {
            return;
        };
        let Some(declared) = declared_type(ty, state) else {
            return;
        };
        let all_match = declaration.declarators().all(|declarator| {
            declarator.initializer().is_some_and(|initializer| {
                // Only a bare integer literal (`double d = 1`) keeps its type;
                // an explicit Expr::Paren is not this syntax exclusion.
                !(matches!(initializer, Expr::IntLit(_)) && !declared.interface("dart:core", "int"))
                    && !state.generic_function_returns_type_parameter(initializer)
                    && !promoted_type_parameter(initializer, state)
                    && has_static_type(initializer, &declared, state)
            })
        });
        if all_match {
            self.report(ty);
        }
    }

    fn check_for_in(&mut self, ty: DartType<'_>, iterable: Expr<'_>, state: &SemanticState<'_>) {
        let Some(declared) = declared_type(ty, state) else {
            return;
        };
        let context = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
            arguments: vec![declared.clone()].into(),
            nullable: false,
            extension_type: false,
        };
        let inferred = state.infer_with_context(iterable, Some(&context));
        if !matches!(inferred, ResolvedType::Interface { .. }) {
            return;
        }
        let element = state
            .signatures
            .instantiated_supertype(&inferred, "dart:core", "Iterable", &state.model)
            .and_then(|iterable| iterable.type_argument(0));
        if element
            .is_some_and(|element| state.signatures.instantiate_to_bounds(&element) == declared)
        {
            self.report(ty);
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'_>) {
        match node {
            Stmt::LocalVar(declaration) => self.check_declaration(declaration, state),
            Stmt::For(statement) => match statement.header().init() {
                Some(ForInit::VarDecl(declaration)) => self.check_declaration(declaration, state),
                Some(ForInit::ForIn(init)) => {
                    if let Some(ty) = init.var_type() {
                        self.check_for_in(ty, init.iterable(), state);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn annotation_spans_exclude_trailing_comments_and_whitespace() {
        let source = "void f() { List<int> /* named */ a; int Function() /* function */ b; (int, String) /* record */ c; }";
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(block)) = function.body() else {
            panic!("expected block");
        };
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let mut collector = Collector {
            diags: Vec::new(),
            ctx: &ctx,
        };
        for statement in block.stmts() {
            let Stmt::LocalVar(declaration) = statement else {
                panic!("expected local declaration");
            };
            collector.report(declaration.var_type().unwrap());
        }
        let spans: Vec<_> = collector
            .diags
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(spans, ["List<int>", "int Function()", "(int, String)"]);
    }
}
