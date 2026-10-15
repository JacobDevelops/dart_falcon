//! Avoid method calls or property accesses on a `dynamic` target.
//!
//! Members reached through `dynamic` are looked up at runtime: typos and
//! signature mismatches surface as `NoSuchMethodError` instead of compile
//! errors, and the dispatch is slower:
//!
//! ```dart
//! void log(dynamic event) {
//!   print(event.name); // BAD
//! }
//!
//! void log(Event event) {
//!   print(event.name); // GOOD
//! }
//! ```
//!
//! Reported targets: member accesses, method calls, index operations,
//! user-definable binary operators (not `==`/`!=`), prefix and postfix
//! operators, and compound assignments on a `dynamic` receiver, plus calls of a
//! `dynamic` or `Function`-typed variable, including `.call()` cascade sections.
//! `hashCode`, `runtimeType`, `toString`, and `noSuchMethod` — members every
//! object has — are allowed (except method calls directly on cascade targets),
//! as are explicit `as dynamic` casts and `x!`.
//!
//! Receiver and callable types come from the semantic resolver, including
//! flow-promoted types at each use site.
//! Diagnostics retain grouped receiver spans; compound updates use the complete
//! update expression, and selector chains retain their cascade-section start.
//!
//! Adopted from the official `avoid_dynamic_calls` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{
    CascadeAssignment, SemanticRuleVisitor, SemanticState, visit_program,
};

/// Members that exist on every object, even one typed `Object?`.
const ALLOWED_MEMBERS: [&str; 4] = ["hashCode", "noSuchMethod", "runtimeType", "toString"];

#[derive(Default)]
pub struct AvoidDynamicCalls;

impl Rule for AvoidDynamicCalls {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            callees: HashSet::new(),
            cascade_sections: HashMap::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Member accesses already handled as the callee of a call.
    callees: HashSet<(usize, usize)>,
    /// Complete selector receiver before the next operation in each section.
    cascade_sections: HashMap<usize, Span>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-dynamic-calls",
            "Method invocation or property access on a 'dynamic' target.",
            span,
        ));
    }

    /// Reports `target` when it is provably `dynamic`; explicit casts are allowed.
    fn report_if_dynamic(&mut self, target: &Expr, state: &SemanticState<'_>) -> bool {
        if matches!(target.unparenthesized(), Expr::As { .. }) || !self.is_dynamic(target, state) {
            return false;
        }
        self.report(*target.span());
        true
    }

    fn is_dynamic(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        matches!(state.infer_invocation(expr), ResolvedType::Dynamic)
            || dynamic_receiver(expr).is_some_and(|receiver| self.is_dynamic(receiver, state))
    }

    fn is_function_typed(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        state
            .infer_invocation(expr)
            .interface("dart:core", "Function")
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_cascade_operation(
        &mut self,
        operation: &CascadeOp,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'_>,
    ) {
        let end = match operation {
            CascadeOp::Field { name, .. } => name.span.end,
            CascadeOp::Call(_, _, args) | CascadeOp::Invoke(_, args) => args.span.end,
            CascadeOp::Index { span, .. } => span.end,
            CascadeOp::Assign(..) | CascadeOp::IndexAssign(..) => section.end,
        };
        let previous = self
            .cascade_sections
            .insert(section.start, Span::new(section.start, end));
        if let CascadeOp::Call(name, _, args) = operation
            && !is_universal_call(&name.name, args)
            && state
                .signatures
                .resolved_field(receiver, &name.name, &state.model)
                .is_some_and(|(ty, owner)| {
                    let ty = state.model.substitute(&ty, &owner);
                    matches!(ty, ResolvedType::Dynamic) || ty.interface("dart:core", "Function")
                })
        {
            // Calling a field/getter is a function-expression invocation, not
            // a statically resolved method invocation on the receiver.
            self.report(Span::new(section.start, name.span.end));
        }
        let Some(receiver_span) = previous else {
            return;
        };
        let reportable = match operation {
            CascadeOp::Call(name, _, args) => !is_universal_call(&name.name, args),
            CascadeOp::Field { name, .. } => !ALLOWED_MEMBERS.contains(&name.name.as_str()),
            CascadeOp::Invoke(..)
            | CascadeOp::Index { .. }
            | CascadeOp::Assign(..)
            | CascadeOp::IndexAssign(..) => true,
        };
        if reportable && matches!(receiver, ResolvedType::Dynamic) {
            self.report(receiver_span);
        } else if let CascadeOp::Call(name, _, _) = operation
            && name.name == "call"
            && receiver.interface("dart:core", "Function")
        {
            self.report(name.span);
        } else if matches!(operation, CascadeOp::Invoke(..))
            && receiver.interface("dart:core", "Function")
        {
            self.report(receiver_span);
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'_>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'_>,
    ) {
        if matches!(assignment.operator, AssignOp::Eq | AssignOp::NullCoalesceEq) {
            return;
        }
        let dynamic_read = if assignment.indexed {
            matches!(receiver, ResolvedType::Dynamic)
                || state
                    .signatures
                    .resolved_member(receiver, "[]", &state.model)
                    .is_some_and(|(signature, owner)| {
                        matches!(
                            state.model.substitute(&signature.return_type, &owner),
                            ResolvedType::Dynamic
                        )
                    })
        } else if let Expr::Ident(name) = assignment.target {
            (matches!(receiver, ResolvedType::Dynamic)
                && !ALLOWED_MEMBERS.contains(&name.name.as_str()))
                || state
                    .signatures
                    .resolved_field(receiver, &name.name, &state.model)
                    .is_some_and(|(ty, owner)| {
                        matches!(state.model.substitute(&ty, &owner), ResolvedType::Dynamic)
                    })
        } else {
            false
        };
        if dynamic_read {
            self.report(section);
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Assign {
                target, op, span, ..
            } => {
                if !matches!(op, AssignOp::Eq | AssignOp::NullCoalesceEq)
                    && self.is_dynamic(target, state)
                {
                    self.report(*span);
                }
            }
            Expr::Binary { op, left, .. } => {
                if is_user_definable(*op) && !matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) {
                    self.report_if_dynamic(left, state);
                }
            }
            Expr::Unary {
                op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                operand,
                span,
            }
            | Expr::PostfixIncDec { operand, span, .. } => {
                if self.is_dynamic(operand, state) {
                    self.report(*span);
                }
            }
            Expr::Unary { operand, .. } => {
                self.report_if_dynamic(operand, state);
            }
            Expr::Index { object, .. } => {
                self.report_if_dynamic(object, state);
            }
            Expr::Field {
                object,
                field,
                span,
                ..
            } => {
                if !self.callees.contains(&(span.start, span.end))
                    && !ALLOWED_MEMBERS.contains(&field.name.as_str())
                {
                    self.report_if_dynamic(object, state);
                }
            }
            Expr::Call { callee, args, .. } => match callee.as_ref() {
                Expr::Field {
                    object,
                    field,
                    span,
                    ..
                } => {
                    self.callees.insert((span.start, span.end));
                    if !is_universal_call(&field.name, args)
                        && !matches!(object.unparenthesized(), Expr::As { .. })
                        && !self.report_if_dynamic(object, state)
                        && (self.is_dynamic(callee, state)
                            || self.is_function_typed(callee, state)
                            || (field.name == "call" && self.is_function_typed(object, state)))
                    {
                        self.report(
                            if field.name == "call" && self.is_function_typed(object, state) {
                                field.span
                            } else {
                                *callee.span()
                            },
                        );
                    }
                }
                other => {
                    if !matches!(other.unparenthesized(), Expr::As { .. })
                        && (self.is_dynamic(other, state)
                            || self.is_function_typed(other, state)
                            || matches!(
                                other.unparenthesized(),
                                Expr::Field { object, field, .. }
                                    if field.name == "call" && self.is_function_typed(object, state)
                            ))
                    {
                        self.report(*callee.span());
                    }
                }
            },
            Expr::Cascade {
                object, sections, ..
            } => {
                let dynamic_section = sections.iter().any(|section| match section.ops.first() {
                    Some(CascadeOp::Call(..) | CascadeOp::Invoke(..)) => true,
                    Some(CascadeOp::Field { name, .. }) => {
                        !ALLOWED_MEMBERS.contains(&name.name.as_str())
                    }
                    Some(CascadeOp::Index { .. } | CascadeOp::IndexAssign(..)) => true,
                    Some(CascadeOp::Assign(target, ..)) => !matches!(
                        target.unparenthesized(),
                        Expr::Ident(name) if ALLOWED_MEMBERS.contains(&name.name.as_str())
                    ),
                    None => false,
                });
                if dynamic_section {
                    self.report_if_dynamic(object, state);
                }
                if !matches!(object.unparenthesized(), Expr::As { .. })
                    && self.is_function_typed(object, state)
                {
                    for section in sections {
                        if let Some(CascadeOp::Call(name, _, _)) = section.ops.first()
                            && name.name == "call"
                        {
                            self.report(name.span);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// `toString()` and `noSuchMethod(invocation)` exist on every object.
fn is_universal_call(name: &str, args: &ArgList) -> bool {
    (name == "toString" && args.positional.is_empty() && args.named.is_empty())
        || (name == "noSuchMethod" && args.positional.len() == 1 && args.named.is_empty())
}

/// The operand whose `dynamic` type a dynamic operation on it passes on, for
/// operations whose result is `dynamic` when that operand is.
fn dynamic_receiver(expr: &Expr) -> Option<&Expr> {
    match expr.unparenthesized() {
        Expr::NullAssert { operand, .. } => Some(operand),
        Expr::Field { object, field, .. } if !ALLOWED_MEMBERS.contains(&field.name.as_str()) => {
            Some(object)
        }
        Expr::Index { object, .. } => Some(object),
        Expr::Call { callee, args, .. } => match callee.unparenthesized() {
            Expr::Field { object, field, .. } => {
                (!is_universal_call(&field.name, args)).then_some(object.as_ref())
            }
            other => Some(other),
        },
        Expr::Binary { op, left, .. }
            if is_user_definable(*op) && !matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) =>
        {
            Some(left)
        }
        _ => None,
    }
}

fn is_user_definable(op: BinaryOp) -> bool {
    !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce)
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, SemanticModel, SignatureIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn dynamic_result_context_preserves_inferred_accumulator_type() {
        let source = r"
import 'expect.dart';
T fold<T>(T initial, T Function(T, int) combine) => initial;
void use(dynamic value) {
  expect(fold(0, (sum, item) => sum + item));
  value.missing();
}
";
        let path = Path::new("/project/lib/test.dart");
        let expect_path = Path::new("/project/lib/expect.dart");
        let (expect_program, errors) = parse("void expect(dynamic actual) {}");
        assert!(errors.is_empty(), "{errors:?}");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[
                IdentitySource {
                    path,
                    program: &program,
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: expect_path,
                    program: &expect_program,
                    has_parse_errors: false,
                },
            ],
            &[],
        );
        let files = [(path, &program), (expect_path, &expect_program)];
        let types = falcon_analyze::TypeIndex::from_resolved_files(
            files.map(|(path, program)| {
                (
                    path,
                    falcon_analyze::LibrarySource {
                        program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let signatures = SignatureIndex::build(
            &files.map(|(path, program)| (path.to_path_buf(), program)),
            None,
            &identities,
            &types,
        );
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_signatures(&signatures);
        struct AccumulatorTypes(Vec<ResolvedType>);
        impl SemanticRuleVisitor for AccumulatorTypes {
            fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
                if matches!(node, Expr::Ident(identifier) if identifier.name == "sum") {
                    self.0.push(state.infer(node));
                }
            }
        }
        let mut accumulator = AccumulatorTypes(Vec::new());
        visit_program(
            &mut accumulator,
            &program,
            SemanticState::new(&program, &ctx).unwrap(),
        );
        assert_eq!(accumulator.0.len(), 1);
        assert!(
            accumulator.0[0].interface("dart:core", "int"),
            "{:?}",
            accumulator.0
        );
        let diagnostics = AvoidDynamicCalls.analyze(&program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["value"], "{diagnostics:?}");
    }

    #[test]
    fn reports_dynamic_operations_at_their_ast_receivers() {
        let source = "class Callbacks { final Function callback; Callbacks(this.callback); }\nvoid f(dynamic d, Function fn, Callbacks callbacks) {\n  d\n    ..x += 1;\n  d\n    ..a.b();\n  fn.call();\n  --d.z;\n  (d)();\n  d.toString();\n  (d as dynamic).foo();\n  callbacks\n      .callback();\n}";
        let path = Path::new("/project/lib/test.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let diagnostics = AvoidDynamicCalls.analyze(&program, &ctx);
        let mut starts: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.span.start)
            .collect();
        starts.sort_unstable();
        let mut expected = vec![
            source.find("d\n").unwrap(),
            source.find("..x").unwrap(),
            source.find("d\n    ..a").unwrap(),
            source.find("..a").unwrap(),
            source.find("call()").unwrap(),
            source.find("--d").unwrap(),
            source.find("d.z").unwrap(),
            source.find("(d)()").unwrap(),
            source.find("callbacks\n").unwrap(),
        ];
        expected.sort_unstable();
        assert_eq!(starts, expected, "{diagnostics:?}");
    }

    #[test]
    fn cascade_receivers_and_updates_keep_exact_spans_through_registry() {
        let source = "class SpanOps {\n  dynamic a;\n  Function f;\n  SpanOps(this.a, this.f);\n}\nvoid cascadeSpanWitness(SpanOps o, dynamic e) {\n  o..f.call();\n  ++e;\n  e--;\n  o..a.x += 1;\n  o..a.b();\n  o..a[0].x;\n  o..a[0].x += 1;\n}\n";
        let path = Path::new("/project/lib/test.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        let diagnostics = registry.run_all(&program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        // Fresh SDK analyzer truth: same-start read/write diagnostics must
        // remain distinct after the registry's identical-span deduplication.
        let mut expected = Vec::with_capacity(11);
        for (text, lengths) in [
            ("call()", &[4][..]),
            ("++e", &[3][..]),
            ("e--", &[3][..]),
            ("..a.x += 1", &[3, 10][..]),
            ("..a.b()", &[3][..]),
            ("..a[0].x;", &[3, 6][..]),
            ("..a[0].x += 1", &[3, 6, 13][..]),
        ] {
            let start = source.find(text).unwrap();
            expected.extend(lengths.iter().map(|length| (start, start + length)));
        }
        expected.sort_unstable();
        assert_eq!(actual, expected, "{diagnostics:?}");
    }

    #[test]
    fn grouped_callable_and_cascade_getter_spans_match_analyzer() {
        let source = r#"class SpanCallable {
  Function field;
  dynamic dynField;
  SpanCallable(this.field, this.dynField);
  Function get getter => field;
  dynamic get dynGetter => dynField;
  Function method() => field;
  SpanCallable get nested => this;
}
void groupedCallWitness(Function fn, dynamic d, SpanCallable o) {
  fn.call();
  (fn.call)();
  ((fn.call))();
  (d.foo)();
  ((d.foo))();
  o.field();
  (o.field)();
  o.dynField();
  (o.dynField)();
  o.getter();
  (o.getter)();
  o.dynGetter();
  (o.dynGetter)();
  o.method();
  (o.method)();
  (o.method())();
  o..field();
  o..getter();
  o..dynField();
  o..dynGetter();
  o..method();
  o..nested.field();
  o..nested.getter();
  o..nested.dynField();
  o..nested.dynGetter();
  o..nested.method();
  o..field;
  o..getter;
  o..nested.field;
  o..dynField.foo;
  o..field().foo;
}
"#;
        let path = Path::new("/project/lib/test.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        let diagnostics = registry.run_all(&program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let line_starts: Vec<_> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        // Pinned real-analyzer (line, column, length) truth, including negative
        // statically resolved methods and non-invoked getter/field controls.
        let mut expected: Vec<_> = [
            (11, 6, 4),
            (12, 3, 9),
            (13, 3, 11),
            (14, 3, 7),
            (14, 4, 1),
            (15, 3, 9),
            (15, 5, 1),
            (16, 3, 7),
            (17, 3, 9),
            (18, 3, 10),
            (19, 3, 12),
            (20, 3, 8),
            (21, 3, 10),
            (22, 3, 11),
            (23, 3, 13),
            (26, 3, 12),
            (27, 4, 7),
            (28, 4, 8),
            (29, 4, 10),
            (30, 4, 11),
            (32, 4, 14),
            (33, 4, 15),
            (34, 4, 17),
            (35, 4, 18),
            (40, 4, 10),
            (41, 4, 9),
            (41, 4, 7),
        ]
        .into_iter()
        .map(|(line, column, length)| {
            let start = line_starts[line - 1] + column - 1;
            (start, start + length)
        })
        .collect();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{diagnostics:?}");
    }

    #[test]
    fn trailing_trivia_and_chained_cascade_invocations_match_analyzer_spans() {
        let source = r#"class Holder {
  dynamic method() => () {};
  Function get getter => () {};
  dynamic Function() get typed => () => () {};
}
void cases(dynamic d, Function fn, Holder h) {
  d.field /* gap */ + 1;
  d.method() /* gap */ + 1;
  d[0] /* gap */ + 1;
  (d) /* gap */ .field;
  d..method()();
  d..method()() /* gap */ .field;
  h..method()();
  h..getter()();
  h..typed()();
  d..[0]();
  d..method()()();
  d..method()<int>();
}
"#;
        let path = Path::new("/project/lib/test.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        let diagnostics = registry.run_all(&program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let line_starts: Vec<_> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        // Pinned analyzer truth includes both same-start, different-length
        // diagnostics and generic/non-generic invocations of cascade results.
        let mut expected: Vec<_> = [
            (7, 3, 7),
            (7, 3, 1),
            (8, 3, 10),
            (8, 3, 1),
            (9, 3, 4),
            (9, 3, 1),
            (10, 3, 3),
            (11, 3, 1),
            (11, 4, 10),
            (12, 3, 1),
            (12, 4, 10),
            (12, 4, 12),
            (13, 4, 10),
            (14, 4, 10),
            (14, 4, 8),
            (15, 4, 9),
            (16, 3, 1),
            (16, 4, 5),
            (17, 3, 1),
            (17, 4, 10),
            (17, 4, 12),
            (18, 3, 1),
            (18, 4, 10),
        ]
        .into_iter()
        .map(|(line, column, length)| {
            let start = line_starts[line - 1] + column - 1;
            (start, start + length)
        })
        .collect();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{diagnostics:?}");
    }
}
