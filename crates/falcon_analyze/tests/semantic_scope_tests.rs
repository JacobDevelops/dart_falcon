use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ResolvedType, RuleAnalysis, RulePreparation, SemanticRuleVisitor,
    SemanticState, SignatureIndex, returned_expressions, visit_program,
};
use falcon_dart_parser::parse;
use falcon_syntax::cst::*;

#[test]
fn generic_predicate_closure_keeps_completed_input_context() {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = "bool predicate<T>(bool Function(T) callback) => true; \
                  void use() { predicate((trace) { trace as StackTrace; return true; }); }";
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    struct Observed;
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_call_arguments(
            &mut self,
            _: ArgList<'a>,
            positional: &[ResolvedType],
            _: &HashMap<String, ResolvedType>,
            state: &SemanticState<'a>,
        ) {
            let ResolvedType::Function {
                positional: inputs, ..
            } = &positional[0]
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
    visit_program(
        &mut Observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
}

#[test]
fn collection_callbacks_and_implicit_function_calls_match_context() {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = r#"
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
"#;
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    #[derive(Default)]
    struct Observed {
        elements: Vec<ResolvedType>,
        implicit_call: Option<Vec<ResolvedType>>,
        explicit_call: Option<Vec<ResolvedType>>,
        shadowed: bool,
        static_generic: bool,
        element_contexts: Vec<(String, Option<ResolvedType>)>,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
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
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(
        observed.elements,
        [
            ResolvedType::Dynamic,
            ResolvedType::Dynamic,
            ResolvedType::Unknown,
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
    assert_eq!(observed.element_contexts[4].1, Some(ResolvedType::Unknown));
    let implicit = observed.implicit_call.unwrap();
    assert!(implicit.is_empty());
    let explicit = observed.explicit_call.unwrap();
    assert_eq!(explicit.len(), 2);
    assert!(explicit[0].nullable());
    assert!(explicit[1].nullable());
    assert!(observed.shadowed);
    assert!(observed.static_generic);
}

#[test]
fn inherited_implicit_generic_method_preserves_call_parameters() {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = r#"
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
"#;
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);

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
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(observed.calls, 4);
}

#[test]
fn nullsafe_accesses_preserve_call_and_assignment_contexts_without_an_sdk() {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = r#"
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
"#;
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);

    fn assert_value(state: &SemanticState<'_>, ty: &ResolvedType, nullable: bool) {
        assert!(
            matches!(ty, ResolvedType::Interface { identity, .. }
                if Some(identity.clone()) == state.model.resolve_name("Value")),
            "{ty:?}"
        );
        assert_eq!(ty.nullable(), nullable, "{ty:?}");
    }

    #[derive(Default)]
    struct Observed {
        calls: usize,
        assignments: usize,
        reads: usize,
        echoes: usize,
        record_fields: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            match node {
                Expr::Call(call) if matches!(call.callee(), Expr::Field(field) if field.field().name() == "echo") =>
                {
                    let Expr::Field(field) = call.callee() else {
                        unreachable!()
                    };
                    assert_value(
                        state,
                        &state.infer_access_receiver(field.object(), "echo", false),
                        false,
                    );
                    let signature = state
                        .call_signature(call.callee(), call.type_args(), call.args(), None)
                        .unwrap();
                    assert!(signature.positional.is_empty());
                    assert_value(state, &state.infer_with_context(node, None), true);
                    assert_value(state, &state.infer(node), true);
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
                    assert_value(state, &positional[0], false);
                    assert_value(state, &signature.positional[0], false);
                    assert_value(state, &state.infer(node), true);
                    self.calls += 1;
                }
                Expr::Assign(assignment) => {
                    let target = assignment.target();
                    let nullable =
                        matches!(target, Expr::Field(field) if field.field().name() == "maybe");
                    assert_value(state, &state.writable_type(target), nullable);
                    self.assignments += 1;
                }
                Expr::Field(field)
                    if field.is_null_safe() && matches!(field.field().name(), "$1" | "slot") =>
                {
                    let nullable = matches!(field.object(), Expr::Ident(identifier) if identifier.name() == "maybe");
                    assert_value(
                        state,
                        &state.infer_access_receiver(node, "", false),
                        nullable,
                    );
                    assert_value(state, &state.infer(node), true);
                    self.record_fields += 1;
                }
                Expr::Field(field)
                    if field.is_null_safe() && matches!(field.field().name(), "item" | "maybe") =>
                {
                    assert_value(state, &state.infer(node), true);
                    self.reads += 1;
                }
                Expr::Index(index) if index.is_null_safe() => {
                    assert_value(state, &state.infer(node), true);
                    self.reads += 1;
                }
                _ => {}
            }
        }
    }
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
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
    #[derive(Default)]
    struct CaseOperand(usize);
    impl<'a> SemanticRuleVisitor<'a> for CaseOperand {
        fn visit_expr(&mut self, node: Expr<'a>, _state: &SemanticState<'a>) {
            if matches!(node, Expr::IntLit(literal) if literal.value() == "7") {
                self.0 += 1;
            }
        }
    }
    let mut operand = CaseOperand::default();
    visit_program(
        &mut operand,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(operand.0, 1);
}

#[test]
fn primary_parameters_and_locals_stay_in_constructor_scopes() {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    // Analyzer ConstructorInitializerScope includes primary parameters in
    // non-late instance initializers, but not in methods or late/static fields.
    let source = r#"
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
"#;
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    #[derive(Default)]
    struct Observed {
        future_types: Vec<ResolvedType>,
        member_types: Vec<ResolvedType>,
        promoted_field: bool,
        body_field_types: Vec<ResolvedType>,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
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
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
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

#[test]
fn assignment_patterns_and_nullable_cascades_supply_precise_contexts() {
    use falcon_analyze::{AnalyzeContext, IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = r#"
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
"#;
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    #[derive(Default)]
    struct Observed {
        promoted: bool,
        elements: Vec<ResolvedType>,
        cascade_nullable: Vec<bool>,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
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
                    state.expected().cloned().unwrap_or(ResolvedType::Unknown)
                };
                self.elements
                    .push(ty.type_argument(0).unwrap_or(ResolvedType::Unknown));
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
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert!(observed.promoted);
    assert_eq!(
        observed.elements,
        [ResolvedType::Void, ResolvedType::Dynamic]
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

#[test]
fn registry_shares_semantic_state_and_preserves_collector_lifecycle() {
    use std::path::PathBuf;
    use std::sync::mpsc::{Sender, channel};

    use falcon_analyze::{IdentityIndex, IdentitySource, Rule, RuleRegistry};
    use falcon_config::FalconConfig;
    use falcon_diagnostics::{Diagnostic, Severity};
    use falcon_syntax::cst::Program;

    type Events = Sender<(&'static str, usize)>;

    struct Observe {
        name: &'static str,
        events: Events,
    }

    struct Collector<'a> {
        name: &'static str,
        events: Events,
        context: &'a AnalyzeContext<'a>,
        diagnostics: Vec<Diagnostic>,
        program_ended: bool,
    }

    impl<'a> SemanticRuleVisitor<'a> for Collector<'_> {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            assert!(!self.program_ended, "expressions precede the end hook");
            self.events
                .send((self.name, std::ptr::from_ref(state) as usize))
                .unwrap();
            self.diagnostics.push(Diagnostic::new(
                self.name,
                Severity::Warning,
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

        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            assert!(self.program_ended, "the end hook precedes finish");
            self.diagnostics
        }
    }

    impl Rule for Observe {
        fn prepare<'a>(
            &self,
            _program: Program<'a>,
            context: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            RulePreparation::Semantic(Box::new(Collector {
                name: self.name,
                events: self.events.clone(),
                context,
                diagnostics: Vec::new(),
                program_ended: false,
            }))
        }
    }

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
            Box::new(Observe {
                name,
                events: events.clone(),
            }),
        );
    }
    // Enablement belongs to the registry's caller: disabled rules are not registered.
    let disabled = Observe {
        name: "shared-disabled",
        events: events.clone(),
    };
    let diagnostics = registry.run_all(program, &context);
    let observed: Vec<_> = received.try_iter().collect();
    assert!(!observed.is_empty());
    assert_eq!(observed.len() % 2, 0);
    for pair in observed.as_chunks::<2>().0 {
        assert_eq!(pair[0].0, "shared-first");
        assert_eq!(pair[1].0, "shared-second");
        assert_eq!(pair[0].1, pair[1].1, "collectors must share semantic state");
    }
    assert!(observed.iter().all(|(name, _)| *name != disabled.name));
    for name in ["shared-first", "shared-second"] {
        let standalone = Observe {
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

#[test]
fn declaration_references_preserve_dispatch_and_lexical_shadowing() {
    use falcon_analyze::{IdentityIndex, IdentitySource, MemberSelectorKind, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = r#"
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
"#;
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
    let types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            falcon_analyze::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let signatures = SignatureIndex::from_project_files(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    struct References<'s> {
        source: &'s str,
        observed: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for References<'_> {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
            let span = node.span();
            let text = &self.source[span.start..span.end];
            let (owner, kind, is_static, name) = match text {
                "base.value" | "super.value" | "Base.value" => {
                    ("Base", MemberSelectorKind::Read, false, "value")
                }
                "Base.shared" => ("Base", MemberSelectorKind::Field, true, "shared"),
                "Base.named" => ("Base", MemberSelectorKind::Constructor, false, "named"),
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
    let mut references = References {
        source,
        observed: 0,
    };
    visit_program(
        &mut references,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(references.observed, 14);
}

#[test]
fn binary_chain_visits_on_the_normal_stack() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;

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
fn flow_joins_preserve_reachable_paths_and_nested_scope_depth() {
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;

    let source = r#"
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
"#;
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
    let program = parsed.program();
    let mut observed = Observed::default();
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(observed.0, 7);
}

#[test]
fn lexical_shadowing_restores_declarations_constants_and_root_members() {
    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::Path;
    use std::sync::Arc;

    let source = r#"
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
"#;
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let path = Path::new("/project/main.dart");
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
    struct Observed<'s> {
        source: &'s str,
        calls: usize,
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed<'_> {
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
    let mut observed = Observed { source, calls: 0 };
    visit_program(
        &mut observed,
        program,
        SemanticState::new(program, &context).unwrap(),
    );
    assert_eq!(observed.calls, 18);
}
