//! Runtime traversal coverage complements the exhaustive enum matches in the walkers.
use std::collections::{BTreeMap, BTreeSet};

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::*;

#[derive(Default)]
struct Recorder {
    addresses: BTreeSet<(&'static str, usize)>,
    counts: BTreeMap<&'static str, usize>,
    names: Vec<String>,
    ints: Vec<String>,
    null_asserts: usize,
}

impl Recorder {
    fn record<T>(&mut self, kind: &'static str, node: &T) {
        assert!(
            self.addresses
                .insert((kind, std::ptr::from_ref(node) as usize)),
            "{kind} source node visited more than once"
        );
        *self.counts.entry(kind).or_default() += 1;
    }
}

macro_rules! recording_visitor {
    ($($ty:ident => $hook:ident $(/ $walk:ident)?),* $(,)?) => {
        impl Visitor for Recorder {
            $(fn $hook(&mut self, node: &$ty) {
                self.record(stringify!($ty), node);
                $($walk(self, node);)?
            })*

            fn visit_identifier(&mut self, node: &Identifier) {
                self.record("Identifier", node);
                self.names.push(node.name.clone());
            }

            fn visit_name(&mut self, node: &Identifier) {
                self.visit_identifier(node);
            }

            fn visit_expr(&mut self, node: &Expr) {
                self.record("Expr", node);
                if let Expr::IntLit { value, .. } = node {
                    self.ints.push(value.clone());
                }
                walk_expr(self, node);
            }

            fn visit_cascade_op(&mut self, node: &CascadeOp) {
                self.record("CascadeOp", node);
                if let CascadeOp::NullAssert { .. } = node {
                    self.null_asserts += 1;
                }
                walk_cascade_op(self, node);
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
    FunctionKind => visit_function_kind / walk_function_kind,
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
    Pattern => visit_pattern / walk_pattern,
    LiteralPattern => visit_literal_pattern / walk_literal_pattern,
    LiteralPatternValue => visit_literal_pattern_value / walk_literal_pattern_value,
    ConstPattern => visit_const_pattern / walk_const_pattern,
    ConstPatternValue => visit_const_pattern_value / walk_const_pattern_value,
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
    let parsed = parse(
        r#"
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
    "#,
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut recorder = Recorder::default();
    recorder.visit_program(&parsed.program);

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
    assert_eq!(
        recorder.counts,
        BTreeMap::from([
            ("Program", 1),
            ("Identifier", expected_names.len()),
            ("StringLitNode", 2),
            ("StringInterpolation", 1),
            ("LibraryDirective", 1),
            ("ImportDirective", 1),
            ("ImportCombinator", 1),
            ("TopLevelDecl", 2),
            ("ClassDecl", 1),
            ("ClassMember", 3),
            ("FieldDecl", 1),
            ("VarDeclarator", 3),
            ("ConstructorDecl", 1),
            ("ConstructorInitializer", 1),
            ("MethodDecl", 1),
            ("TypeAliasDecl", 1),
            ("DartType", 10),
            ("NamedType", 9),
            ("FunctionType", 1),
            ("FunctionTypeParam", 1),
            ("TypeParam", 1),
            ("FormalParamList", 2),
            ("FormalParam", 2),
            ("FunctionBody", 1),
            ("Annotation", 1),
            ("Block", 4),
            ("Stmt", 9),
            ("TryCatchStmt", 1),
            ("CatchClause", 1),
            ("ThrowStmt", 1),
            ("ReturnStmt", 2),
            ("ExprStmt", 3),
            ("LocalVarDecl", 2),
            ("SwitchStmt", 1),
            ("SwitchCase", 2),
            ("SwitchCaseKind", 2),
            ("Pattern", 3),
            ("ListPattern", 1),
            ("ListPatternElement", 2),
            ("CascadeSection", 1),
            ("CascadeOp", 3),
            ("CollectionElement", 5),
            ("IfCondition", 1),
            ("ForHeader", 1),
            ("ForInit", 1),
            ("ArgList", 3),
            ("NamedArg", 1),
            ("Expr", 28),
        ])
    );
}

#[test]
fn recovery_nodes_and_retained_declarations_are_walked() {
    let parsed = parse("} void retained() {}");
    assert!(!parsed.errors.is_empty());
    let mut recorder = Recorder::default();
    recorder.visit_program(&parsed.program);
    assert!(recorder.counts["ErrorNode"] > 0);
    assert_eq!(recorder.counts["FunctionDecl"], 1);
    assert_eq!(recorder.names, ["retained"]);
}

#[test]
fn legacy_typedef_walks_written_parameters_not_synthesized_copies() {
    let parsed =
        parse("typedef int Legacy<T>(T value, {String? label}); typedef Untyped(int value);");
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut recorder = Recorder::default();
    recorder.visit_program(&parsed.program);
    let mut expected: Vec<_> = "Legacy T int T value String label Untyped int value"
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    expected.sort();
    recorder.names.sort();
    assert_eq!(recorder.names, expected);
    assert_eq!(recorder.counts["TypeAliasDecl"], 2);
    assert_eq!(recorder.counts["FormalParamList"], 2);
    assert_eq!(recorder.counts["FormalParam"], 3);
    assert!(!recorder.counts.contains_key("FunctionType"));
    assert!(!recorder.counts.contains_key("FunctionTypeParam"));
}
