//! Don't pass or assign values to `void` (or `FutureOr<void>`) locations.
//!
//! ```dart
//! void accept<T>(T value) {}
//! accept<void>(1); // BAD
//! accept<void>(null); // GOOD
//! ```
//!
//! Arguments of direct function, method, and constructor invocations, assignments
//! (including assigned-variable patterns and cascades), and index assignments
//! are checked against the instantiated parameter type, including receiver and
//! inferred method type arguments in cascade calls: a `void` location accepts only
//! `void`, `Null`, `Never`, and `Future`s of those; a `FutureOr<void>` location
//! additionally accepts `dynamic` and any `FutureOr` or `Future` of an
//! acceptable type. Only block-bodied function literals have their inferred
//! return type checked, inferring the literal against the location's type;
//! expression bodies and tear-offs are not checked as callbacks. Return
//! statements use the enclosing function's return type (a function literal's
//! inferred one, not its context); returning to `void` is checked only for a
//! `dynamic` value. Assignments and returns report at the whole statement,
//! arguments at the value expression.
//! Calls through function-valued variables, fields, and getters are excluded;
//! an explicit `.call(...)` remains a method invocation and is checked.
//! Grouped callees, including instantiated generic tear-offs, remain excluded;
//! grouping a direct invocation does not exclude the invocation itself.
//! Unresolved values stay silent.
//!
//! Adopted from the official `void_checks` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::is_parenthesized;
use crate::lint::function_bodies::{BodyOwner, FunctionBodies};
#[cfg(test)]
use falcon_analyze::semantic_scope::visit_program;
use falcon_analyze::semantic_scope::{CascadeAssignment, SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct VoidChecks;

impl Rule for VoidChecks {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            pending_return: None,
            functions: Vec::new(),
            bodies: FunctionBodies::new(program),
            pattern_assignments: Vec::new(),
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    pending_return: Option<(Span, Span)>,
    /// Local functions enclosing the current node, outermost first: a block
    /// function literal with its inferred return type, or a local function
    /// declaration (`None`), whose returns use the walker's return type.
    functions: Vec<(Span, Option<ResolvedType>)>,
    bodies: FunctionBodies<'a>,
    pattern_assignments: Vec<Span>,
}

impl<'a> Collector<'a> {
    fn enter_function(&mut self, span: Span, return_type: Option<ResolvedType>) {
        self.functions
            .retain(|(enclosing, _)| enclosing.start <= span.start && span.end <= enclosing.end);
        self.functions.push((span, return_type));
    }

    /// The official `visitReturnStatement` checks against the enclosing
    /// function's return type; for a function literal that is its inferred
    /// return type, not the context it was inferred against.
    fn return_type(&self, statement: Span, state: &SemanticState<'_>) -> Option<ResolvedType> {
        match self.functions.iter().rev().find(|(enclosing, _)| {
            enclosing.start <= statement.start && statement.end <= enclosing.end
        }) {
            Some((_, Some(return_type))) => Some(return_type.clone()),
            _ if self
                .bodies
                .enclosing(&statement)
                .is_some_and(|body| body.owner == BodyOwner::Setter) =>
            {
                Some(ResolvedType::Void)
            }
            _ => state.expected().cloned(),
        }
    }

    /// The official `_check`, including its restriction to block-bodied
    /// function literals when checking a function-typed location.
    fn check_value(
        &mut self,
        expected: &ResolvedType,
        value: Expr<'a>,
        state: &SemanticState<'a>,
        reported_span: Span,
        return_statement: bool,
    ) {
        if !matches!(expected, ResolvedType::Void)
            && !(expected.interface("dart:async", "FutureOr")
                && matches!(expected.arguments().first(), Some(ResolvedType::Void)))
            && !(matches!(expected, ResolvedType::Function { .. })
                && matches!(value, Expr::FuncExpr(function)
                    if matches!(function.body(), FunctionBody::Block(_))))
        {
            return;
        }
        // Static argument types include downward inference from their location,
        // including generic constructor calls such as `Future.value(value)`.
        let contextual = state.infer_with_context(value, Some(expected));
        let actual = if contextual == ResolvedType::Unknown {
            state.infer(value)
        } else {
            contextual
        };
        if acceptable(
            expected,
            &actual,
            matches!(value, Expr::FuncExpr(function)
                if matches!(function.body(), FunctionBody::Block(_))),
            return_statement,
        ) == Truth::No
        {
            self.diagnostics.push(self.ctx.diagnostic("This value is not valid for a generic type instantiated with void.", reported_span));
        }
    }

    fn check_arguments(
        &mut self,
        positional: &[ResolvedType],
        named: &std::collections::HashMap<String, ResolvedType>,
        args: ArgList<'a>,
        state: &SemanticState<'a>,
    ) {
        for (argument, expected) in args.positional().zip(positional) {
            self.check_value(expected, argument, state, argument.span(), false);
        }
        for argument in args.named() {
            if let Some(expected) = named.get(argument.name().name()) {
                self.check_value(
                    expected,
                    argument.value(),
                    state,
                    argument.value().span(),
                    false,
                );
            }
        }
    }

    fn check_call(
        &mut self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        state: &SemanticState<'a>,
    ) {
        // Function-expression invocations have no MethodInvocation visitor in
        // the official rule. An explicit `.call(...)` is still a method call.
        if is_parenthesized(callee)
            || match callee {
                Expr::Ident(name) => {
                    if state.environment.is_bound(name.name()) {
                        !state.environment.is_local_function(name.name())
                    } else {
                        state.is_variable_read(name.name())
                            || state
                                .model
                                .resolve_value(name.name())
                                .is_some_and(|identity| state.signatures.is_value_getter(&identity))
                            || state.enclosing_type.as_ref().is_some_and(|receiver| {
                                state
                                    .signatures
                                    .resolved_member_facts(receiver, name.name(), &state.model)
                                    .is_some_and(|facts| {
                                        facts.iter().any(|fact| {
                                            fact.kind == falcon_analyze::MemberKind::Getter
                                        })
                                    })
                            })
                    }
                }
                Expr::Field(field) => {
                    let object = field.object();
                    let receiver = state.infer_receiver(object);
                    state
                        .signatures
                        .resolved_field(&receiver, field.field().name(), &state.model)
                        .is_some()
                        || state
                            .signatures
                            .resolved_extension_getter(
                                &receiver,
                                field.field().name(),
                                &state.model,
                            )
                            .is_some()
                        || falcon_analyze::expression_segments(object, true)
                            .and_then(|segments| state.model.resolve_dotted(&segments))
                            .is_some_and(|identity| {
                                state
                                    .signatures
                                    .declared_member_facts(&identity, field.field().name())
                                    .iter()
                                    .any(|fact| {
                                        matches!(
                                            fact.kind,
                                            falcon_analyze::MemberKind::Getter
                                                | falcon_analyze::MemberKind::Field { .. }
                                        )
                                    })
                            })
                        || matches!(object, Expr::Ident(prefix)
                        if !state.environment.is_bound(prefix.name())
                            && state.model.resolve_value(
                                falcon_analyze::QualifiedName::prefixed(prefix.name(), field.field().name()),
                            ).is_some_and(|identity| {
                                state.signatures.value_type(&identity).is_some()
                            }))
                }
                _ => true,
            }
        {
            return;
        }
        if let Some(signature) = state.call_signature(callee, type_args, args, state.expected()) {
            self.check_arguments(&signature.positional, &signature.named, args, state);
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        // Invoking a preceding selector's result is a FunctionExpressionInvocation,
        // not one of the MethodInvocation nodes visited by the official lint.
        let CascadeOp::Call(call) = operation else {
            return;
        };
        let name = call.name();
        let type_args = call.type_args();
        let args = call.args();
        if let Some((signature, owner)) =
            state
                .signatures
                .resolved_member(receiver, name.name(), &state.model)
        {
            let substitutions =
                state.signature_substitutions(signature, &owner, type_args, args, None);
            let (positional, named) = state.substituted_parameters(signature, &substitutions);
            self.check_arguments(&positional, &named, args, state);
        } else if name.name() == "call"
            && let ResolvedType::Function {
                positional, named, ..
            } = receiver
        {
            for (argument, expected) in args.positional().zip(positional.iter()) {
                self.check_value(expected, argument, state, argument.span(), false);
            }
            for argument in args.named() {
                if let Some((_, expected)) = named
                    .iter()
                    .find(|(name, _)| name == argument.name().name())
                {
                    self.check_value(
                        expected,
                        argument.value(),
                        state,
                        argument.value().span(),
                        false,
                    );
                }
            }
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
    ) {
        let expected = if assignment.indexed {
            state
                .signatures
                .resolved_member(receiver, "[]=", &state.model)
                .and_then(|(signature, owner)| {
                    signature
                        .positional
                        .get(1)
                        .map(|ty| state.model.substitute(ty, &owner))
                })
        } else if let Expr::Ident(name) = assignment.target {
            state
                .signatures
                .resolved_writable_field(receiver, name.name(), &state.model)
                .map(|(ty, owner)| state.model.substitute(&ty, &owner))
        } else {
            None
        };
        if let Some(expected) = expected {
            self.check_value(&expected, assignment.value, state, section, false);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Some((value_span, return_span)) = self.pending_return.take()
            && value_span == node.span()
            && let Some(expected) = self.return_type(return_span, state)
        {
            self.check_value(&expected, node, state, return_span, true);
        }
        match node {
            Expr::Call(call) => {
                if let Expr::GenericInstantiation(instantiation) = call.callee() {
                    self.check_call(
                        instantiation.target(),
                        instantiation.type_args(),
                        call.args(),
                        state,
                    );
                } else {
                    self.check_call(call.callee(), call.type_args(), call.args(), state);
                }
            }
            Expr::New(new) => {
                let args = new.args();
                let constructor = falcon_analyze::resolve_new_constructor(
                    new.dart_type(),
                    new.constructor_name(),
                    &state.model,
                );
                let resolved = state.infer_with_context(node, state.expected());
                if let ResolvedType::Interface { identity, .. } = &resolved
                    && let Some(signature) = state
                        .signatures
                        .constructor(
                            identity,
                            constructor.as_ref().map_or("new", |(_, name)| *name),
                        )
                        .cloned()
                {
                    let substitutions: std::collections::HashMap<_, _> = signature
                        .owner_parameters
                        .iter()
                        .cloned()
                        .zip(resolved.type_arguments())
                        .collect();
                    let mut signature = signature;
                    for ty in &mut signature.positional {
                        *ty = state.model.substitute(ty, &substitutions);
                    }
                    for ty in signature.named.values_mut() {
                        *ty = state.model.substitute(ty, &substitutions);
                    }
                    self.check_arguments(&signature.positional, &signature.named, args, state);
                }
            }
            Expr::Assign(assignment) if assignment.op() == AssignOp::Eq => {
                let target = assignment.target();
                let expected = if let Expr::Index(index) = target {
                    state
                        .signatures
                        .resolved_member(&state.infer(index.object()), "[]=", &state.model)
                        .and_then(|(signature, substitutions)| {
                            signature
                                .positional
                                .get(1)
                                .map(|ty| state.model.substitute(ty, &substitutions))
                        })
                        .unwrap_or(ResolvedType::Unknown)
                } else {
                    state.writable_type(target)
                };
                self.check_value(&expected, assignment.value(), state, node.span(), false);
            }
            Expr::FuncExpr(function) if matches!(function.body(), FunctionBody::Block(_)) => {
                let return_type = state
                    .infer_with_context(node, state.expected())
                    .function_return_type()
                    .unwrap_or(ResolvedType::Unknown);
                self.enter_function(node.span(), Some(return_type));
            }
            _ => {}
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        match node {
            Stmt::Return(statement) => {
                if let Some(value) = statement.value() {
                    self.pending_return = Some((value.span(), node.span()));
                }
            }
            Stmt::LocalFunc(_) => self.enter_function(node.span(), None),
            Stmt::PatternAssign(assignment) => {
                self.pattern_assignments.push(assignment.pattern().span());
            }
            _ => {}
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        if let Pattern::Variable(variable) = node
            && self.pattern_assignments.iter().any(|assignment| {
                assignment.start <= node.span().start && node.span().end <= assignment.end
            })
            && let Some(actual) = matched
        {
            let expected = state.environment.declared_type(variable.name().name());
            if acceptable(&expected, actual, false, false) == Truth::No {
                self.diagnostics.push(self.ctx.diagnostic("This value is not valid for a generic type instantiated with void.", node.span()));
            }
        }
    }
}

fn acceptable(
    expected: &ResolvedType,
    actual: &ResolvedType,
    block_literal: bool,
    return_statement: bool,
) -> Truth {
    if matches!(expected, ResolvedType::Void) {
        if return_statement && !matches!(actual, ResolvedType::Dynamic) {
            return Truth::Yes;
        }
        return acceptable_for_void(actual);
    }
    if expected.interface("dart:async", "FutureOr")
        && matches!(expected.arguments().first(), Some(ResolvedType::Void))
    {
        return acceptable_for_future_or_void(actual);
    }
    if block_literal
        && let ResolvedType::Function {
            return_type: expected,
            ..
        } = expected
        && let ResolvedType::Function {
            return_type: actual,
            ..
        } = actual
    {
        return acceptable(expected, actual, block_literal, return_statement);
    }
    Truth::Yes
}

/// The official `isTypeAcceptableWhenExpectingVoid`.
fn acceptable_for_void(actual: &ResolvedType) -> Truth {
    match actual {
        ResolvedType::Unknown => Truth::Unknown,
        ResolvedType::Void | ResolvedType::Null | ResolvedType::Never => Truth::Yes,
        ResolvedType::Interface { .. } if actual.interface("dart:async", "Future") => actual
            .type_argument(0)
            .map_or(Truth::No, |argument| acceptable_for_void(&argument)),
        _ => Truth::No,
    }
}

/// The official `isTypeAcceptableWhenExpectingFutureOrVoid`.
fn acceptable_for_future_or_void(actual: &ResolvedType) -> Truth {
    match actual {
        ResolvedType::Dynamic => Truth::Yes,
        _ if actual.interface("dart:async", "FutureOr") => Truth::Yes,
        ResolvedType::Interface { .. } if actual.interface("dart:async", "Future") => {
            actual.type_argument(0).map_or(Truth::Yes, |argument| {
                acceptable_for_future_or_void(&argument)
            })
        }
        _ => acceptable_for_void(actual),
    }
}

#[cfg(test)]
mod tests {
    use falcon_analyze::RuleAnalysis;
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, ParsedFile, SignatureIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn grouped_generic_callees_are_not_direct_invocations() {
        let source = r"
class Value {}
void accept<T>(T value) {}
void use(Value value) {
  accept<void>(value);
  (accept<void>(value));
  (accept)<void>(value);
  (accept<void>)(value);
  ((accept<void>))(value);
}
";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, false);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        let diagnostics = VoidChecks.analyze(VoidChecks::NAME, program, &ctx);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    }

    #[test]
    fn generic_invocations_keep_their_downward_argument_context() {
        let source = r"
class Box<T> {
  Box(T value);
  Box.named(T value);
}
Box<T> make<T>(T value) => Box<T>(value);
void accept(Box<void> value) {}
void use() {
  accept(make(1));
  accept(Box(2));
  accept(new Box(3));
  accept(Box<void>.named(4));
  accept(make<int>(5));
  accept(Box<int>(6));
  Box(7);
  new Box(8);
  accept(Box(null));
}
";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, false);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        struct ContextProbe(usize);
        impl<'a> SemanticRuleVisitor<'a> for ContextProbe {
            fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
                if let Expr::Call(call) = node
                    && call.type_args().is_empty()
                    && matches!(call.callee(), Expr::Ident(name) if name.name() == "make")
                {
                    let inferred = state.infer_with_context(node, state.expected());
                    assert_eq!(inferred.type_argument(0), Some(ResolvedType::Void));
                    self.0 += 1;
                }
            }
        }
        let mut probe = ContextProbe(0);
        visit_program(
            &mut probe,
            program,
            SemanticState::new(program, &ctx).unwrap(),
        );
        assert_eq!(probe.0, 1);
        let diagnostics = VoidChecks.analyze(VoidChecks::NAME, program, &ctx);
        let mut actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        actual.sort_unstable();
        assert_eq!(actual, ["1", "2", "3", "4"], "{diagnostics:?}");
    }

    #[test]
    fn future_constructor_context_reports_inner_or_outer_expression_exactly() {
        use falcon_analyze::{
            AnalysisEnvironment, ExternalLibraries, ExternalSources, LibrarySource, TypeIndex,
        };
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        crate::lint::test_support::write_sdk(root, &[
            ("core", "library dart.core; class Object {} abstract class num {} \
             abstract class int extends num {} abstract class Null {} \
             abstract class Function {} abstract class Type {}"),
            ("async", "library dart.async; abstract class FutureOr<T> {} \
             class Future<T> { Future(FutureOr<T> Function() computation); \
             Future.value([FutureOr<T>? value]); }"),
        ]);
        std::fs::create_dir_all(root.join("app/.dart_tool")).unwrap();
        std::fs::create_dir_all(root.join("app/lib")).unwrap();
        std::fs::write(
            root.join("app/.dart_tool/package_config.json"),
            serde_json::json!({
                "configVersion": 2,
                "flutterRoot": format!("file://{}", root.join("flutter").display()),
                "packages": [{"name": "app", "rootUri": "../", "packageUri": "lib/"}]
            })
            .to_string(),
        )
        .unwrap();
        let path = root.join("app/lib/main.dart");
        let source = r"
import 'dart:async';
void take(FutureOr<void> value) {}
void use() {
  take(Future.value(1));
  take(new Future.value(2));
  take(Future<void>.value(3));
  take(new Future<void>.value(4));
  take(Future<int>.value(5));
  Future<void> typed = Future.value(6);
  take(typed);
  Future.value(7);
  new Future.value(8);
  take(Future.value(null));
  take(Future<void>(() {}));
  take(Future(() {}));
}
";
        std::fs::write(&path, source).unwrap();
        let path = falcon_analyze::canonical_or_lexical(&path);
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |config| {
                let root = config?.flutter_root.as_ref()?;
                falcon_analyze::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
            });
        let external = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), program, source)],
            |candidate| candidate == path.as_path(),
        );
        assert!(!external.is_empty());
        let identities = IdentityIndex::build(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        let mut types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                LibrarySource {
                    program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        types.add_external(&external, &identities);
        let signatures = SignatureIndex::build(
            &[(path.clone(), Arc::clone(&parsed))],
            Some(&external),
            &identities,
            &types,
        );
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(&path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        let diagnostics = VoidChecks.analyze(VoidChecks::NAME, program, &ctx);
        let mut actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        actual.sort_unstable();
        let expected: Vec<_> = ["1", "2", "3", "4", "Future<int>.value(5)", "6"]
            .into_iter()
            .map(|text| {
                let start = source.find(text).unwrap();
                (start, start + text.len())
            })
            .collect();
        assert_eq!(actual, expected, "{diagnostics:?}");
    }

    #[test]
    fn cascade_result_invocation_is_distinct_from_explicit_call_method() {
        let source = r#"class Callback {
  void call(void value) {}
}
class Factory {
  void Function(void) make() => (void value) {};
  Callback object() => Callback();
}
void cases(Factory owner, Object value) {
  owner..make()(value);
  owner..object()(value);
  owner..make()(null);
  owner..object()(null);
  owner..make().call(value);
  owner..object().call(value);
}
"#;
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, false);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("void-checks", Box::new(VoidChecks));
        let diagnostics = registry.run_all(program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let line_starts: Vec<_> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        // Real analyzer visits the two explicit `.call` MethodInvocations,
        // not the four preceding FunctionExpressionInvocation controls.
        let expected: Vec<_> = [(13, 22, 5), (14, 24, 5)]
            .into_iter()
            .map(|(line, column, length)| {
                let start = line_starts[line - 1] + column - 1;
                (start, start + length)
            })
            .collect();
        assert_eq!(actual, expected, "{diagnostics:?}");
    }
}
