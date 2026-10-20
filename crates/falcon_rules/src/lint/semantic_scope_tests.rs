use std::collections::HashMap;

use falcon_analyze::semantic_scope::*;
use falcon_analyze::{AnalyzeContext, ResolvedType, RuleAnalysis, RulePreparation, SignatureIndex};
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
    }
    impl<'a> SemanticRuleVisitor<'a> for Observed {
        fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
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
fn binary_operands_use_operator_parameters_not_the_result_context() {
    use falcon_analyze::{AnalyzeContext, IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use std::path::PathBuf;

    let source = r#"
T pick<T extends num>(T value) => value;
class Numeric {
  double operator +(num value) => 0;
  double operator -(double value) => 0;
}
void run(Numeric number) {
  double broad = number + pick(1.0);
  double narrow = number - pick(2.0);
  double direct = pick(3.0);
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
    let diagnostics =
        crate::lint::style::prefer_int_literals::PreferIntLiterals.analyze(program, &context);
    let reported: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(reported, ["2.0", "3.0"]);
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
        .analyze(program, &context);
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
    let standalone = rule.analyze(program, &context);
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
fn real_semantic_rules_match_standalone_diagnostics_with_suppressions() {
    use std::path::Path;

    use falcon_analyze::{
        FileSuppressions, IdentityIndex, IdentitySource, RuleRegistry, SemanticModel,
    };
    use falcon_config::FalconConfig;

    use crate::lint::correctness::{
        avoid_dynamic_calls::AvoidDynamicCalls, unnecessary_statements::UnnecessaryStatements,
        void_checks::VoidChecks,
    };
    use crate::lint::style::prefer_is_empty::PreferIsEmpty;

    let source = r"
class Value {}
void accept<T>(T value) {}
void f(dynamic target, Value value) {
  target.missing();
  // falcon-ignore lint/correctness/avoid-dynamic-calls: intentional dynamic dispatch
  target.suppressed();
  1 + 2;
  accept<void>(value);
  if (items.length == 0) return;
}
";
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
    let model = SemanticModel::new(path, &parsed, &identities, None);
    let signatures = SignatureIndex::from_program(program, &model);
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed)
        .with_identities(&identities)
        .with_signatures(&signatures);

    // As in production, only enabled rules are registered.
    for enable_style in [false, true] {
        let mut registry = RuleRegistry::with_lookup(crate::meta::suppression_lookup);
        registry.register("void-checks", Box::new(VoidChecks));
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        registry.register("unnecessary-statements", Box::new(UnnecessaryStatements));
        if enable_style {
            registry.register("prefer-is-empty", Box::new(PreferIsEmpty));
        }
        let mut expected: Vec<_> = registry
            .rules()
            .iter()
            .flat_map(|(_, rule)| rule.analyze(program, &context))
            .collect();
        let suppressions = FileSuppressions::parse(
            source,
            &path.to_string_lossy(),
            crate::meta::suppression_lookup,
            &parsed.tree,
        );
        assert!(suppressions.diagnostics().is_empty());
        expected.retain(|diagnostic| {
            !suppressions.is_suppressed(
                diagnostic.rule,
                suppressions.line_for_offset(diagnostic.span.start),
            )
        });
        expected.sort_by(|left, right| {
            left.rule
                .cmp(right.rule)
                .then(left.span.start.cmp(&right.span.start))
                .then(left.span.end.cmp(&right.span.end))
                .then(left.message.cmp(&right.message))
        });
        expected.dedup_by(|left, right| {
            left.rule == right.rule && left.span == right.span && left.message == right.message
        });

        let actual = registry.run_all(program, &context);
        for name in [
            "void-checks",
            "avoid-dynamic-calls",
            "unnecessary-statements",
        ] {
            assert!(
                actual.iter().any(|diagnostic| diagnostic.rule == name),
                "{name}: {actual:?}"
            );
        }
        assert_eq!(
            actual
                .iter()
                .any(|diagnostic| diagnostic.rule == "prefer-is-empty"),
            enable_style,
        );
        assert_eq!(
            actual
                .iter()
                .filter(|diagnostic| diagnostic.rule == "avoid-dynamic-calls")
                .count(),
            1,
            "the suppressed dynamic call must not survive",
        );
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.rule, expected.rule);
            assert_eq!(actual.severity, expected.severity);
            assert_eq!(actual.message, expected.message);
            assert_eq!(actual.file_path, expected.file_path);
            assert_eq!(actual.span, expected.span);
            assert_eq!(actual.line, expected.line);
            assert_eq!(actual.col, expected.col);
        }
    }
}

#[test]
fn real_structural_rules_match_standalone_diagnostics_with_suppressions() {
    use std::path::Path;

    use falcon_analyze::{FileSuppressions, RuleRegistry};
    use falcon_config::FalconConfig;

    use crate::lint::correctness::{
        proper_from_environment::ProperFromEnvironment, throw_in_finally::ThrowInFinally,
    };
    use crate::lint::style::unnecessary_new::UnnecessaryNew;
    use crate::lint::suspicious::avoid_dynamic::AvoidDynamic;

    let source = r"
class Box {
  Box(int value);
}
void consume(Object first, Object second) {}
void f() {
  Map<dynamic, dynamic> map = <dynamic, dynamic>{new Box(1): new Box(2)};
  Map<String, List<dynamic>> nested = {};
  dynamic visible = new Box(3);
  // falcon-ignore lint/suspicious/avoid-dynamic: intentional escape hatch
  dynamic hidden = new Box(4);
  consume(new Box(5), String.fromEnvironment('NESTED'));
  final environment = String.fromEnvironment('VISIBLE');
  const constant = String.fromEnvironment('CONST');
  // falcon-ignore lint/correctness/proper-from-environment: intentional runtime fallback
  final fallback = String.fromEnvironment('SUPPRESSED');
  try {
    consume(visible, map);
  } finally {
    final callback = () => throw new Box(6);
    try {
      throw new Box(7);
    } catch (_) {}
    throw new Box(8);
    // falcon-ignore lint/correctness/throw-in-finally: intentional cleanup failure
    throw new Box(9);
  }
  // falcon-ignore lint/style/unnecessary-new: intentional explicit construction
  new Box(10);
}
";
    let path = Path::new("/project/lib/structural.dart");
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed);
    let suppressions = FileSuppressions::parse(
        source,
        &path.to_string_lossy(),
        crate::meta::suppression_lookup,
        &parsed.tree,
    );
    assert!(suppressions.diagnostics().is_empty());

    // Disabled rules must not join the shared pass or produce diagnostics.
    for enable_style in [false, true] {
        let mut registry = RuleRegistry::with_lookup(crate::meta::suppression_lookup);
        registry.register("avoid-dynamic", Box::new(AvoidDynamic));
        registry.register("proper-from-environment", Box::new(ProperFromEnvironment));
        registry.register("throw-in-finally", Box::new(ThrowInFinally));
        if enable_style {
            registry.register("unnecessary-new", Box::new(UnnecessaryNew));
        }
        let mut expected: Vec<_> = registry
            .rules()
            .iter()
            .flat_map(|(_, rule)| rule.analyze(program, &context))
            .collect();
        for (name, count) in [
            ("avoid-dynamic", 3),
            ("proper-from-environment", 3),
            ("throw-in-finally", 2),
            ("unnecessary-new", if enable_style { 10 } else { 0 }),
        ] {
            assert_eq!(
                expected
                    .iter()
                    .filter(|diagnostic| diagnostic.rule == name)
                    .count(),
                count,
                "standalone {name}: {expected:?}",
            );
        }
        expected.retain(|diagnostic| {
            !suppressions.is_suppressed(
                diagnostic.rule,
                suppressions.line_for_offset(diagnostic.span.start),
            )
        });
        expected.sort_by(|left, right| {
            left.rule
                .cmp(right.rule)
                .then(left.span.start.cmp(&right.span.start))
                .then(left.span.end.cmp(&right.span.end))
                .then(left.message.cmp(&right.message))
        });
        expected.dedup_by(|left, right| {
            left.rule == right.rule && left.span == right.span && left.message == right.message
        });

        let actual = registry.run_all(program, &context);
        for (name, count) in [
            ("avoid-dynamic", 2),
            ("proper-from-environment", 2),
            ("throw-in-finally", 1),
            ("unnecessary-new", if enable_style { 9 } else { 0 }),
        ] {
            assert_eq!(
                actual
                    .iter()
                    .filter(|diagnostic| diagnostic.rule == name)
                    .count(),
                count,
                "registry {name}: {actual:?}",
            );
        }
        let dynamic_offsets: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.rule == "avoid-dynamic")
            .map(|diagnostic| diagnostic.span.start)
            .collect();
        assert_eq!(
            dynamic_offsets,
            [
                source.find("dynamic>>").unwrap(),
                source.find("dynamic visible").unwrap(),
            ],
            "direct Map arguments are exempt, but nested and local types are visited",
        );
        let throws: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.rule == "throw-in-finally")
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(throws, ["throw new Box(8)"]);
        let environment_calls: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.rule == "proper-from-environment")
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            environment_calls,
            [
                "String.fromEnvironment('NESTED')",
                "String.fromEnvironment('VISIBLE')",
            ],
        );
        if enable_style {
            // Map exemptions, const-context routing, and finally function/try
            // boundaries are collector-local: ordinary expression callbacks
            // must still see constructors inside all of those subtrees.
            let constructions: Vec<_> = actual
                .iter()
                .filter(|diagnostic| diagnostic.rule == "unnecessary-new")
                .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
                .collect();
            assert_eq!(
                constructions,
                [
                    "new Box(1)",
                    "new Box(2)",
                    "new Box(3)",
                    "new Box(4)",
                    "new Box(5)",
                    "new Box(6)",
                    "new Box(7)",
                    "new Box(8)",
                    "new Box(9)",
                ],
            );
        }
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.rule, expected.rule);
            assert_eq!(actual.severity, expected.severity);
            assert_eq!(actual.message, expected.message);
            assert_eq!(actual.file_path, expected.file_path);
            assert_eq!(actual.span, expected.span);
            assert_eq!(actual.line, expected.line);
            assert_eq!(actual.col, expected.col);
        }
    }
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
            let mut diagnostics = syntax.analyze(program, &context);
            diagnostics.extend(semantic.analyze(program, &context));
            diagnostics
        };
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            ["syntax", "prepare", "expression", "end", "finish"],
        );
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            ["syntax", "semantic"],
        );
    }
}
