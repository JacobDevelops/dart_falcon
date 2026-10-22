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

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[cfg(test)]
use falcon_analyze::semantic_scope::visit_program;
use falcon_analyze::semantic_scope::{CascadeAssignment, SemanticRuleVisitor, SemanticState};

/// Members that exist on every object, even one typed `Object?`.
const ALLOWED_MEMBERS: [&str; 4] = ["hashCode", "noSuchMethod", "runtimeType", "toString"];

#[derive(Default)]
pub struct AvoidDynamicCalls;

impl Rule for AvoidDynamicCalls {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            callees: HashSet::new(),
            cascade_sections: HashMap::new(),
        };
        RulePreparation::Semantic(Box::new(collector))
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

impl<'a> Collector<'a> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic("Avoid invoking methods or accessing properties on a `dynamic` target.", span));
    }

    /// Reports `target` when it is provably `dynamic`; explicit casts are allowed.
    fn report_if_dynamic(&mut self, target: Expr<'a>, state: &SemanticState<'a>) -> bool {
        if matches!(target.unparenthesized(), Expr::As(_)) || !self.is_dynamic(target, state) {
            return false;
        }
        self.report(target.span());
        true
    }

    fn is_dynamic(&self, expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
        matches!(state.infer(expr), ResolvedType::Dynamic)
            || dynamic_receiver(expr).is_some_and(|receiver| self.is_dynamic(receiver, state))
    }

    fn is_function_typed(&self, expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
        state.infer(expr).interface("dart:core", "Function")
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
        section: Span,
        state: &SemanticState<'a>,
    ) {
        let end = match operation {
            CascadeOp::Field(field) => field.name().span().end,
            CascadeOp::Call(call) => call.args().span().end,
            CascadeOp::Invoke(invoke) => invoke.args().span().end,
            CascadeOp::Index(index) => index.span().end,
            CascadeOp::NullAssert(assertion) => assertion.span().end,
            CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) => section.end,
        };
        let previous = self
            .cascade_sections
            .insert(section.start, Span::new(section.start, end));
        if let CascadeOp::Call(call) = operation
            && !is_universal_call(call.name().name(), call.args())
            && state
                .signatures
                .resolved_field(receiver, call.name().name(), &state.model)
                .is_some_and(|(ty, owner)| {
                    let ty = state.model.substitute(&ty, &owner);
                    matches!(ty, ResolvedType::Dynamic) || ty.interface("dart:core", "Function")
                })
        {
            // Calling a field/getter is a function-expression invocation, not
            // a statically resolved method invocation on the receiver.
            self.report(Span::new(section.start, call.name().span().end));
        }
        let Some(receiver_span) = previous else {
            return;
        };
        let reportable = match operation {
            CascadeOp::NullAssert(_) => false,
            CascadeOp::Call(call) => !is_universal_call(call.name().name(), call.args()),
            CascadeOp::Field(field) => !ALLOWED_MEMBERS.contains(&field.name().name()),
            CascadeOp::Invoke(_)
            | CascadeOp::Index(_)
            | CascadeOp::Assign(_)
            | CascadeOp::IndexAssign(_) => true,
        };
        if reportable && matches!(receiver, ResolvedType::Dynamic) {
            self.report(receiver_span);
        } else if let CascadeOp::Call(call) = operation
            && call.name().name() == "call"
            && receiver.interface("dart:core", "Function")
        {
            self.report(call.name().span());
        } else if matches!(operation, CascadeOp::Invoke(_))
            && receiver.interface("dart:core", "Function")
        {
            self.report(receiver_span);
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
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
            (matches!(receiver, ResolvedType::Dynamic) && !ALLOWED_MEMBERS.contains(&name.name()))
                || state
                    .signatures
                    .resolved_field(receiver, name.name(), &state.model)
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

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Assign(assignment) => {
                if !matches!(assignment.op(), AssignOp::Eq | AssignOp::NullCoalesceEq)
                    && self.is_dynamic(assignment.target(), state)
                {
                    self.report(assignment.span());
                }
            }
            Expr::Binary(binary) => {
                if is_user_definable(binary.op())
                    && !matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq)
                {
                    self.report_if_dynamic(binary.left(), state);
                }
            }
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                if self.is_dynamic(unary.operand(), state) {
                    self.report(unary.span());
                }
            }
            Expr::PostfixIncDec(postfix) => {
                if self.is_dynamic(postfix.operand(), state) {
                    self.report(postfix.span());
                }
            }
            Expr::Unary(unary) => {
                self.report_if_dynamic(unary.operand(), state);
            }
            Expr::Index(index) => {
                self.report_if_dynamic(index.object(), state);
            }
            Expr::Field(field) => {
                let span = field.span();
                if !self.callees.contains(&(span.start, span.end))
                    && !ALLOWED_MEMBERS.contains(&field.field().name())
                {
                    self.report_if_dynamic(field.object(), state);
                }
            }
            Expr::Call(call) => {
                let callee = call.callee();
                match callee {
                    Expr::Field(field) => {
                        let object = field.object();
                        let name = field.field();
                        let span = field.span();
                        self.callees.insert((span.start, span.end));
                        if !is_universal_call(name.name(), call.args())
                            && !matches!(object.unparenthesized(), Expr::As(_))
                            && !self.report_if_dynamic(object, state)
                            && (self.is_dynamic(callee, state)
                                || self.is_function_typed(callee, state)
                                || (name.name() == "call" && self.is_function_typed(object, state)))
                        {
                            self.report(
                                if name.name() == "call" && self.is_function_typed(object, state) {
                                    name.span()
                                } else {
                                    callee.span()
                                },
                            );
                        }
                    }
                    other => {
                        if !matches!(other.unparenthesized(), Expr::As(_))
                            && (self.is_dynamic(other, state)
                                || self.is_function_typed(other, state)
                                || matches!(
                                    other.unparenthesized(),
                                    Expr::Field(field)
                                        if field.field().name() == "call"
                                            && self.is_function_typed(field.object(), state)
                                ))
                        {
                            self.report(callee.span());
                        }
                    }
                }
            }
            Expr::Cascade(cascade) => {
                let object = cascade.object();
                let dynamic_section =
                    cascade
                        .sections()
                        .any(|section| match section.ops().next() {
                            Some(CascadeOp::Call(_) | CascadeOp::Invoke(_)) => true,
                            Some(CascadeOp::Field(field)) => {
                                !ALLOWED_MEMBERS.contains(&field.name().name())
                            }
                            Some(CascadeOp::Index(_) | CascadeOp::IndexAssign(_)) => true,
                            Some(CascadeOp::Assign(assignment)) => !matches!(
                                assignment.target().unparenthesized(),
                                Expr::Ident(name) if ALLOWED_MEMBERS.contains(&name.name())
                            ),
                            Some(CascadeOp::NullAssert(_)) | None => false,
                        });
                if dynamic_section {
                    self.report_if_dynamic(object, state);
                }
                if !matches!(object.unparenthesized(), Expr::As(_))
                    && self.is_function_typed(object, state)
                {
                    for section in cascade.sections() {
                        if let Some(CascadeOp::Call(call)) = section.ops().next()
                            && call.name().name() == "call"
                        {
                            self.report(call.name().span());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// `toString()` and `noSuchMethod(invocation)` exist on every object.
fn is_universal_call(name: &str, args: ArgList<'_>) -> bool {
    (name == "toString" && args.positional().is_empty() && args.named().is_empty())
        || (name == "noSuchMethod" && args.positional().len() == 1 && args.named().is_empty())
}

/// The operand whose `dynamic` type a dynamic operation on it passes on, for
/// operations whose result is `dynamic` when that operand is.
fn dynamic_receiver(expr: Expr<'_>) -> Option<Expr<'_>> {
    match expr.unparenthesized() {
        Expr::NullAssert(assertion) => Some(assertion.operand()),
        Expr::Field(field) if !ALLOWED_MEMBERS.contains(&field.field().name()) => {
            Some(field.object())
        }
        Expr::Index(index) => Some(index.object()),
        Expr::Call(call) => match call.callee().unparenthesized() {
            Expr::Field(field) => {
                (!is_universal_call(field.field().name(), call.args())).then_some(field.object())
            }
            other => Some(other),
        },
        Expr::Binary(binary)
            if is_user_definable(binary.op())
                && !matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) =>
        {
            Some(binary.left())
        }
        _ => None,
    }
}

fn is_user_definable(op: BinaryOp) -> bool {
    !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce)
}

#[cfg(test)]
mod tests {
    use falcon_analyze::RuleAnalysis;
    use super::*;
    use falcon_analyze::{ParsedFile};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::{path::Path, sync::Arc};

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
        let expect_source = "void expect(dynamic actual) {}";
        let falcon_dart_parser::ParseResult { tree, errors } = parse(expect_source);
        assert!(errors.is_empty(), "{errors:?}");
        let expect_parsed = Arc::new(ParsedFile {
            source: expect_source.into(),
            tree,
        });

        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let indexes = crate::lint::test_support::TestIndexes::new(&[(path, &parsed), (expect_path, &expect_parsed)]);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&indexes.identities)
            .with_signatures(&indexes.signatures);
        struct AccumulatorTypes(Vec<ResolvedType>);
        impl<'a> SemanticRuleVisitor<'a> for AccumulatorTypes {
            fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
                if matches!(node, Expr::Ident(identifier) if identifier.name() == "sum") {
                    self.0.push(state.infer(node));
                }
            }
        }
        let mut accumulator = AccumulatorTypes(Vec::new());
        visit_program(
            &mut accumulator,
            program,
            SemanticState::new(program, &ctx).unwrap(),
        );
        assert_eq!(accumulator.0.len(), 1);
        assert!(
            accumulator.0[0].interface("dart:core", "int"),
            "{:?}",
            accumulator.0
        );
        let diagnostics = AvoidDynamicCalls.analyze(AvoidDynamicCalls::NAME, program, &ctx);
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
        let diagnostics = AvoidDynamicCalls.analyze(AvoidDynamicCalls::NAME, program, &ctx);
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
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        let diagnostics = registry.run_all(program, &ctx);
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
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        let diagnostics = registry.run_all(program, &ctx);
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
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        let diagnostics = registry.run_all(program, &ctx);
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
