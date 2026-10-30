use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ResolvedType, ResolvedTypeView, RuleAnalysis, RulePreparation,
    SemanticRuleVisitor, SemanticState, SignatureIndex, returned_expressions, visit_program,
};
use falcon_dart_parser::parse;
use falcon_syntax::cst::*;

fn visit_with_signatures(
    source: &str,
    path: &std::path::Path,
    visitor: &mut impl for<'a> SemanticRuleVisitor<'a>,
) {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::sync::Arc;

    let falcon_dart_parser::ParseResult { errors, tree } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let types = TypeIndex::from_resolved_files(
        [(
            path,
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.to_path_buf(), Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    visit_program(
        visitor,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
}

#[test]
fn generic_predicate_closure_keeps_completed_input_context() {
    struct Observed;
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_call_arguments(
            &mut self,
            _: ArgList<'a>,
            positional: &[ResolvedType],
            _: &HashMap<String, ResolvedType>,
            state: &SemanticState<'a>,
        ) {
            let ResolvedTypeView::Function {
                positional: inputs, ..
            } = positional[0].view()
            else {
                panic!("callback context: {positional:?}")
            };
            assert_eq!(
                inputs[0],
                state.model.core_type("Object", true),
                "callback context"
            );
        }

        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            if let Expr::As(cast) = node {
                assert_eq!(
                    state.infer(cast.expr()),
                    state.model.core_type("Object", true),
                    "closure binding",
                );
            }
        }
    }
    let source = "bool predicate<T>(bool Function(T) callback) => true; \
                  void use() { predicate((trace) { trace as StackTrace; return true; }); }";
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut Observed,
    );
}

#[derive(Default)]
struct CollectionContextObservations {
    elements: Vec<ResolvedType>,
    implicit_call: Option<Vec<ResolvedType>>,
    explicit_call: Option<Vec<ResolvedType>>,
    shadowed: bool,
    static_generic: bool,
    element_contexts: Vec<(String, Option<ResolvedType>)>,
}
impl<'a> SemanticRuleVisitor<'a> for CollectionContextObservations {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Ident(identifier) = node
            && matches!(
                identifier.name(),
                "listLeaf" | "listSpread" | "mapKey" | "mapValue" | "mapSpread"
            )
        {
            self.element_contexts
                .push((identifier.name().to_owned(), state.expected().cloned()));
        }
        match node {
            Expr::Call(call) if matches!(call.callee(), Expr::Ident(name) if name.name() == "maybeFind") =>
            {
                assert!(state.implicit_member_type("maybeFind").is_none());
                let result = state.infer(node);
                assert!(result.interface("dart:core", "int"));
                assert!(result.nullable());
                self.static_generic = true;
            }
            Expr::Field(field) if field.field().name() == "type" => {
                self.elements.push(state.infer(field.object()));
            }
            Expr::Call(call)
                if matches!(call.callee(), Expr::Ident(name) if name.name() == "call")
                    || matches!(call.callee(), Expr::Field(field) if field.field().name() == "call") =>
            {
                let callee = call.callee();
                let args = call.args();
                let signature = state.call_signature(callee, call.type_args(), args, None);
                if args.positional().len() == 2 && matches!(callee, Expr::Ident(_)) {
                    assert!(state.infer(callee).interface("dart:core", "Function"));
                    assert!(signature.is_none());
                    self.implicit_call = Some(Vec::new());
                    return;
                }
                let signature = signature.unwrap();
                if args.positional().len() == 1 {
                    assert_eq!(signature.positional.len(), 1);
                    assert!(signature.positional[0].interface("dart:core", "int"));
                    assert!(state.implicit_member_type("call").is_none());
                    self.shadowed = true;
                } else {
                    self.explicit_call = Some(signature.positional);
                }
            }
            _ => {}
        }
    }
}

fn assert_collection_element_contexts(observed: &CollectionContextObservations) {
    assert_eq!(
        observed.elements,
        [
            ResolvedType::DYNAMIC,
            ResolvedType::DYNAMIC,
            ResolvedType::UNKNOWN,
        ]
    );
    assert_eq!(
        observed
            .element_contexts
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["listLeaf", "listSpread", "mapKey", "mapValue", "mapSpread"]
    );
    let element_contexts: Vec<_> = observed
        .element_contexts
        .iter()
        .map(|(_, ty)| ty.as_ref().expect("element context"))
        .collect();
    assert!(element_contexts[0].interface("dart:core", "int"));
    assert!(element_contexts[1].interface("dart:core", "Iterable"));
    assert!(
        element_contexts[1]
            .type_argument(0)
            .unwrap()
            .interface("dart:core", "int")
    );
    assert!(element_contexts[2].interface("dart:core", "String"));
    assert!(element_contexts[3].interface("dart:core", "int"));
    assert_eq!(observed.element_contexts[4].1, Some(ResolvedType::UNKNOWN));
}

#[test]
fn collection_callbacks_and_implicit_function_calls_match_context() {
    let source = r"
void allOf(dynamic value) {}
void objectContext(Object value) {}
void unresolvedContext(Missing value) {}
void f() {
  allOf([(dynamicElement) => dynamicElement.type]);
  objectContext([(objectElement) => objectElement.type]);
  unresolvedContext([(unresolvedElement) => unresolvedElement.type]);
}
void elementContexts(bool flag, Iterable<int> values, dynamic listLeaf,
    dynamic listSpread, dynamic mapKey, dynamic mapValue, dynamic mapSpread) {
  <int>[if (flag) for (int item in values) listLeaf, ...listSpread];
  <String, int>{if (flag) for (int item in values) mapKey: mapValue, ...mapSpread};
}
class JSObject {}
typedef PostMessage = void Function(JSObject? msg, List<JSObject>? transfer);
extension on PostMessage {
  void sendTyped(JSObject object, List<JSObject> transfer) {
call(object, transfer);
this.call(object, transfer);
  }
  void shadowed(void Function(int) call) { call(1); }
}
class GenericLookup {
  static T? maybeFind<T>() => null;
  int example() => maybeFind<int>()!;
}
";
    let mut observed = CollectionContextObservations::default();
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut observed,
    );
    assert_collection_element_contexts(&observed);
    let implicit = observed.implicit_call.unwrap();
    assert_eq!(implicit, [] as [falcon_analyze::ResolvedType; 0]);
    let explicit = observed.explicit_call.unwrap();
    assert_eq!(explicit.len(), 2);
    assert!(explicit[0].nullable());
    assert!(explicit[1].nullable());
    assert!(observed.shadowed);
    assert!(observed.static_generic);
}

#[test]
fn inherited_implicit_generic_method_preserves_call_parameters() {
    #[derive(Default)]
    struct Observed {
        calls: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            let Expr::Call(call) = node else {
                return;
            };
            let callee = call.callee();
            if !matches!(callee, Expr::Ident(name) if name.name() == "identity") {
                return;
            }
            let token =
                ResolvedType::interface_identity(state.model.resolve_name("Token").unwrap());
            let target = state.call_target(callee, call.type_args()).unwrap();
            assert_eq!(target.signature.call_parameters.len(), 1);
            assert!(target.signature.defaults.contains_key("flag"));
            let signature = state
                .call_signature(callee, call.type_args(), call.args(), None)
                .unwrap();
            assert_eq!(
                signature.positional.as_slice(),
                std::slice::from_ref(&token)
            );
            assert_eq!(signature.return_type, token);
            assert_eq!(state.infer(node), token);
            self.calls += 1;
        }
    }
    let source = r"
class Token {}
class Base { T identity<T>(T value, {bool flag = false}) => value; }
class Child extends Base {
  void probe(Token value) {
identity<Token>(value);
identity(value);
  }
}
class Other {}
extension OtherMethods on Other {
  T identity<T>(T value, {bool flag = false}) => value;
  void probe(Token value) {
identity<Token>(value);
identity(value);
  }
}
";
    let mut observed = Observed::default();
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut observed,
    );
    assert_eq!(observed.calls, 4);
}

fn assert_nullsafe_value(state: &SemanticState<'_>, ty: &ResolvedType, nullable: bool) {
    assert!(
        matches!(ty.view(), ResolvedTypeView::Interface { identity, .. }
            if Some(identity.clone()) == state.model.resolve_name("Value")),
        "{ty:?}"
    );
    assert_eq!(ty.nullable(), nullable, "{ty:?}");
}

#[derive(Default)]
struct NullsafeAccessObservations {
    calls: usize,
    assignments: usize,
    reads: usize,
    echoes: usize,
    record_fields: usize,
}
impl<'a> SemanticRuleVisitor<'a> for NullsafeAccessObservations {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) if matches!(call.callee(), Expr::Field(field) if field.field().name() == "echo") =>
            {
                let Expr::Field(field) = call.callee() else {
                    unreachable!()
                };
                assert_nullsafe_value(
                    state,
                    &state.infer_access_receiver(field.object(), "echo", false),
                    false,
                );
                let signature = state
                    .call_signature(call.callee(), call.type_args(), call.args(), None)
                    .unwrap();
                assert_eq!(
                    signature.positional,
                    [] as [falcon_analyze::ResolvedType; 0]
                );
                assert_nullsafe_value(state, &state.infer_with_context(node, None), true);
                assert_nullsafe_value(state, &state.infer(node), true);
                self.echoes += 1;
            }
            Expr::Call(call) if matches!(call.callee(), Expr::Field(field) if field.field().name() == "take") =>
            {
                let signature = state
                    .call_signature(call.callee(), call.type_args(), call.args(), None)
                    .unwrap();
                let (positional, named) = state
                    .call_parameter_types(call.callee(), call.type_args(), call.args(), None)
                    .unwrap();
                assert!(named.is_empty());
                assert_eq!(positional.len(), 1);
                assert_nullsafe_value(state, &positional[0], false);
                assert_nullsafe_value(state, &signature.positional[0], false);
                assert_nullsafe_value(state, &state.infer(node), true);
                self.calls += 1;
            }
            Expr::Assign(assignment) => {
                let target = assignment.target();
                let nullable =
                    matches!(target, Expr::Field(field) if field.field().name() == "maybe");
                assert_nullsafe_value(state, &state.writable_type(target), nullable);
                self.assignments += 1;
            }
            Expr::Field(field)
                if field.is_null_safe() && matches!(field.field().name(), "$1" | "slot") =>
            {
                let nullable = matches!(field.object(), Expr::Ident(identifier) if identifier.name() == "maybe");
                assert_nullsafe_value(
                    state,
                    &state.infer_access_receiver(node, "", false),
                    nullable,
                );
                assert_nullsafe_value(state, &state.infer(node), true);
                self.record_fields += 1;
            }
            Expr::Field(field)
                if field.is_null_safe() && matches!(field.field().name(), "item" | "maybe") =>
            {
                assert_nullsafe_value(state, &state.infer(node), true);
                self.reads += 1;
            }
            Expr::Index(index) if index.is_null_safe() => {
                assert_nullsafe_value(state, &state.infer(node), true);
                self.reads += 1;
            }
            _ => {}
        }
    }
}

#[test]
fn nullsafe_accesses_preserve_call_and_assignment_contexts_without_an_sdk() {
    let source = r"
class Value { Value echo() => this; }
class Box {
  Value item;
  Value? maybe;
  Value take(Value value) => value;
  T generic<T>(T value) => value;
  Value operator [](int index) => item;
  void operator []=(int index, Value value) {}
}
class Parent { Box? box; }
class Child extends Parent {
  void check(Value value) {
box?.take(value);
super.box?.take(value);
super.box?.item.echo();
super.box?.take(value).echo();
super.box?.generic<Value>.call(value).echo();
super.box?.item;
super.box?[0];
super.box?.item = value;
super.box?.maybe = value;
super.box?[0] = value;
  }
}
void check(Box? box, Value value) {
  box?.take(value);
  box?.item = value;
  box?.maybe = value;
  box?[0] = value;
}
void records((Value,)? positional, ({Value slot})? named, (Value?,)? maybe) {
  positional?.$1.echo();
  named?.slot.echo();
  maybe?.$1;
}
";
    let mut observed = NullsafeAccessObservations::default();
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut observed,
    );
    assert_eq!(observed.calls, 4);
    assert_eq!(observed.assignments, 6);
    assert_eq!(observed.reads, 9);
    assert_eq!(observed.echoes, 5);
    assert_eq!(observed.record_fields, 3);
}

#[test]
fn legacy_switch_case_expressions_reach_semantic_visitors() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;
    #[derive(Default)]
    struct CaseOperand(usize);
    impl<'a> SemanticRuleVisitor<'a> for CaseOperand {
        fn visit_expr(&mut self, node: Expr<'a>, _state: &SemanticState<'a>) {
            if matches!(node, Expr::IntLit(literal) if literal.value() == "7") {
                self.0 += 1;
            }
        }
    }

    let source = "// @dart=2.19\nvoid f(int x) { switch (x) { case 7: break; } }";
    let path = PathBuf::from("/project/lib/main.dart");
    let falcon_dart_parser::ParseResult { errors, tree } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed).with_identities(&identities);
    let mut operand = CaseOperand::default();
    visit_program(
        &mut operand,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(operand.0, 1);
}

#[derive(Default)]
struct ConstructorScopeObservations {
    future_types: Vec<ResolvedType>,
    member_types: Vec<ResolvedType>,
    promoted_field: bool,
    body_field_types: Vec<ResolvedType>,
}
impl<'a> SemanticRuleVisitor<'a> for ConstructorScopeObservations {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Ident(identifier) = node {
            let ty = state.infer(node);
            match identifier.name() {
                "Future" => self.future_types.push(ty),
                "f" => self.member_types.push(ty),
                "y" => self.body_field_types.push(ty),
                "_x" => {
                    self.promoted_field |= ty.interface("dart:core", "int") && !ty.nullable();
                }
                _ => {}
            }
        }
    }
}

#[test]
fn primary_parameters_and_locals_stay_in_constructor_scopes() {
    // Analyzer ConstructorInitializerScope includes primary parameters in
    // non-late instance initializers, but not in methods or late/static fields.
    let source = r"
bool Future = true;
class C(final int? _x, var int? y, int Future) {
  final x = Future;
  late bool delayed = Future;
  static bool shared = Future;
  bool get f => true;
  this { final f = 1; Future; if (y != null) y; }
  void later() { f; Future; }
  int promote() => _x != null ? _x.abs() : 0;
}
extension type E(int? y) {
  this { if (y != null) y; }
}
";
    let mut observed = ConstructorScopeObservations::default();
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut observed,
    );
    assert_eq!(observed.future_types.len(), 5);
    for (ty, name) in observed
        .future_types
        .iter()
        .zip(["int", "bool", "bool", "int", "bool"])
    {
        assert!(ty.interface("dart:core", name), "{ty:?}");
    }
    assert_eq!(observed.member_types.len(), 1);
    assert!(observed.member_types[0].interface("dart:core", "bool"));
    assert!(observed.promoted_field);
    assert_eq!(observed.body_field_types.len(), 4);
    assert!(
        observed
            .body_field_types
            .iter()
            .all(|ty| { ty.interface("dart:core", "int") && ty.nullable() })
    );
}

#[derive(Default)]
struct AssignmentContextObservations {
    promoted: bool,
    elements: Vec<ResolvedType>,
    cascade_nullable: Vec<bool>,
}
impl<'a> SemanticRuleVisitor<'a> for AssignmentContextObservations {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Field(field) = node
            && field.field().name() == "length"
        {
            self.promoted = state.infer(field.object()).interface("dart:core", "String");
        }
        if let Expr::List(list) = node {
            // Observe the schema directly without requiring SDK
            // List-to-Iterable inheritance in this scope-only test.
            let ty = if list.type_arg().is_some() {
                state.infer_with_context(node, state.expected())
            } else {
                state.expected().cloned().unwrap_or(ResolvedType::UNKNOWN)
            };
            self.elements
                .push(ty.type_argument(0).unwrap_or(ResolvedType::UNKNOWN));
        }
    }
    fn visit_cascade_operation(
        &mut self,
        _operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'a>,
    ) {
        self.cascade_nullable.push(receiver.nullable());
    }
}

#[test]
fn assignment_patterns_and_nullable_cascades_supply_precise_contexts() {
    let source = r"
void f(dynamic d, dynamic other, void value) {
  if (d is! String) return;
  int a = 0;
  (d, a) = (other, a);
  d.length;
  [value] = [1];
  [value] = <dynamic>[1];
}
class C { C? parent; void go() {} }
extension Nullable on C? { void method() {} }
void cascades(C? value) {
  value?..method();
  value..method();
}
void assertedCascade(C value) {
  value..parent!.go();
}
";
    let mut observed = AssignmentContextObservations::default();
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut observed,
    );
    assert!(observed.promoted);
    assert_eq!(
        observed.elements,
        [ResolvedType::VOID, ResolvedType::DYNAMIC]
    );
    assert_eq!(observed.cascade_nullable, [false, true, false, true, false]);
}

#[test]
fn finds_nested_returns_without_crossing_function_boundaries() {
    let source = "void f() {
            if (true) { return 1; } else { return 2; }
            for (;;) { return 3; }
            while (true) { return 4; }
            do { return 5; } while (true);
            switch (1) { case 1: return 6; default: return 7; }
            try { return 8; } catch (e) { return 9; } finally { return 10; }
            label: { return 11; }
            void local() { return 12; }
            () { return 13; };
            return () { return 14; };
        }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!()
    };
    let returns = returned_expressions(function.body().unwrap());
    assert_eq!(returns.len(), 12);
    for (expression, expected) in returns
        .iter()
        .zip(["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"])
    {
        assert!(matches!(expression, Expr::IntLit(literal) if literal.value() == expected));
    }
    assert!(matches!(returns[11], Expr::FuncExpr(_)));
}

#[test]
fn returns_borrowed_block_and_arrow_expressions() {
    let source = "int block() { return 1; } int arrow() => 2;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    for declaration in program.declarations() {
        let TopLevelDecl::Function(function) = declaration else {
            panic!()
        };
        let body = function.body().unwrap();
        let expression = match body {
            FunctionBody::Block(block) => {
                let Stmt::Return(statement) = block.stmts().next().unwrap() else {
                    panic!()
                };
                statement.value().unwrap()
            }
            FunctionBody::Arrow(arrow) => arrow.expr(),
            FunctionBody::Native(_) => panic!(),
        };
        let returns = returned_expressions(body);
        assert_eq!(returns.len(), 1);
        assert_eq!(returns[0].syntax().id(), expression.syntax().id());
    }
}

type LifecycleEvents = std::sync::mpsc::Sender<(&'static str, usize)>;

struct LifecycleRule {
    name: &'static str,
    events: LifecycleEvents,
}

struct LifecycleCollector<'a> {
    name: &'static str,
    events: LifecycleEvents,
    context: &'a AnalyzeContext<'a>,
    diagnostics: Vec<falcon_diagnostics::Diagnostic>,
    program_ended: bool,
}

impl<'a> SemanticRuleVisitor<'a> for LifecycleCollector<'_> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        assert!(!self.program_ended, "expressions precede the end hook");
        self.events
            .send((self.name, std::ptr::from_ref(state) as usize))
            .unwrap();
        self.diagnostics.push(falcon_diagnostics::Diagnostic::new(
            self.name,
            falcon_diagnostics::Severity::Warning,
            "observed expression",
            self.context.file_path().to_string_lossy().into_owned(),
            falcon_diagnostics::Span {
                start: node.span().start,
                end: node.span().end,
            },
        ));
    }

    fn visit_program_end(&mut self, state: &SemanticState<'_>) {
        assert!(!self.program_ended, "the end hook runs exactly once");
        assert!(
            !self.diagnostics.is_empty(),
            "expressions precede the end hook"
        );
        assert!(state.model.resolve_name("Root").is_some());
        assert!(!state.environment.is_bound("inner"));
        self.program_ended = true;
    }

    fn finish(self: Box<Self>) -> Vec<falcon_diagnostics::Diagnostic> {
        assert!(self.program_ended, "the end hook precedes finish");
        self.diagnostics
    }
}

impl falcon_analyze::Rule for LifecycleRule {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        context: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(LifecycleCollector {
            name: self.name,
            events: self.events.clone(),
            context,
            diagnostics: Vec::new(),
            program_ended: false,
        }))
    }
}

#[test]
fn registry_shares_semantic_state_and_preserves_collector_lifecycle() {
    use std::path::PathBuf;
    use std::sync::mpsc::channel;

    use falcon_analyze::{IdentityIndex, IdentitySource, RuleRegistry};
    use falcon_config::FalconConfig;

    let source = "class Root {} int root = 1; void f() { int inner = root; }";
    let path = PathBuf::from("/project/lib/main.dart");
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed).with_identities(&identities);
    let (events, received) = channel();
    let mut registry = RuleRegistry::new();
    for name in ["shared-first", "shared-second"] {
        registry.register(
            name,
            Box::new(LifecycleRule {
                name,
                events: events.clone(),
            }),
        );
    }
    // Enablement belongs to the registry's caller: disabled rules are not registered.
    let disabled = LifecycleRule {
        name: "shared-disabled",
        events: events.clone(),
    };
    let diagnostics = registry.run_all(program, &context);
    let observed: Vec<_> = received.try_iter().collect();
    assert_ne!(observed, [] as [(&str, usize); 0]);
    assert_eq!(observed.len() % 2, 0);
    for pair in observed.as_chunks::<2>().0 {
        assert_eq!(pair[0].0, "shared-first");
        assert_eq!(pair[1].0, "shared-second");
        assert_eq!(pair[0].1, pair[1].1, "collectors must share semantic state");
    }
    assert!(observed.iter().all(|(name, _)| *name != disabled.name));
    for name in ["shared-first", "shared-second"] {
        let standalone = LifecycleRule {
            name,
            events: events.clone(),
        }
        .analyze(name, program, &context);
        let shared: Vec<_> = diagnostics
            .iter()
            .filter(|diag| diag.rule == name)
            .collect();
        assert_eq!(shared.len(), standalone.len());
        for (shared, standalone) in shared.into_iter().zip(&standalone) {
            assert_eq!(shared.rule, standalone.rule);
            assert_eq!(shared.span, standalone.span);
            assert_eq!(shared.message, standalone.message);
        }
    }
}

#[test]
fn semantic_collectors_finish_without_identities() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use falcon_analyze::{Rule, RuleRegistry};
    use falcon_config::FalconConfig;
    use falcon_diagnostics::{Diagnostic, Severity};
    use falcon_syntax::cst::Program;

    struct FinishOnly(Arc<AtomicUsize>);

    impl<'a> SemanticRuleVisitor<'a> for FinishOnly {
        fn visit_expr(&mut self, _: Expr<'a>, _: &SemanticState<'a>) {
            panic!("semantic traversal requires identities");
        }

        fn visit_program_end(&mut self, _: &SemanticState<'_>) {
            panic!("the semantic end hook requires identities");
        }

        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            self.0.fetch_add(1, Ordering::Relaxed);
            vec![Diagnostic::new(
                "finish-only",
                Severity::Warning,
                "finished without semantic traversal",
                "/project/lib/main.dart",
                falcon_diagnostics::Span { start: 0, end: 0 },
            )]
        }
    }

    impl Rule for FinishOnly {
        fn prepare<'a>(&self, _: Program<'a>, _: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
            RulePreparation::Semantic(Box::new(Self(Arc::clone(&self.0))))
        }
    }

    let source = "void f() => 1;";
    let path = PathBuf::from("/project/lib/main.dart");
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed);
    let finishes = Arc::new(AtomicUsize::new(0));
    let rule = FinishOnly(Arc::clone(&finishes));
    let standalone = rule.analyze("finish-only", program, &context);
    assert_eq!(finishes.load(Ordering::Relaxed), 1);
    assert_eq!(standalone.len(), 1);

    let mut registry = RuleRegistry::new();
    registry.register("finish-only", Box::new(rule));
    let shared = registry.run_all(program, &context);
    assert_eq!(finishes.load(Ordering::Relaxed), 2);
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].rule, standalone[0].rule);
    assert_eq!(shared[0].span, standalone[0].span);
    assert_eq!(shared[0].message, standalone[0].message);
}

#[test]
fn rules_prepare_syntax_or_semantics_exactly_once() {
    use std::path::Path;
    use std::sync::mpsc::{Sender, channel};

    use falcon_analyze::{IdentityIndex, IdentitySource, Rule, RuleRegistry};
    use falcon_config::FalconConfig;
    use falcon_diagnostics::{Diagnostic, Severity};
    use falcon_syntax::cst::Program;

    struct Syntax(Sender<&'static str>);
    struct Semantic(Sender<&'static str>);

    fn diagnostic(message: &'static str, start: usize) -> Diagnostic {
        Diagnostic::new(
            "mixed-rule",
            Severity::Warning,
            message,
            "/project/lib/main.dart",
            falcon_diagnostics::Span { start, end: start },
        )
    }

    impl Rule for Syntax {
        fn prepare<'a>(&self, _: Program<'a>, _: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
            self.0.send("syntax").unwrap();
            RulePreparation::Diagnostics(vec![diagnostic("syntax", 0)])
        }
    }

    impl Rule for Semantic {
        fn prepare<'a>(&self, _: Program<'a>, _: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
            self.0.send("prepare").unwrap();
            RulePreparation::Semantic(Box::new(Self(self.0.clone())))
        }
    }

    impl<'a> SemanticRuleVisitor<'a> for Semantic {
        fn visit_expr(&mut self, _: Expr<'a>, _: &SemanticState<'a>) {
            self.0.send("expression").unwrap();
        }

        fn visit_program_end(&mut self, _: &SemanticState<'_>) {
            self.0.send("end").unwrap();
        }

        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            self.0.send("finish").unwrap();
            vec![diagnostic("semantic", 1)]
        }
    }

    let source = "void f() => 1;";
    let path = Path::new("/project/lib/main.dart");
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);

    for shared in [false, true] {
        let (events, received) = channel();
        let syntax = Syntax(events.clone());
        let semantic = Semantic(events);
        let diagnostics = if shared {
            let mut registry = RuleRegistry::new();
            registry.register("syntax-rule", Box::new(syntax));
            registry.register("semantic-rule", Box::new(semantic));
            registry.run_all(program, &context)
        } else {
            let mut diagnostics = syntax.analyze("syntax-rule", program, &context);
            diagnostics.extend(semantic.analyze("semantic-rule", program, &context));
            diagnostics
        };
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            ["syntax", "prepare", "expression", "end", "finish"],
        );
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.rule, diagnostic.message.as_str()))
            .collect();
        assert_eq!(
            actual,
            if shared {
                [("semantic-rule", "semantic"), ("syntax-rule", "syntax")]
            } else {
                [("syntax-rule", "syntax"), ("semantic-rule", "semantic")]
            },
        );
    }
}

struct DeclarationReferences<'s> {
    source: &'s str,
    observed: usize,
}
impl<'a> SemanticRuleVisitor<'a> for DeclarationReferences<'_> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        use falcon_analyze::MemberSelectorKind;

        let span = node.span();
        let text = &self.source[span.start..span.end];
        let (owner, kind, is_static, name) = match text {
            "base.value" | "super.value" | "Base.value" => {
                ("Base", MemberSelectorKind::Read, false, "value")
            }
            "Base.shared" => ("Base", MemberSelectorKind::Field, true, "shared"),
            "Base.named" => ("Base", MemberSelectorKind::Constructor, false, "named"),
            "p.Base.method" => ("Base", MemberSelectorKind::Read, true, "method"),
            "base.extra" => ("Extra", MemberSelectorKind::Read, false, "extra"),
            "libraryValue" if matches!(node, Expr::Ident(_)) => {
                if state.constant_owner.is_none() && state.enclosing_type.is_some() {
                    assert!(state.referenced_declaration(node, false).is_none());
                    self.observed += 1;
                    return;
                }
                let base = state.model.resolve_name("Base").unwrap();
                if state.constant_owner.as_ref() == Some(&base) {
                    ("Base", MemberSelectorKind::Read, false, "libraryValue")
                } else {
                    ("libraryValue", MemberSelectorKind::Declaration, false, "")
                }
            }
            "shared" if matches!(node, Expr::Ident(_)) => {
                assert!(state.referenced_declaration(node, false).is_none());
                self.observed += 1;
                return;
            }
            "getterOnly" if matches!(node, Expr::Ident(_)) => {
                assert!(state.referenced_declaration(node, true).is_none());
                self.observed += 1;
                return;
            }
            _ => return,
        };
        let reference = state.referenced_declaration(node, false).expect(text);
        let expected_owner = state
            .model
            .resolve_value(owner)
            .or_else(|| state.model.resolve_name(owner))
            .or_else(|| state.model.resolve_extension(owner))
            .expect("expected declaration in its value, type, or extension namespace");
        assert_eq!(reference.owner, expected_owner, "{text}");
        assert_eq!(reference.selector.kind, kind, "{text}");
        assert_eq!(reference.selector.is_static, is_static, "{text}");
        assert_eq!(reference.selector.name, name, "{text}");
        if text == "base.value" {
            let write = state.referenced_declaration(node, true).unwrap();
            assert_eq!(write.owner, expected_owner);
            assert_eq!(write.selector.kind, MemberSelectorKind::Write);
        }
        self.observed += 1;
    }
}

#[test]
fn prefixed_static_calls_preserve_member_source_selectors() {
    let source = "import 'main.dart' as p; class Base { static void method(Object value) {} } void example() { p.Base.method(1); }";
    let mut visitor = DeclarationReferences {
        source,
        observed: 0,
    };
    visit_with_signatures(
        source,
        std::path::Path::new("/project/main.dart"),
        &mut visitor,
    );
    assert_eq!(visitor.observed, 1);
}

#[test]
fn declaration_references_preserve_dispatch_and_lexical_shadowing() {
    let source = r"
int? libraryValue;
class Base {
  Base.named();
  int value = 0;
  static int shared = 0;
  int? libraryValue;
  void own() { print(libraryValue); }
}
class Child extends Base {
  Child() : super.named();
  void use(Base base) {
    print(base.value);
    base.value = 1;
    print(super.value);
    print(Base.shared);
    print(Base.named);
    final shared = 3;
    print(shared);
    print(libraryValue);
    {
      final Base = base;
      print(Base.value);
    }
  }
}
extension Extra on Base {
  int get extra => value;
  void readLibrary() { print(libraryValue); }
  int get getterOnly => 0;
  void invalidWrite() { getterOnly = 1; }
}
void use(Base base) { print(base.extra); }
extension on Base {
  int get libraryValue => 0;
  int get shared => 0;
  void unnamed() { print(libraryValue); print(shared); }
}
";
    let mut references = DeclarationReferences {
        source,
        observed: 0,
    };
    visit_with_signatures(
        source,
        std::path::Path::new("/project/lib/main.dart"),
        &mut references,
    );
    assert_eq!(references.observed, 14);
}

#[test]
fn binary_chain_visits_on_the_normal_stack() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;
    #[derive(Default)]
    struct Count<'a> {
        binaries: usize,
        lefts: HashMap<Span, Expr<'a>>,
        preorder: HashMap<Span, ResolvedType>,
        changed: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for Count<'a> {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            if let Some(left) = self.lefts.remove(&node.span()) {
                let cached = state.infer(left);
                assert_eq!(cached, state.infer_with_context(left, None));
                if self
                    .preorder
                    .get(&left.span())
                    .is_some_and(|before| *before != cached)
                {
                    self.changed += 1;
                }
            }
            if let Expr::Binary(binary) = node {
                self.binaries += 1;
                let inferred = state.infer(node);
                if node.span().end - node.span().start < 64 {
                    assert_eq!(inferred, state.infer_with_context(node, None));
                    self.lefts.insert(binary.right().span(), binary.left());
                    self.preorder.insert(node.span(), inferred);
                }
            }
            if matches!(node, Expr::Ident(_)) {
                assert_eq!(state.infer(node), state.infer_with_context(node, None));
            }
        }
    }

    let operands = 100_000;
    let source = format!(
        "class A {{ A operator +(A other) => this; }} class B {{}} \
         void f(int a) {{ {}; }} \
         void h(dynamic a) {{ if (a is A) {{ a + (a = B()) + a; }} }}",
        vec!["a"; operands].join(" + ")
    );
    let result = parse(&source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let program = parsed.program();
    let mut count = Count::default();
    visit_program(
        &mut count,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(count.binaries, operands + 1);
    assert_eq!(count.changed, 1, "the fixture must observe a flow change");
}

#[test]
fn optional_binary_chains_visit_every_operand_on_the_normal_stack() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;

    let operands = 100_000;
    for (operator, parameter) in [("&&", "bool"), ("||", "bool"), ("??", "int?")] {
        #[derive(Default)]
        struct Count {
            binaries: usize,
            operands: usize,
        }
        impl<'a> SemanticRuleVisitor<'a> for Count {
            fn visit_expr(&mut self, node: Expr<'a>, _: &SemanticState<'a>) {
                match node {
                    Expr::Binary(_) => self.binaries += 1,
                    Expr::Ident(name) if name.name() == "a" => self.operands += 1,
                    _ => {}
                }
            }
        }
        let chain = vec!["a"; operands].join(&format!(" {operator} "));
        let source =
            format!("void f({parameter} a) {{ var value = {chain}; consume({chain}); {chain}; }}");
        let result = parse(&source);
        assert!(result.errors.is_empty(), "{operator}: {:?}", result.errors);
        let parsed = Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: result.tree,
        });
        let path = Path::new("/project/main.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
        let program = parsed.program();
        let mut count = Count::default();
        visit_program(
            &mut count,
            program,
            SemanticState::new(program, &context).unwrap(),
        );
        assert_eq!(count.binaries, 3 * (operands - 1), "{operator}");
        assert_eq!(count.operands, 3 * operands, "{operator}");
    }
}

#[test]
fn conditional_callbacks_preserve_branch_context_types_and_flow_joins() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;
    #[derive(Default)]
    struct Observed {
        typed: usize,
        calls: usize,
        contexts: Vec<(String, Option<ResolvedType>)>,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            let text = node.syntax().text();
            if matches!(text, "value is int ? value : flag ? 2 : 3" | "flag ? 2 : 3") {
                let integer = state.model.core_type("int", false);
                assert_eq!(state.infer(node), integer);
                self.contexts
                    .push((text.to_owned(), state.expected().cloned()));
                self.typed += 1;
            }
            if text == "flag ? value : value" {
                assert_eq!(state.infer(node), state.model.core_type("Object", false));
                self.typed += 1;
            }
            let Expr::Call(call) = node else {
                return;
            };
            let Expr::Ident(name) = call.callee() else {
                return;
            };
            let nullable = match name.name() {
                "expectInt" => false,
                "expectNullable" => true,
                _ => return,
            };
            assert_eq!(
                state.infer(call.args().positional().next().unwrap()),
                state.model.core_type("int", nullable),
                "{}",
                name.name(),
            );
            self.calls += 1;
        }
    }

    let source = r"
void f(bool flag, Object? value, int? n) {
  int result = value is int ? value : flag ? 2 : 3;
  Object promoted = value != null ? flag ? value : value : 1;
  n != null ? n : (n = 2);
  expectInt(n);
  flag ? (n = null) : 1;
  expectNullable(n);
  n != null ? n : throw 0;
  expectInt(n);
}
";
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let program = parsed.program();
    let state = SemanticState::new(program, &context).unwrap();
    let integer = state.model.core_type("int", false);
    let mut observed = Observed::default();
    visit_program(&mut observed, program, state);
    assert_eq!(observed.typed, 3);
    assert_eq!(observed.calls, 3);
    assert_eq!(
        observed.contexts,
        [
            (
                "value is int ? value : flag ? 2 : 3".into(),
                Some(integer.clone())
            ),
            ("flag ? 2 : 3".into(), Some(integer)),
        ],
    );
}

#[derive(Default)]
struct SelectorContextObservations(Vec<(String, Option<ResolvedType>)>, usize);
impl<'a> SemanticRuleVisitor<'a> for SelectorContextObservations {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::LocalVar(declaration) = node else {
            return;
        };
        let Some(initializer) = declaration
            .declarators()
            .next()
            .and_then(VarDeclarator::initializer)
        else {
            return;
        };
        if initializer.syntax().text() != "box?.child[index]!.child" {
            return;
        }
        let box_type = ResolvedType::interface_identity(state.model.resolve_name("Box").unwrap());
        // Statement callbacks have no enclosing expression inference
        // lifetime: the helper must retain its own reached-type cache.
        assert_eq!(
            state.infer_access_receiver(initializer, "child", false),
            box_type
        );
        assert_eq!(state.infer(initializer), box_type.with_nullable(true));
        self.1 += 1;
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let text = node.syntax().text();
        if !matches!(
            text,
            "box!!.child[index].child"
                | "box!!.child[index]"
                | "box!!.child"
                | "box!!"
                | "box!"
                | "box"
                | "index"
                | "box?.child[index]!.child"
                | "box?.child[index]!"
                | "box?.child[index]"
                | "box?.child"
        ) {
            return;
        }
        let expected_type = if text == "index" {
            state.model.core_type("int", false)
        } else {
            ResolvedType::interface_identity(state.model.resolve_name("Box").unwrap())
                .with_nullable(text == "box" || text.starts_with("box?."))
        };
        assert_eq!(state.infer(node), expected_type, "{text}");
        if matches!(text, "box!" | "box!!") {
            assert_eq!(state.infer_receiver(node), expected_type);
        }
        if matches!(node, Expr::Field(_) | Expr::Index(_) | Expr::NullAssert(_)) {
            let reached = if text.starts_with("box?.") {
                expected_type.non_null()
            } else {
                expected_type.clone()
            };
            assert_eq!(
                state.infer_access_receiver(node, "child", false),
                reached,
                "{text}"
            );
            assert_eq!(
                state.infer_access_receiver(node, "child", true),
                reached.non_null(),
                "{text}",
            );
        }
        self.0.push((text.into(), state.expected().cloned()));
    }
}

#[test]
fn selector_callbacks_preserve_preorder_context_and_receiver_types() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;

    let source = r"
class Box {
  Box get child => this;
  Box operator [](int index) => this;
}
void f(Box? box, int index) {
  Box result = box!!.child[index].child;
}
void g(Box? box, int index) {
  Box? result = box?.child[index]!.child;
}
";
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let program = parsed.program();
    let state = SemanticState::new(program, &context).unwrap();
    let box_type = ResolvedType::interface_identity(state.model.resolve_name("Box").unwrap());
    let mut observed = SelectorContextObservations::default();
    visit_program(&mut observed, program, state);
    assert_eq!(observed.1, 1);
    assert_eq!(
        observed.0,
        [
            ("box!!.child[index].child".into(), Some(box_type.clone())),
            ("box!!.child[index]".into(), None),
            ("box!!.child".into(), Some(ResolvedType::UNKNOWN)),
            ("box!!".into(), None),
            ("box!".into(), Some(ResolvedType::UNKNOWN)),
            ("box".into(), Some(ResolvedType::UNKNOWN)),
            ("index".into(), Some(ResolvedType::UNKNOWN)),
            (
                "box?.child[index]!.child".into(),
                Some(box_type.with_nullable(true))
            ),
            ("box?.child[index]!".into(), None),
            ("box?.child[index]".into(), Some(ResolvedType::UNKNOWN)),
            ("box?.child".into(), Some(ResolvedType::UNKNOWN)),
            ("box".into(), None),
            ("index".into(), Some(ResolvedType::UNKNOWN)),
        ],
    );
}

#[test]
fn optional_binary_callbacks_keep_preorder_context_and_write_promotions() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;
    struct Observed {
        events: Vec<(String, Option<ResolvedType>)>,
        calls: usize,
        context_end: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            if node.span().start < self.context_end
                && matches!(node, Expr::Binary(_) | Expr::Ident(_))
            {
                self.events
                    .push((node.syntax().text().to_owned(), state.expected().cloned()));
            }
            if let Expr::Call(call) = node
                && let Expr::Ident(name) = call.callee()
            {
                let expected = match name.name() {
                    "promotedAnd" | "promotedOr" | "promotedRight" => "String",
                    "afterWrite" | "comparedRight" | "comparedOrRight" => "Object",
                    _ => return,
                };
                let argument = call.args().positional().next().unwrap();
                assert_eq!(
                    state.infer(argument),
                    state.model.core_type(expected, false),
                    "{}",
                    name.name(),
                );
                self.calls += 1;
            }
        }
    }

    let source = r"
void f(bool a, bool b, bool c, int? n, int? m, int tail, Object? value) {
  bool logical = a && b || c;
  int? optional = n ?? m ?? tail;
  value is String && value != null && promotedAnd(value);
  value is! String || value == null || promotedOr(value);
  a && value is String && promotedRight(value);
  a && value != null && comparedRight(value);
  a || value == null || comparedOrRight(value);
  value is String && ((value = 0) == 0 && afterWrite(value));
}
";
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let program = parsed.program();
    let state = SemanticState::new(program, &context).unwrap();
    let boolean = state.model.core_type("bool", false);
    let nullable_int = state.model.core_type("int", true);
    let mut observed = Observed {
        events: Vec::new(),
        calls: 0,
        context_end: source.find("value is String").unwrap(),
    };
    visit_program(&mut observed, program, state);
    let unknown = Some(ResolvedType::UNKNOWN);
    assert_eq!(
        observed.events,
        [
            ("a && b || c".into(), Some(boolean)),
            ("a && b".into(), unknown.clone()),
            ("a".into(), unknown.clone()),
            ("b".into(), unknown.clone()),
            ("c".into(), unknown.clone()),
            ("n ?? m ?? tail".into(), Some(nullable_int.clone())),
            ("n ?? m".into(), unknown.clone()),
            ("n".into(), unknown),
            ("m".into(), Some(nullable_int.clone())),
            ("tail".into(), Some(nullable_int)),
        ],
    );
    assert_eq!(observed.calls, 6);
}

#[test]
fn flow_joins_preserve_reachable_paths_and_nested_scope_depth() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;
    #[derive(Default)]
    struct Observed(usize);
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            let Expr::Call(call) = node else {
                return;
            };
            let Expr::Ident(name) = call.callee() else {
                return;
            };
            let nullable = match name.name() {
                "expectInt" => false,
                "expectNullable" => true,
                _ => return,
            };
            let argument = call.args().positional().next().unwrap();
            assert_eq!(
                state.infer(argument),
                state.model.core_type("int", nullable),
                "{} at {:?}",
                name.name(),
                node.span(),
            );
            self.0 += 1;
        }
    }

    let source = r"
void precise(bool flag) {
  int? value;
  if (flag) { value = 1; } else { throw 0; }
  expectInt(value);
  while (true) { { int inner = 2; value = inner; break; } }
  expectInt(value);
  try { value = null; } finally { value = 3; }
  expectInt(value);
  switch (flag) {
    case true: { int inner = 4; value = inner; break; }
    case false: { int inner = 5; value = inner; break; }
  }
  expectInt(value);
}
void optional(bool flag) {
  int? value;
  while (flag) { int inner = 1; value = inner; break; }
  expectNullable(value);
  switch (flag) { case true: { int inner = 2; value = inner; break; } }
  expectNullable(value);
  try { value = 3; } catch (_) { value = null; }
  expectNullable(value);
}
";
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let program = parsed.program();
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(observed.0, 7);
}

struct LexicalScopeObservations<'s> {
    source: &'s str,
    calls: usize,
}
impl<'a> SemanticRuleVisitor<'a> for LexicalScopeObservations<'_> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Expr::Ident(name) = call.callee() else {
            return;
        };
        let (declaration, constant, ty) = match name.name() {
            "expectRoot" => (None, None, "int"),
            "expectOuter" => (Some("value = 1"), Some("1"), "int"),
            "expectInner" => (Some("value = 2"), Some("2"), "int"),
            "expectMutable" => (Some("value = 3"), None, "int"),
            "expectPattern" => (Some("value when"), None, "int"),
            "expectLoop" => (Some("value = 4"), None, "int"),
            "expectCatch" => (Some("value) {"), None, "Object"),
            _ => return,
        };
        let argument = call.args().positional().next().unwrap();
        assert_eq!(state.infer(argument), state.model.core_type(ty, false));
        assert_eq!(state.is_own_field("value"), declaration.is_none());
        if let Some(declaration) = declaration {
            let span = state.environment.binding_declaration("value").unwrap();
            assert_eq!(span.start, self.source.find(declaration).unwrap());
            assert_eq!(&self.source[span.start..span.end], "value");
            assert!(state.implicit_member_type("value").is_none());
            assert!(state.referenced_declaration(argument, false).is_none());
        } else {
            assert!(state.referenced_declaration(argument, false).is_some());
        }
        assert_eq!(
            state.local_constant("value").map(|value| {
                let span = value.span();
                &self.source[span.start..span.end]
            }),
            constant,
        );
        assert_eq!(
            state.visible_local_constants().contains_key("value"),
            constant.is_some(),
        );
        self.calls += 1;
    }
}

#[test]
fn lexical_shadowing_restores_declarations_constants_and_root_members() {
    let source = r"
class Owner {
  int value = 0;
  void probe(Object input) {
    expectRoot(value);
    {
      const value = 1;
      expectOuter(value);
      {
        const value = 2;
        expectInner(value);
        () { expectInner(value); };
      }
      expectOuter(value);
      {
        int value = 3;
        expectMutable(value);
        () { expectMutable(value); };
      }
      if (input case int value when expectPattern(value)) {
        expectPattern(value);
      } else {
        expectOuter(value);
      }
      expectOuter(value);
      for (int value = 4; value < 5; value++) {
        expectLoop(value);
        () { expectLoop(value); };
      }
      expectOuter(value);
      try {} catch (value) {
        expectCatch(value);
        () { expectCatch(value); };
      }
      expectOuter(value);
    }
    expectRoot(value);
  }
}
";
    let mut observed = LexicalScopeObservations { source, calls: 0 };
    visit_with_signatures(
        source,
        std::path::Path::new("/project/main.dart"),
        &mut observed,
    );
    assert_eq!(observed.calls, 18);
}

#[test]
fn binary_chains_carry_evaluated_receiver_types_across_local_writes() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;
    #[derive(Default)]
    struct Observed {
        writes: usize,
        typed_optional: usize,
        rhs: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            if let Expr::Assign(assignment) = node
                && matches!(assignment.target(), Expr::Ident(name) if name.name() == "y")
            {
                self.writes += 1;
            }
            if let Expr::Call(call) = node
                && matches!(call.callee(), Expr::Ident(name) if name.name() == "rhs")
            {
                // The evaluated A result supplies Object even after the RHS assigns B.
                assert_eq!(
                    state.expected(),
                    Some(&state.model.core_type("Object", false)),
                );
                self.rhs += 1;
            }
            if let Expr::Call(call) = node
                && matches!(call.callee(), Expr::Ident(name) if name.name() == "tail")
            {
                // Contextual num fallback makes the + result num, which has no & operator.
                assert_eq!(state.expected(), Some(&ResolvedType::UNKNOWN));
                self.typed_optional += 1;
            }
        }
    }

    let writes = 8_000;
    let source = format!(
        "class A {{ A operator +(Object other) => this; }} \
         class B {{ B operator +(B other) => this; }} \
         T rhs<T>() => throw 0; \
         T fallback<T>() => throw 0; T tail<T>() => throw 0; \
         void typedOptional(int x, int? y) {{ x + (y ?? fallback()) & tail(); }} \
         void promoted(dynamic a) {{ if (a is A) {{ a + (a = B()) + rhs(); }} }} \
         void parenthesized(dynamic a) {{ if (a is A) {{ (a + (a = B())) + rhs(); }} }} \
         void long(int a, int? y) {{ {}a; }} \
         void optional(int? a, int? y) {{ {}a; }} \
         void nested(int a) {{ {}a{}; }}",
        "a + (y = 1).hashCode + ".repeat(writes),
        "a ?? (y = 1) ?? ".repeat(writes),
        "a + (".repeat(16),
        ")".repeat(16),
    );
    let result = parse(&source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let program = parsed.program();
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(observed.writes, writes * 2);
    assert_eq!(observed.rhs, 2);
    assert_eq!(observed.typed_optional, 1);
}
