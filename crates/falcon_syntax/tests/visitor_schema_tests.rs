//! Runtime traversal coverage complements the exhaustive enum matches in the walkers.
use std::collections::{BTreeMap, BTreeSet};

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::visit::*;
use falcon_syntax::cst::*;

#[derive(Default)]
struct Recorder {
    addresses: BTreeSet<(&'static str, NodeId, usize, usize)>,
    current_node: Option<(NodeId, Span)>,
    counts: BTreeMap<&'static str, usize>,
    names: Vec<String>,
    ints: Vec<String>,
    null_asserts: usize,
}

// In particular, do not override visit_expr/visit_pattern here: their default
// implementations flatten chains without invoking those hooks for each link.
#[derive(Default)]
struct DefaultRecorder(Recorder);

impl DefaultRecorder {
    fn recorder(&mut self) -> &mut Recorder {
        &mut self.0
    }
}

impl Recorder {
    fn recorder(&mut self) -> &mut Self {
        self
    }

    fn record(&mut self, kind: &'static str, id: NodeId, span: Span) {
        assert!(
            self.addresses.insert((kind, id, span.start, span.end)),
            "{kind} source node visited more than once"
        );
        *self.counts.entry(kind).or_default() += 1;
    }

    fn assert_reaches_every_node(&self, root: SyntaxNode<'_>) {
        self.assert_reaches_node_hooks(root, false);
        let mut default = DefaultRecorder::default();
        default.visit_program(Program::cast(root).unwrap());
        default.0.assert_reaches_node_hooks(root, true);
    }

    fn assert_reaches_node_hooks(&self, root: SyntaxNode<'_>, default_chains: bool) {
        for node in root.descendants() {
            // ErrorNode is the visitor's opaque recovery payload. Its raw
            // Error subtree is lossless storage, not retained typed syntax.
            if node
                .ancestors()
                .any(|ancestor| ancestor.kind() == NodeKind::Error)
            {
                continue;
            }
            // Match the most specific hook, not just a node id: a DartType
            // callback cannot conceal a missing NamedType callback, for example.
            let hook = match node.kind() {
                NodeKind::Program => "Program",
                NodeKind::LibraryDirective => "LibraryDirective",
                NodeKind::PartOfDirective => "PartOfDirective",
                NodeKind::PartDirective => "PartDirective",
                NodeKind::ImportDirective => "ImportDirective",
                NodeKind::ExportDirective => "ExportDirective",
                NodeKind::ConfigurableUri => "ConfigurableUri",
                NodeKind::Combinator => "ImportCombinator",
                NodeKind::ClassDecl => "ClassDecl",
                NodeKind::PrimaryConstructor => "PrimaryConstructor",
                NodeKind::PrimaryConstructorBody => "PrimaryConstructorBody",
                NodeKind::ClassTypeAliasDecl => "ClassTypeAliasDecl",
                NodeKind::FieldDecl => "FieldDecl",
                NodeKind::VarDeclarator => "VarDeclarator",
                NodeKind::ConstructorDecl => "ConstructorDecl",
                NodeKind::RedirectedConstructor => "RedirectedConstructor",
                NodeKind::SuperCallInitializer
                | NodeKind::ThisCallInitializer
                | NodeKind::FieldInitializer
                | NodeKind::AssertInitializer => "ConstructorInitializer",
                NodeKind::MethodDecl => "MethodDecl",
                NodeKind::GetterDecl => "GetterDecl",
                NodeKind::SetterDecl => "SetterDecl",
                NodeKind::OperatorDecl => "OperatorDecl",
                NodeKind::MixinDecl => "MixinDecl",
                NodeKind::MixinClassDecl => "MixinClassDecl",
                NodeKind::EnumDecl => "EnumDecl",
                NodeKind::EnumVariant => "EnumVariant",
                NodeKind::ExtensionDecl => "ExtensionDecl",
                NodeKind::ExtensionTypeDecl => "ExtensionTypeDecl",
                NodeKind::FunctionDecl => "FunctionDecl",
                NodeKind::TopLevelVarDecl => "TopLevelVarDecl",
                NodeKind::TypeAliasDecl => "TypeAliasDecl",
                NodeKind::FormalParamList => "FormalParamList",
                NodeKind::FormalParam(_) => "FormalParam",
                NodeKind::ArrowBody | NodeKind::NativeBody => "FunctionBody",
                NodeKind::Annotation => "Annotation",
                NodeKind::NamedType => "NamedType",
                NodeKind::FunctionType => "FunctionType",
                NodeKind::FunctionTypeParam(_) => "FunctionTypeParam",
                NodeKind::RecordType => "RecordType",
                NodeKind::NamedRecordField => "NamedRecordField",
                NodeKind::TypeParam => "TypeParam",
                NodeKind::VoidType | NodeKind::DynamicType => "DartType",
                NodeKind::Block => "Block",
                NodeKind::IfStmt => "IfStmt",
                NodeKind::CaseCondition => "IfCondition",
                NodeKind::ForStmt => "ForStmt",
                NodeKind::ForHeader => "ForHeader",
                NodeKind::ForInInit | NodeKind::PatternForInInit | NodeKind::ForExprsInit => {
                    "ForInit"
                }
                NodeKind::WhileStmt => "WhileStmt",
                NodeKind::DoWhileStmt => "DoWhileStmt",
                NodeKind::SwitchStmt => "SwitchStmt",
                NodeKind::SwitchCase => "SwitchCase",
                NodeKind::CasePatternLabel | NodeKind::CaseExprLabel | NodeKind::DefaultLabel => {
                    "SwitchCaseKind"
                }
                NodeKind::TryCatchStmt => "TryCatchStmt",
                NodeKind::CatchClause => "CatchClause",
                NodeKind::ReturnStmt => "ReturnStmt",
                NodeKind::ThrowStmt => "ThrowStmt",
                NodeKind::RethrowStmt | NodeKind::EmptyStmt => "Stmt",
                NodeKind::BreakStmt => "BreakStmt",
                NodeKind::ContinueStmt => "ContinueStmt",
                NodeKind::LocalVarDecl => "LocalVarDecl",
                NodeKind::PatternDeclaration => "PatternDeclaration",
                NodeKind::PatternAssignStmt => "PatternAssignStmt",
                NodeKind::LabeledStmt => "LabeledStmt",
                NodeKind::LocalFuncDecl => "LocalFuncDecl",
                NodeKind::AssertStmt => "AssertStmt",
                NodeKind::YieldStmt => "YieldStmt",
                NodeKind::ExprStmt => "ExprStmt",
                NodeKind::ErrorNode => "ErrorNode",
                NodeKind::Identifier => "Identifier",
                NodeKind::StringLitNode | NodeKind::AdjacentStrings => "StringLitNode",
                NodeKind::StringInterpolation => "StringInterpolation",
                NodeKind::CascadeSection => "CascadeSection",
                NodeKind::CascadeField
                | NodeKind::CascadeIndex
                | NodeKind::CascadeNullAssert
                | NodeKind::CascadeCall
                | NodeKind::CascadeInvoke
                | NodeKind::CascadeAssign
                | NodeKind::CascadeIndexAssign => "CascadeOp",
                NodeKind::NullAwareElement
                | NodeKind::SpreadElement
                | NodeKind::IfElement
                | NodeKind::ForElement => "CollectionElement",
                NodeKind::MapEntry => "MapEntry",
                NodeKind::MapSpreadElement | NodeKind::MapIfElement | NodeKind::MapForElement => {
                    "MapElement"
                }
                NodeKind::RecordField => "RecordField",
                NodeKind::ArgList => "ArgList",
                NodeKind::NamedArg => "NamedArg",
                NodeKind::SwitchExprArm => "SwitchExprArm",
                NodeKind::LiteralPattern => "LiteralPattern",
                NodeKind::ConstPattern => "ConstPattern",
                NodeKind::ListPattern => "ListPattern",
                NodeKind::RecordPattern => "RecordPattern",
                NodeKind::MapPattern => "MapPattern",
                NodeKind::ObjectPattern => "ObjectPattern",
                NodeKind::RestPattern => "ListPatternElement",
                NodeKind::RecordPatternField => "RecordPatternField",
                NodeKind::MapPatternEntry => "MapPatternEntry",
                NodeKind::ObjectPatternField => "ObjectPatternField",
                NodeKind::WildcardPattern
                | NodeKind::VariablePattern
                | NodeKind::LogicalAndPattern
                | NodeKind::LogicalOrPattern
                | NodeKind::RelationalPattern
                | NodeKind::CastPattern
                | NodeKind::NullCheckPattern
                | NodeKind::NullAssertPattern
                | NodeKind::ParenPattern
                | NodeKind::ErrorPattern => {
                    if default_chains {
                        // Default pattern chains promise their observable
                        // children, not a callback for each intermediate link.
                        continue;
                    }
                    "Pattern"
                }
                NodeKind::IntLitExpr
                | NodeKind::DoubleLitExpr
                | NodeKind::BoolLitExpr
                | NodeKind::NullLitExpr
                | NodeKind::ParenExpr
                | NodeKind::ThisExpr
                | NodeKind::SuperExpr
                | NodeKind::UnaryExpr
                | NodeKind::PostfixIncDecExpr
                | NodeKind::BinaryExpr
                | NodeKind::AssignExpr
                | NodeKind::ConditionalExpr
                | NodeKind::IsExpr
                | NodeKind::AsExpr
                | NodeKind::FieldExpr
                | NodeKind::IndexExpr
                | NodeKind::CallExpr
                | NodeKind::CascadeExpr
                | NodeKind::ListExpr
                | NodeKind::MapExpr
                | NodeKind::SetExpr
                | NodeKind::RecordExpr
                | NodeKind::FuncExpr
                | NodeKind::NewExpr
                | NodeKind::DotShorthandExpr
                | NodeKind::AwaitExpr
                | NodeKind::ThrowExpr
                | NodeKind::SwitchExpr
                | NodeKind::NullAssertExpr
                | NodeKind::SymbolLitExpr
                | NodeKind::GenericInstantiationExpr
                | NodeKind::ErrorExpr => {
                    if default_chains {
                        // The unmodified default expression hook has no leaf
                        // callback for literal values or chain links themselves.
                        continue;
                    }
                    "Expr"
                }
                // Transparent source grouping nodes have no independent hook.
                NodeKind::ExtendsClause
                | NodeKind::WithClause
                | NodeKind::ImplementsClause
                | NodeKind::OnClause
                | NodeKind::SetterParameter
                | NodeKind::LegacyTypedef
                | NodeKind::TypeArgs
                | NodeKind::ForCondition
                | NodeKind::ForUpdates
                | NodeKind::AwaitFor
                | NodeKind::ExprCondition
                | NodeKind::ExprGuard
                | NodeKind::ConstPatternExpr
                | NodeKind::Error => continue,
            };
            // Adjacent strings expose one logical literal to visitors; the
            // segment nodes are spelling, not additional expression visits.
            if node.kind() == NodeKind::StringLitNode
                && node
                    .parent()
                    .is_some_and(|parent| parent.kind() == NodeKind::AdjacentStrings)
            {
                continue;
            }
            assert!(
                self.addresses
                    .iter()
                    .any(|(kind, id, _, _)| *kind == hook && *id == node.id()),
                "{} walk missed {hook} for {:?} at {:?}: {:?}",
                if default_chains {
                    "default"
                } else {
                    "explicit"
                },
                node.kind(),
                node.span(),
                node.text()
            );
        }
    }
}

macro_rules! recording_visitor {
    ($($ty:ident => $hook:ident $(/ $walk:ident)?),* $(,)?) => {
        recording_visitor!(@impl Recorder {
            fn visit_expr(&mut self, node: Expr<'a>) {
                self.record("Expr", node.syntax().id(), node.span());
                if let Expr::IntLit(value) = node {
                    self.ints.push(value.value().to_owned());
                }
                walk_expr(self, node);
            }

            fn visit_pattern(&mut self, node: Pattern<'a>) {
                self.record("Pattern", node.syntax().id(), node.span());
                walk_pattern(self, node);
            }
        } $($ty => $hook $(/ $walk)?),*);
        recording_visitor!(@impl DefaultRecorder {} $($ty => $hook $(/ $walk)?),*);
    };
    (@impl $recorder:ident { $($extra:tt)* } $($ty:ident => $hook:ident $(/ $walk:ident)?),* $(,)?) => {
        impl<'a> Visitor<'a> for $recorder {
            $($extra)*
            $(fn $hook(&mut self, node: $ty<'a>) {
                self.recorder().record(stringify!($ty), node.syntax().id(), node.span());
                let previous = self.recorder().current_node.replace((node.syntax().id(), node.span()));
                $($walk(self, node);)?
                self.recorder().current_node = previous;
            })*

            fn visit_identifier(&mut self, node: Identifier<'a>) {
                self.recorder().record("Identifier", node.syntax().id(), node.span());
                self.recorder().names.push(node.name().to_owned());
            }

            fn visit_name(&mut self, node: Identifier<'a>) {
                self.visit_identifier(node);
            }

            fn visit_cascade_op(&mut self, node: CascadeOp<'a>) {
                self.recorder().record("CascadeOp", node.syntax().id(), node.span());
                if let CascadeOp::NullAssert(_) = node {
                    self.recorder().null_asserts += 1;
                }
                walk_cascade_op(self, node);
            }

            fn visit_function_kind(&mut self, node: FunctionKind<'a>) {
                let (id, span) = self.recorder().current_node.expect("function kind owner");
                self.recorder().record("FunctionKind", id, span);
                walk_function_kind(self, node);
            }

            fn visit_literal_pattern_value(&mut self, node: LiteralPatternValue<'a>) {
                let (id, span) = self.recorder().current_node.expect("literal pattern owner");
                self.recorder().record("LiteralPatternValue", id, span);
                walk_literal_pattern_value(self, node);
            }

            fn visit_const_pattern_value(&mut self, node: ConstPatternValue<'a>) {
                let (id, span) = self.recorder().current_node.expect("constant pattern owner");
                self.recorder().record("ConstPatternValue", id, span);
                walk_const_pattern_value(self, node);
            }
        }
    };
}

recording_visitor! {
    Program => visit_program / walk_program,
    StringLitNode => visit_string_lit,
    StringLitNode => visit_native_string,
    StringInterpolation => visit_string_interpolation / walk_string_interpolation,
    LibraryDirective => visit_library_directive / walk_library_directive,
    PartOfDirective => visit_part_of_directive / walk_part_of_directive,
    PartDirective => visit_part_directive / walk_part_directive,
    ImportDirective => visit_import / walk_import,
    ExportDirective => visit_export / walk_export,
    ConfigurableUri => visit_configurable_uri / walk_configurable_uri,
    ImportCombinator => visit_import_combinator / walk_import_combinator,
    TopLevelDecl => visit_top_level_decl / walk_top_level_decl,
    ClassDecl => visit_class_decl / walk_class_decl,
    PrimaryConstructor => visit_primary_constructor / walk_primary_constructor,
    PrimaryConstructorBody => visit_primary_constructor_body / walk_primary_constructor_body,
    ClassTypeAliasDecl => visit_class_type_alias / walk_class_type_alias,
    ClassMember => visit_class_member / walk_class_member,
    FieldDecl => visit_field_decl / walk_field_decl,
    VarDeclarator => visit_var_declarator / walk_var_declarator,
    ConstructorDecl => visit_constructor_decl / walk_constructor_decl,
    RedirectedConstructor => visit_redirected_constructor / walk_redirected_constructor,
    ConstructorInitializer => visit_constructor_initializer / walk_constructor_initializer,
    MethodDecl => visit_method_decl / walk_method_decl,
    GetterDecl => visit_getter_decl / walk_getter_decl,
    SetterDecl => visit_setter_decl / walk_setter_decl,
    OperatorDecl => visit_operator_decl / walk_operator_decl,
    MixinDecl => visit_mixin_decl / walk_mixin_decl,
    MixinClassDecl => visit_mixin_class / walk_mixin_class,
    EnumDecl => visit_enum_decl / walk_enum_decl,
    EnumVariant => visit_enum_variant / walk_enum_variant,
    ExtensionDecl => visit_extension_decl / walk_extension_decl,
    ExtensionTypeDecl => visit_extension_type / walk_extension_type,
    FunctionDecl => visit_function_decl / walk_function_decl,
    TopLevelVarDecl => visit_top_level_var / walk_top_level_var,
    TypeAliasDecl => visit_type_alias_decl / walk_type_alias_decl,
    DartType => visit_dart_type / walk_dart_type,
    NamedType => visit_named_type / walk_named_type,
    FunctionType => visit_function_type / walk_function_type,
    FunctionTypeParam => visit_function_type_param / walk_function_type_param,
    RecordType => visit_record_type / walk_record_type,
    NamedRecordField => visit_named_record_field / walk_named_record_field,
    TypeParam => visit_type_param / walk_type_param,
    FormalParamList => visit_formal_param_list / walk_formal_param_list,
    FormalParam => visit_formal_param / walk_formal_param,
    FunctionBody => visit_function_body / walk_function_body,
    Annotation => visit_annotation / walk_annotation,
    Block => visit_block / walk_block,
    Stmt => visit_stmt / walk_stmt,
    IfStmt => visit_if_stmt / walk_if_stmt,
    IfCondition => visit_if_condition / walk_if_condition,
    ForStmt => visit_for_stmt / walk_for_stmt,
    ForInit => visit_for_init / walk_for_init,
    ForHeader => visit_for_header / walk_for_header,
    WhileStmt => visit_while_stmt / walk_while_stmt,
    DoWhileStmt => visit_do_while_stmt / walk_do_while_stmt,
    SwitchStmt => visit_switch_stmt / walk_switch_stmt,
    SwitchCase => visit_switch_case / walk_switch_case,
    SwitchCaseKind => visit_switch_case_kind / walk_switch_case_kind,
    TryCatchStmt => visit_try_catch_stmt / walk_try_catch_stmt,
    CatchClause => visit_catch_clause / walk_catch_clause,
    ReturnStmt => visit_return_stmt / walk_return_stmt,
    ThrowStmt => visit_throw_stmt / walk_throw_stmt,
    BreakStmt => visit_break_stmt / walk_break_stmt,
    ContinueStmt => visit_continue_stmt / walk_continue_stmt,
    LocalVarDecl => visit_local_var / walk_local_var,
    PatternDeclaration => visit_pattern_declaration / walk_pattern_declaration,
    PatternAssignStmt => visit_pattern_assign_stmt / walk_pattern_assign_stmt,
    LabeledStmt => visit_labeled_stmt / walk_labeled_stmt,
    LocalFuncDecl => visit_local_func_decl / walk_local_func_decl,
    AssertStmt => visit_assert_stmt / walk_assert_stmt,
    YieldStmt => visit_yield_stmt / walk_yield_stmt,
    ExprStmt => visit_expr_stmt / walk_expr_stmt,
    CascadeSection => visit_cascade_section / walk_cascade_section,
    CollectionElement => visit_collection_element / walk_collection_element,
    MapEntry => visit_map_entry / walk_map_entry,
    MapElement => visit_map_element / walk_map_element,
    RecordField => visit_record_field / walk_record_field,
    ArgList => visit_arg_list / walk_arg_list,
    NamedArg => visit_named_arg / walk_named_arg,
    SwitchExprArm => visit_switch_expr_arm / walk_switch_expr_arm,
    LiteralPattern => visit_literal_pattern / walk_literal_pattern,
    ConstPattern => visit_const_pattern / walk_const_pattern,
    ListPattern => visit_list_pattern / walk_list_pattern,
    ListPatternElement => visit_list_pattern_element / walk_list_pattern_element,
    RecordPattern => visit_record_pattern / walk_record_pattern,
    RecordPatternField => visit_record_pattern_field / walk_record_pattern_field,
    MapPattern => visit_map_pattern / walk_map_pattern,
    MapPatternEntry => visit_map_pattern_entry / walk_map_pattern_entry,
    ObjectPattern => visit_object_pattern / walk_object_pattern,
    ObjectPatternField => visit_object_pattern_field / walk_object_pattern_field,
    ErrorNode => visit_error_node,
}

#[test]
fn representative_program_walks_source_children_exactly_once() {
    let source = r#"
        library sample;
        import 'a.dart' as prefix show Visible;
        typedef Callback = int Function(int argument);
        @meta class C<T extends Bound> {
          int field = 90001;
          C(this.field) : assert(field > 90002);
          int method(int parameter) {
            var values = <int>[90003, if (true) 90004,
              for (var index = 90005; index < 90006; index++) index];
            try { target(named: 'hello ${parameter}'); }
            on Failure catch (error, stack) { throw error; }
            finally { cleanup(); }
            target..field!.method<int>(90007);
            switch (parameter) {
              case [var item, ...var rest]: return item;
              default: return field;
            }
          }
        }
    "#;
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut recorder = Recorder::default();
    recorder.visit_program(parsed.tree.program(source));

    // The expected leaf multiset is independent of the walkers. Missing edges
    // lose a name or sentinel; duplicate structural visits fail in record().
    let mut expected_names: Vec<_> = "sample prefix Visible Callback int int argument \
        meta C T Bound int field C field field int method int parameter values int \
        index index index index target named parameter Failure error stack error \
        cleanup target field method int parameter item rest item field"
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    expected_names.sort();
    recorder.names.sort();
    assert_eq!(recorder.names, expected_names);
    assert_eq!(
        recorder.ints,
        [
            "90001", "90002", "90003", "90004", "90005", "90006", "90007"
        ]
    );
    assert_eq!(recorder.null_asserts, 1);
    recorder.assert_reaches_every_node(parsed.tree.root(source));
}

#[test]
fn recovery_nodes_and_retained_declarations_are_walked() {
    let source = "} void retained() {}";
    let parsed = parse(source);
    assert!(!parsed.errors.is_empty());
    let mut recorder = Recorder::default();
    recorder.visit_program(parsed.tree.program(source));
    assert!(recorder.counts["ErrorNode"] > 0);
    assert_eq!(recorder.counts["FunctionDecl"], 1);
    assert_eq!(recorder.names, ["retained"]);
    recorder.assert_reaches_every_node(parsed.tree.root(source));
}

#[test]
fn legacy_typedef_walks_written_parameters_not_synthesized_copies() {
    let source = "typedef int Legacy<T>(T value, {String? label}); \
        typedef Untyped(int value); typedef Generic<T extends Bound>(T value);";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut recorder = Recorder::default();
    recorder.visit_program(parsed.tree.program(source));
    let mut expected: Vec<_> =
        "Legacy T int T value String label Untyped int value Generic T Bound T value"
            .split_whitespace()
            .map(str::to_owned)
            .collect();
    expected.sort();
    recorder.names.sort();
    assert_eq!(recorder.names, expected);
    assert_eq!(recorder.counts["TypeAliasDecl"], 3);
    assert_eq!(recorder.counts["FormalParamList"], 3);
    assert_eq!(recorder.counts["FormalParam"], 4);
    assert!(!recorder.counts.contains_key("FunctionType"));
    assert!(!recorder.counts.contains_key("FunctionTypeParam"));
    recorder.assert_reaches_every_node(parsed.tree.root(source));
}

#[test]
fn explicit_hooks_and_default_leaves_reach_all_retained_syntax() {
    let sources = [
        r#"
            import 'default.dart' if (dart.library.io) 'io.dart' show Visible;
            export 'api.dart' hide Hidden;
            part 'part.dart';
            class C<T extends Base> extends Parent with Mix implements Interface {
              int field;
              C(this.field) : super(1);
              C.redirect() : this(2);
              int get value => field;
              set value(int next) { field = next; }
              C operator +(C other) => this;
            }
            mixin M on Parent implements Interface {}
            extension E on C {}
            extension type ET(int value) implements Interface {}
            enum Choice { first, second; }
            class Alias = Parent with Mix;
            mixin class Mixed() {}
            class Primary(dynamic value) { this {} }
            class Factory { factory Factory() = C; }
            class Initialized { int field; Initialized() : field = 1; }
            class Derived extends Parent { Object method() => super.value; }
            void nativeFunction() native '${nativeArgument}';
        "#,
        r#"
            typedef Callback = (int, {String label}) Function<T>(T value, {int count});
            var closure = <T extends Bound>(T value) => value;
            void f(Object value, [int count = 1]) async {
              var expressions = [
                1, 2.5, 'hello ${value}' ' world', true, null, (value),
                -count, count++, count + 1, count = 2,
                true ? count : 0, value is C, value as C,
                value.field, value[0], target<int>(named: count),
                <int>{1}, <String, int>{'key': 2}, (count, name: value),
                new C(count), const C(1), await future, value!, #symbol,
                target<int>, switch (value) { _ => 1 },
                if (true) 1 else 2, for (var item in values) item, ...values,
                ?value, true ? count : throw value, .named(count),
                <String, int>{
                  ...entries, if (guard) 'a': 1 else 'b': 2,
                  for (var item in values) 'c': item,
                },
              ];
              value..field = 1..method<int>(2);
              value..[index]..[index] = assigned..method()()<int>(argument);
              value..method()!.field;
              for (final (left, right) in pairs) { target(left, right); }
              if (value case [var first, ...var rest] when guard) {
                while (guard) { break; }
              } else { do { continue; } while (guard); }
              for (var i = 0; i < count; i++) { assert(guard, 'message'); }
              void local(int inner) { return; }
              labeled: for (;;) { break labeled; }
              throw value;
            }
        "#,
        r#"
            void f(Object value) {
              switch (value) {
                case const (constant + 1): break;
                case {'key': var entry}: break;
                case C(field: var field): break;
                case (var first, named: var second): break;
                case > 0 && < 10: break;
                case 'literal': break;
                case (var nullable?! as C) || var alternative: break;
                default: ;
              }
              var (first, second) = pair;
              (first, second) = pair;
              try { target(); } on Failure catch (error, stack) { rethrow; }
              finally { cleanup(); }
            }
        "#,
        "part of library_name;",
        "Iterable<dynamic> generator() sync* { yield value; yield* values; }",
    ];
    for source in sources {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let mut recorder = Recorder::default();
        recorder.visit_program(parsed.tree.program(source));
        recorder.assert_reaches_every_node(parsed.tree.root(source));
    }
}

#[test]
fn explicit_hooks_and_default_leaves_reach_recovered_expressions_and_patterns() {
    for (source, recovered_kind) in [
        ("var invalid = @;", NodeKind::ErrorExpr),
        ("void f() { if (value case :) {} }", NodeKind::ErrorPattern),
    ] {
        let parsed = parse(source);
        assert!(!parsed.errors.is_empty());
        let root = parsed.tree.root(source);
        assert!(
            root.descendants().any(|node| node.kind() == recovered_kind),
            "missing expected recovery node {recovered_kind:?}"
        );
        let mut recorder = Recorder::default();
        recorder.visit_program(parsed.tree.program(source));
        recorder.assert_reaches_every_node(root);
    }
}
