//! Regression coverage for traversal boundaries and source-child ordering.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::visit::*;
use falcon_syntax::cst::*;

/// Records every leaf the default walk delivers, keyed by kind, so a sentinel's
/// exact-once presence proves traversal reached its position without duplication.
#[derive(Default)]
struct Recorder {
    ints: Vec<String>,
    strings: Vec<String>,
    named_types: Vec<String>,
    idents: Vec<String>,
    dynamic_types: usize,
    annotations: usize,
}

impl Recorder {
    fn has_int(&self, v: &str) -> bool {
        self.ints.iter().filter(|s| s.as_str() == v).count() == 1
    }
    fn has_string(&self, v: &str) -> bool {
        self.strings.iter().filter(|s| s.as_str() == v).count() == 1
    }
    fn has_named(&self, v: &str) -> bool {
        self.named_types.iter().filter(|s| s.as_str() == v).count() == 1
    }
    fn has_ident(&self, v: &str) -> bool {
        self.idents.iter().filter(|s| s.as_str() == v).count() == 1
    }
}

impl<'a> Visitor<'a> for Recorder {
    fn visit_expr(&mut self, node: Expr<'a>) {
        if let Expr::IntLit(value) = node {
            self.ints.push(value.value().to_owned());
        }
        walk_expr(self, node);
    }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) {
        self.strings.push(node.value().into_owned());
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) {
        match node {
            DartType::Named(n) => {
                if let Some(seg) = n.segments().last() {
                    self.named_types.push(seg.name().to_owned());
                }
            }
            DartType::Dynamic(_) => self.dynamic_types += 1,
            _ => {}
        }
        walk_dart_type(self, node);
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) {
        self.idents.push(node.name().to_owned());
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        self.annotations += 1;
        walk_annotation(self, node);
    }
}

fn record(src: &str) -> Recorder {
    let parsed = parse(src);
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parsed;
    assert!(
        errors.is_empty(),
        "unexpected parse errors in {src:?}: {errors:?}"
    );
    let mut r = Recorder::default();
    r.visit_program(tree.program(src));
    r
}

#[test]
fn expression_kinds_cast_and_round_trip_without_losing_syntax() {
    macro_rules! assert_expression_kinds {
        ($($kind:ident => $variant:ident),* $(,)?) => {
            $({
                let source = "x";
                let tree = SyntaxTree::build(source, &[], vec![
                    Event::Start {
                        kind: NodeKind::Program,
                        forward_parent: None,
                        offset: 0,
                    },
                    Event::Start {
                        kind: NodeKind::$kind,
                        forward_parent: None,
                        offset: 0,
                    },
                    Event::Token {
                        kind: falcon_syntax::TokenKind::Ident,
                        len: source.len(),
                        offset: 0,
                    },
                    Event::Finish,
                    Event::Finish,
                ]);
                let syntax = tree.root(source).children().next().unwrap();
                let expression = Expr::cast(syntax)
                    .expect(concat!(stringify!($kind), " must cast to Expr"));
                assert!(matches!(expression, Expr::$variant(_)));
                assert_eq!(expression.syntax(), syntax);
                assert_eq!(<Expr<'_> as AstNode<'_>>::syntax(expression), syntax);
                assert_eq!(expression.span(), syntax.span());
                assert_eq!(<Expr<'_> as AstNode<'_>>::span(expression), syntax.span());
                assert!(Expr::cast(tree.root(source)).is_none());
            })*
        };
    }

    // Keep this independent of expression_enum!: losing a kind's typed cast
    // must fail here rather than silently filtering an expression out of a walk.
    assert_expression_kinds! {
        IntLitExpr => IntLit,
        DoubleLitExpr => DoubleLit,
        StringLitNode => StringLit,
        AdjacentStrings => StringLit,
        BoolLitExpr => BoolLit,
        NullLitExpr => NullLit,
        Identifier => Ident,
        ParenExpr => Paren,
        ThisExpr => This,
        SuperExpr => Super,
        UnaryExpr => Unary,
        PostfixIncDecExpr => PostfixIncDec,
        BinaryExpr => Binary,
        AssignExpr => Assign,
        ConditionalExpr => Conditional,
        IsExpr => Is,
        AsExpr => As,
        FieldExpr => Field,
        IndexExpr => Index,
        CallExpr => Call,
        CascadeExpr => Cascade,
        ListExpr => List,
        MapExpr => Map,
        SetExpr => Set,
        RecordExpr => Record,
        FuncExpr => FuncExpr,
        NewExpr => New,
        DotShorthandExpr => DotShorthand,
        AwaitExpr => Await,
        ThrowExpr => Throw,
        SwitchExpr => Switch,
        NullAssertExpr => NullAssert,
        SymbolLitExpr => SymbolLit,
        GenericInstantiationExpr => GenericInstantiation,
        ErrorExpr => Error,
    }
}

#[test]
fn expression_mapping_preserves_recovery_view_spans() {
    let source = ";";
    let tokens = [
        falcon_syntax::Token::new(falcon_syntax::TokenKind::Semicolon, 0, 1),
        falcon_syntax::Token::new(falcon_syntax::TokenKind::Eof, 1, 0),
    ];
    for kind in [
        NodeKind::Identifier,
        NodeKind::StringLitNode,
        NodeKind::ErrorExpr,
    ] {
        let tree = SyntaxTree::build(
            source,
            &tokens,
            vec![
                Event::Start {
                    kind: NodeKind::Program,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Start {
                    kind,
                    forward_parent: None,
                    offset: 0,
                },
                Event::RecoverySpan(Span::new(0, 1)),
                Event::Finish,
                Event::Finish,
            ],
        );
        let syntax = tree.root(source).children().next().unwrap();
        assert_eq!(syntax.span(), Span::new(0, 0));
        let expression = Expr::cast(syntax).unwrap();
        assert_eq!(expression.syntax(), syntax);
        assert_eq!(expression.span(), Span::new(0, 1));
        assert_eq!(
            <Expr<'_> as AstNode<'_>>::span(expression),
            expression.span()
        );
    }
}

#[test]
fn walks_function_type_parameter_annotations() {
    let r = record("typedef F = void Function(@sentinel(713) int value);");
    assert_eq!(r.annotations, 1);
    assert!(r.has_ident("sentinel"));
    assert!(r.has_int("713"));
}

#[test]
fn walks_catch_types_and_bindings() {
    let r = record("void f() { try {} on CatchType catch (caught, trace) {} }");
    assert!(r.has_named("CatchType"));
    assert!(r.has_ident("caught"));
    assert!(r.has_ident("trace"));
}

#[test]
fn walks_collection_literal_type_arguments() {
    let r = record(
        "void f() { var a = <ListArg>[]; var b = <SetArg>{value}; \
         var c = <MapKey, MapValue>{key: value}; }",
    );
    for name in ["ListArg", "SetArg", "MapKey", "MapValue"] {
        assert!(
            r.has_named(name),
            "collection type argument {name} not walked"
        );
    }
}

#[test]
fn walks_pattern_type_arguments_and_constant_name_segments() {
    let r = record(
        "void f(Object value) { switch (value) { \
         case <PatternList>[a]: break; \
         case <PatternKey, PatternValue>{'key': b}: break; \
         case PatternOwner.member: break; } }",
    );
    for name in ["PatternList", "PatternKey", "PatternValue"] {
        assert!(r.has_named(name), "pattern type argument {name} not walked");
    }
    assert!(r.has_ident("PatternOwner"));
    assert!(r.has_ident("member"));
}

#[test]
fn walks_collection_for_in_header_types_and_bindings() {
    let r = record(
        "void f() { var a = [for (ListItem listItem in source) listItem]; \
         var b = {for (SetItem setItem in source) setItem}; \
         var c = {for (MapItem mapItem in source) mapItem: value}; }",
    );
    for name in ["ListItem", "SetItem", "MapItem"] {
        assert!(r.has_named(name), "for-in header type {name} not walked");
    }
    for name in ["listItem", "setItem", "mapItem"] {
        assert_eq!(
            r.idents
                .iter()
                .filter(|ident| ident.as_str() == name)
                .count(),
            2,
            "for-in binding and body use of {name} must both be walked",
        );
    }
}

#[test]
fn local_var_hook_observes_statement_and_loop_declarations() {
    #[derive(Default)]
    struct Locals {
        declarations: Vec<NodeId>,
    }

    impl<'a> Visitor<'a> for Locals {
        fn visit_local_var(&mut self, declaration: LocalVarDecl<'a>) {
            self.declarations.push(declaration.syntax().id());
        }
    }

    let src = "void f() { var statement = 1; \
     for (var counter = 0; counter < 1; counter++) {} \
     var collection = [for (var item = 0; item < 1; item++) item]; }";
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parse(src);
    let program = tree.program(src);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!("expected function");
    };
    let Some(FunctionBody::Block(body)) = function.body() else {
        panic!("expected block");
    };
    let Stmt::LocalVar(statement) = body.stmts().next().unwrap() else {
        panic!("expected local declaration");
    };
    let Stmt::For(loop_statement) = body.stmts().nth(1).unwrap() else {
        panic!("expected statement loop");
    };
    let Some(ForInit::VarDecl(counter)) = loop_statement.header().init() else {
        panic!("expected loop declaration");
    };
    let Stmt::LocalVar(collection) = body.stmts().nth(2).unwrap() else {
        panic!("expected collection declaration");
    };
    let Some(Expr::List(list)) = collection.declarators().next().unwrap().initializer() else {
        panic!("expected list");
    };
    let CollectionElement::For(element) = list.elements().next().unwrap() else {
        panic!("expected collection loop");
    };
    let Some(ForInit::VarDecl(item)) = element.header().init() else {
        panic!("expected collection loop declaration");
    };

    let mut visitor = Locals::default();
    visitor.visit_stmt(body.stmts().next().unwrap());
    visitor.visit_stmt(body.stmts().nth(1).unwrap());
    visitor.visit_expr(
        collection
            .declarators()
            .next()
            .unwrap()
            .initializer()
            .unwrap(),
    );
    assert_eq!(
        visitor.declarations,
        [
            statement.syntax().id(),
            counter.syntax().id(),
            item.syntax().id(),
        ],
    );
}

#[test]
fn walks_constructor_field_initializer() {
    let r = record("class C { final int a; C() : a = 90001; }");
    assert!(r.has_int("90001"), "field-init value not walked");
}

#[test]
fn walks_constructor_super_call_args() {
    let r = record("class B { B([Object? o]); } class C extends B { C() : super(90002); }");
    assert!(r.has_int("90002"), "super-call arg not walked");
}

#[test]
fn walks_constructor_this_call_args() {
    let r = record("class C { final int a; C() : this._(90003); C._(this.a); }");
    assert!(r.has_int("90003"), "this-call arg not walked");
}

#[test]
fn walks_constructor_assert_condition_and_message() {
    let r = record("class C { C() : assert(90004 > 0, 'msg90005'); }");
    assert!(r.has_int("90004"), "assert condition not walked");
    assert!(r.has_string("msg90005"), "assert message not walked");
}

#[test]
fn walks_annotations_on_every_carrier() {
    let r = record(
        "@A(90010) int tlv = 0;\n\
         @A(90011) extension Ext on int {}\n\
         @A(90012) extension type ET(int v) {}\n\
         class C {\n\
           @A(90013) int f = 0;\n\
           @A(90014) int get g => 0;\n\
           @A(90015) set s(int v) {}\n\
           void m(@A(90016) int p) {}\n\
         }\n\
         enum E { @A(90017) a; }\n",
    );
    for sentinel in [
        "90010", "90011", "90012", "90013", "90014", "90015", "90016", "90017",
    ] {
        assert!(
            r.has_int(sentinel),
            "annotation arg {sentinel} not delivered/walked"
        );
    }
}

#[test]
fn walks_enum_variant_args_and_type_args() {
    let r = record("enum E { a<VarTArg>(90020); const E(int x); }");
    assert!(r.has_int("90020"), "enum variant arg not walked");
    assert!(r.has_named("VarTArg"), "enum variant type-arg not walked");
}

#[test]
fn walks_local_function_param_defaults_and_types() {
    let r = record("void outer() { void g([LocalPT x = 90030]) {} }");
    assert!(r.has_int("90030"), "local-func param default not walked");
    assert!(r.has_named("LocalPT"), "local-func param type not walked");
}

#[test]
fn walks_library_import_export_part_directives() {
    let r = record(
        "library dirlib90040;\n\
         import 'i90041.dart' show Imp90042;\n\
         export 'e90043.dart' show Exp90044;\n\
         part 'p90045.dart';\n",
    );
    assert!(r.has_ident("dirlib90040"), "library name not walked");
    assert!(r.has_string("i90041.dart"), "import uri not walked");
    assert!(r.has_ident("Imp90042"), "import show combinator not walked");
    assert!(r.has_string("e90043.dart"), "export uri not walked");
    assert!(r.has_ident("Exp90044"), "export show combinator not walked");
    assert!(r.has_string("p90045.dart"), "part uri not walked");
}

#[test]
fn walks_part_of_directive() {
    let r = record("part of 'parent90046.dart';\n");
    assert!(r.has_string("parent90046.dart"), "part-of uri not walked");
}

#[test]
fn walks_type_param_bounds_across_declarations() {
    let r = record(
        "class C<T extends CBound> { void m<U extends MBound>() {} }\n\
         void f<T extends FBound>() {}\n\
         typedef Cb<T extends TBound> = void Function();\n\
         mixin Mx<T extends MxBound> {}\n",
    );
    for bound in ["CBound", "MBound", "FBound", "TBound", "MxBound"] {
        assert!(r.has_named(bound), "type-param bound {bound} not walked");
    }
}

#[test]
fn walks_function_expression_type_param_bound() {
    let r = record("var fe = <T extends FeBound>(T x) => x;");
    assert!(
        r.has_named("FeBound"),
        "func-expr type-param bound not walked"
    );
}

#[test]
fn walks_function_typed_formal_type_param_bound() {
    let r = record("void f(int cb<T extends FormalBound>(T x)) {}");
    assert!(
        r.has_named("FormalBound"),
        "function-typed formal type-param bound not walked"
    );
}

#[test]
fn walks_mixin_class_supertypes() {
    let r = record("mixin class MC extends McBase with McMix implements McIface {}\n");
    assert!(r.has_named("McBase"), "mixin-class extends not walked");
    assert!(r.has_named("McMix"), "mixin-class with-clause not walked");
    assert!(r.has_named("McIface"), "mixin-class implements not walked");
}

#[test]
fn walks_extension_type_representation_and_implements() {
    let r = record("extension type ET(EtRepr value) implements EtIface {}");
    assert!(
        r.has_named("EtRepr"),
        "extension-type representation type not walked"
    );
    assert!(
        r.has_named("EtIface"),
        "extension-type implements not walked"
    );
}

#[test]
fn walks_call_type_args() {
    let r = record("void f() { gen<CallTArg>(); } void gen<T>() {}");
    assert!(r.has_named("CallTArg"), "call type-arg not walked");
}

#[test]
fn walks_cascade_call_type_args() {
    let r = record("void f(dynamic x) { x..m<CascTArg>(); }");
    assert!(r.has_named("CascTArg"), "cascade-call type-arg not walked");
}

#[test]
fn walks_literal_pattern_string() {
    let r = record("void f(Object x) { switch (x) { case 'patstr90050': break; } }");
    assert!(
        r.has_string("patstr90050"),
        "literal-pattern string not walked"
    );
}

#[test]
fn walks_operator_params() {
    let r = record("class C { C operator +(OpParam other) => this; }");
    assert!(r.has_named("OpParam"), "operator param type not walked");
}

#[test]
fn walks_function_typed_param_inner_params() {
    let r = record("void f(void cb(FnParam x)) {}");
    assert!(
        r.has_named("FnParam"),
        "function-typed param inner param not walked"
    );
}

#[test]
fn adjacent_string_span_covers_full_run() {
    let src = r"var x = 'a\q' 'b\w';";
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parse(src);
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let prog = tree.program(src);

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected top-level var");
    };
    let Some(Expr::StringLit(lit)) = v.declarators().next().unwrap().initializer() else {
        panic!("expected string literal initializer");
    };

    let full_run = r"'a\q' 'b\w'";
    let expected_start = src.find(full_run).unwrap();
    let expected_end = expected_start + full_run.len();
    assert_eq!(
        lit.span().start,
        expected_start,
        "span.start must be the first fragment"
    );
    assert_eq!(
        lit.span().end,
        expected_end,
        "span.end must be the last fragment"
    );
    assert_eq!(&src[lit.span().start..lit.span().end], full_run);
}

#[test]
fn program_callbacks_preserve_traversal_order_and_coverage() {
    let src = "@tag(1 + 2) int top = 3; \
     class C { \
       int field = 4; \
       C([int value = 5]) : field = 6; \
       Object f([int value = 7]) { \
         for (var i = 8; i < 9; i += 10) { (() { return 11; })(); } \
         return [for (var j = 12; j < 13; j += 14) 15, \
                 for (final x in [16]) 17]; \
       } \
     }";
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parse(src);
    let program = tree.program(src);
    assert!(errors.is_empty(), "{errors:?}");

    struct Expressions(Vec<String>);
    impl<'a> Visitor<'a> for Expressions {
        fn visit_expr(&mut self, expr: Expr<'a>) {
            match expr {
                Expr::Binary(binary) if binary.op() == BinaryOp::Add => self.0.push("+".to_owned()),
                Expr::IntLit(value) => self.0.push(value.value().to_owned()),
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut expressions = Expressions(Vec::new());
    expressions.visit_program(program);
    assert_eq!(
        expressions.0,
        [
            "+", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
            "16", "17",
        ]
    );

    let mut statements = Vec::new();
    for_each_stmt_in_program(program, |stmt| {
        statements.push(match stmt {
            Stmt::For(_) => "for",
            Stmt::Block(_) => "block",
            Stmt::Expr(_) => "expr",
            Stmt::Return(_) => "return",
            _ => "other",
        });
    });
    assert_eq!(statements, ["for", "block", "expr", "return", "return"]);
}

#[test]
fn string_observation_override_does_not_suppress_interpolation_children() {
    // Recorder deliberately overrides visit_string_lit without recursing.
    let recorder = record("void f() { var text = '${90060}'; }");
    assert!(recorder.has_int("90060"));
    assert_eq!(recorder.strings.len(), 1);
}

#[test]
fn source_name_observation_preserves_semantic_identifier_boundary() {
    #[derive(Default)]
    struct Names {
        semantic: Vec<String>,
        source: Vec<String>,
    }

    impl<'a> Visitor<'a> for Names {
        fn visit_identifier(&mut self, node: Identifier<'a>) {
            self.semantic.push(node.name().to_owned());
        }

        fn visit_name(&mut self, node: Identifier<'a>) {
            self.source.push(node.name().to_owned());
        }
    }

    let src = "void f() { target.field; }";
    let result = parse(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let mut names = Names::default();
    names.visit_program(result.tree.program(src));
    assert_eq!(names.semantic, ["f", "target"]);
    assert_eq!(names.source, ["field"]);
}

#[test]
fn non_expression_strings_walk_interpolations_once() {
    let src = "var text = '${90061}';";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Variable(variable) = parsed.tree.program(src).declarations().next().unwrap()
    else {
        panic!("expected variable");
    };
    let Some(Expr::StringLit(literal)) = variable.declarators().next().unwrap().initializer()
    else {
        panic!("expected interpolated string");
    };

    let mut pattern = Recorder::default();
    pattern.visit_literal_pattern_value(LiteralPatternValue::String(literal));
    assert!(pattern.has_int("90061"));

    let src = "void f() native '${90061}';";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Function(function) = parsed.tree.program(src).declarations().next().unwrap()
    else {
        panic!("expected function");
    };
    let body = function.body().expect("expected native body");
    assert!(matches!(body, FunctionBody::Native(_)));
    let mut native = Recorder::default();
    native.visit_function_body(body);
    assert!(native.has_int("90061"));
}

#[test]
fn shorthand_pattern_labels_are_not_duplicate_source_names() {
    #[derive(Default)]
    struct Names {
        bindings: usize,
        labels: usize,
    }

    impl<'a> Visitor<'a> for Names {
        fn visit_identifier(&mut self, node: Identifier<'a>) {
            if matches!(node.name(), "first" | "second") {
                self.bindings += 1;
            }
        }

        fn visit_name(&mut self, node: Identifier<'a>) {
            if matches!(node.name(), "first" | "second") {
                self.labels += 1;
            }
        }
    }

    let src = "void f(Object value) { \
         if (value case (:var first)) {} \
         if (value case C(:var second)) {} \
         }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut names = Names::default();
    names.visit_program(parsed.tree.program(src));
    assert_eq!(names.bindings, 2);
    assert_eq!(names.labels, 0);
}

#[test]
fn default_expression_walk_handles_hundred_thousand_link_arena_chains() {
    use falcon_syntax::TokenKind;

    const LINKS: usize = 100_000;
    for (kind, suffix, identifiers_per_link, types_per_link) in [
        (NodeKind::BinaryExpr, "+a", 1, 0),
        (NodeKind::AssignExpr, "=a", 1, 0),
        (NodeKind::ConditionalExpr, "?a:a", 2, 0),
        (NodeKind::NullAssertExpr, "!", 0, 0),
        (NodeKind::PostfixIncDecExpr, "++", 0, 0),
        (NodeKind::FieldExpr, ".a", 1, 0),
        (NodeKind::IndexExpr, "[a]", 1, 0),
        (NodeKind::CallExpr, "(a)", 1, 0),
        (NodeKind::GenericInstantiationExpr, "<dynamic>", 0, 1),
        (NodeKind::CascadeExpr, "..a", 1, 0),
        (NodeKind::AsExpr, " as dynamic", 0, 1),
        (NodeKind::IsExpr, " is dynamic", 0, 1),
    ] {
        // Use one parsed link to capture valid child shapes, then build the
        // deep arena directly so parser depth policies cannot mask regressions.
        let template_source = format!("var value = a{suffix};");
        let template = parse(&template_source);
        assert!(template.errors.is_empty(), "{:?}", template.errors);
        let TopLevelDecl::Variable(variable) = template
            .tree
            .program(&template_source)
            .declarations()
            .next()
            .unwrap()
        else {
            panic!("expected variable");
        };
        let expr = variable
            .declarators()
            .next()
            .unwrap()
            .initializer()
            .unwrap();
        assert_eq!(expr.syntax().kind(), kind);

        fn append_events(element: SyntaxElement<'_>, base: usize, events: &mut Vec<Event>) {
            match element {
                SyntaxElement::Node(node) => {
                    events.push(Event::Start {
                        kind: node.kind(),
                        forward_parent: None,
                        offset: node.span().start - base,
                    });
                    for child in node.children_with_tokens() {
                        append_events(child, base, events);
                    }
                    events.push(Event::Finish);
                }
                SyntaxElement::Token(token) => events.push(Event::Token {
                    kind: token.kind(),
                    len: token.text().len(),
                    offset: token.offset() - base,
                }),
            }
        }

        fn append_shifted(events: &mut Vec<Event>, template: &[Event], shift: usize) {
            for event in template {
                let mut event = event.clone();
                match &mut event {
                    Event::Start { offset, .. } | Event::Token { offset, .. } => {
                        *offset += shift;
                    }
                    _ => {}
                }
                events.push(event);
            }
        }

        // Assignment values and conditional else branches are right-associative
        // in parsed source; exercise those spines rather than synthetic targets
        // or conditions nested on the left.
        let right_nested = matches!(kind, NodeKind::AssignExpr | NodeKind::ConditionalExpr);
        let children: Vec<_> = expr.syntax().children_with_tokens().collect();
        let edge_children = if right_nested {
            &children[..children.len() - 1]
        } else {
            &children[1..]
        };
        let mut edge_events = Vec::new();
        for child in edge_children {
            append_events(*child, expr.span().start, &mut edge_events);
        }
        let mut source = String::with_capacity(1 + LINKS * suffix.len());
        source.push('a');
        for _ in 0..LINKS {
            source.push_str(suffix);
        }
        let mut events = Vec::with_capacity(3 + LINKS * (edge_events.len() + 2));
        for link in 0..LINKS {
            events.push(Event::Start {
                kind,
                forward_parent: None,
                offset: if right_nested { link * suffix.len() } else { 0 },
            });
            if right_nested {
                append_shifted(&mut events, &edge_events, link * suffix.len());
            }
        }
        let leaf_offset = if right_nested {
            LINKS * suffix.len()
        } else {
            0
        };
        events.extend([
            Event::Start {
                kind: NodeKind::Identifier,
                forward_parent: None,
                offset: leaf_offset,
            },
            Event::Token {
                kind: TokenKind::Ident,
                len: 1,
                offset: leaf_offset,
            },
            Event::Finish,
        ]);
        for link in 0..LINKS {
            if !right_nested {
                append_shifted(&mut events, &edge_events, link * suffix.len());
            }
            events.push(Event::Finish);
        }
        let tree = SyntaxTree::build(&source, &[], events);
        struct Counter {
            identifiers: usize,
            types: usize,
            last_offset: Option<usize>,
        }
        impl Counter {
            fn advance(&mut self, start: usize) {
                if let Some(previous) = self.last_offset {
                    assert!(previous < start);
                }
                self.last_offset = Some(start);
            }
        }
        impl<'a> Visitor<'a> for Counter {
            fn visit_identifier(&mut self, node: Identifier<'a>) {
                self.advance(node.span().start);
                self.identifiers += 1;
            }

            fn visit_name(&mut self, node: Identifier<'a>) {
                self.visit_identifier(node);
            }

            fn visit_dart_type(&mut self, node: DartType<'a>) {
                assert!(matches!(node, DartType::Dynamic(_)));
                self.advance(node.span().start);
                self.types += 1;
            }
        }
        let mut counter = Counter {
            identifiers: 0,
            types: 0,
            last_offset: None,
        };
        counter.visit_expr(Expr::cast(tree.root(&source)).expect("expression root"));
        assert_eq!(
            counter.identifiers,
            1 + LINKS * identifiers_per_link,
            "{kind:?}",
        );
        assert_eq!(counter.types, LINKS * types_per_link, "{kind:?}",);
        let mut expressions = 0;
        for_each_expr(Expr::cast(tree.root(&source)).unwrap(), &mut |_| {
            expressions += 1;
        });
        let expression_children = match kind {
            NodeKind::FieldExpr
            | NodeKind::GenericInstantiationExpr
            | NodeKind::AsExpr
            | NodeKind::IsExpr
            | NodeKind::CascadeExpr => 0,
            _ => identifiers_per_link,
        };
        assert_eq!(
            expressions,
            1 + LINKS * (1 + expression_children),
            "{kind:?}"
        );
    }
}

#[test]
fn expression_helper_preserves_preorder_across_synchronous_children() {
    let source = "var value = a + b * f(c, d);";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Variable(variable) =
        parsed.tree.program(source).declarations().next().unwrap()
    else {
        panic!("expected variable");
    };
    let expression = variable
        .declarators()
        .next()
        .unwrap()
        .initializer()
        .unwrap();
    let mut order = Vec::new();
    for_each_expr(expression, &mut |node| {
        let span = node.span();
        order.push(&source[span.start..span.end]);
    });
    assert_eq!(
        order,
        [
            "a + b * f(c, d)",
            "a",
            "b * f(c, d)",
            "b",
            "f(c, d)",
            "f",
            "c",
            "d"
        ],
    );

    struct Pruned(usize, usize);
    impl<'a> Visitor<'a> for Pruned {
        fn visit_expr(&mut self, node: Expr<'a>) {
            walk_expr_chain_with_exit(
                self,
                node,
                |visitor, _| {
                    visitor.0 += 1;
                    false
                },
                |visitor, _| visitor.1 += 1,
            );
        }
    }
    let mut pruned = Pruned(0, 0);
    pruned.visit_expr(expression);
    assert_eq!((pruned.0, pruned.1), (1, 1));
}

#[test]
fn expression_overrides_keep_synchronous_walk_and_pruning() {
    struct Expressions(Vec<String>);
    impl<'a> Visitor<'a> for Expressions {
        fn visit_expr(&mut self, node: Expr<'a>) {
            let text = node.syntax().text();
            self.0.push(format!("enter {text}"));
            if text != "skip.hidden" {
                walk_expr(self, node);
            }
            self.0.push(format!("exit {text}"));
        }
    }

    let source = "var value = keep.visible + skip.hidden;";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut visitor = Expressions(Vec::new());
    visitor.visit_program(parsed.tree.program(source));
    assert_eq!(
        visitor.0,
        [
            "enter keep.visible + skip.hidden",
            "enter keep.visible",
            "enter keep",
            "exit keep",
            "exit keep.visible",
            "enter skip.hidden",
            "exit skip.hidden",
            "exit keep.visible + skip.hidden",
        ],
    );
}

#[test]
fn default_expression_chains_preserve_suffix_hooks_and_pruning() {
    struct Order(Vec<String>);
    impl<'a> Visitor<'a> for Order {
        fn visit_identifier(&mut self, node: Identifier<'a>) {
            self.0.push(node.name().to_owned());
        }

        fn visit_name(&mut self, node: Identifier<'a>) {
            self.0.push(node.name().to_owned());
        }

        fn visit_dart_type(&mut self, node: DartType<'a>) {
            self.0.push(node.syntax().text().to_owned());
        }

        fn visit_arg_list(&mut self, _node: ArgList<'a>) {
            self.0.push("args".to_owned());
        }
    }

    let source = "var result = target.field<Type>(hidden)[index]!.last as Cast is Check; \
                  var conditional = flag ? yes : no; var assigned = destination = value;";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut visitor = Order(Vec::new());
    visitor.visit_program(parsed.tree.program(source));
    assert_eq!(
        visitor.0,
        [
            "result",
            "target",
            "field",
            "Type",
            "args",
            "index",
            "last",
            "Cast",
            "Check",
            "conditional",
            "flag",
            "yes",
            "no",
            "assigned",
            "destination",
            "value",
        ],
    );
}

#[test]
fn default_pattern_chains_preserve_leaf_and_cast_order() {
    struct Order(Vec<String>);
    impl<'a> Visitor<'a> for Order {
        fn visit_identifier(&mut self, node: Identifier<'a>) {
            self.0.push(node.name().to_owned());
        }

        fn visit_dart_type(&mut self, node: DartType<'a>) {
            self.0.push(node.syntax().text().to_owned());
        }
    }

    let source =
        "void f() { switch (value) { case var a?! as A || var b && var c as C as D: break; } }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let pattern = parsed
        .tree
        .root(source)
        .descendants()
        .find_map(Pattern::cast)
        .unwrap();
    let mut order = Order(Vec::new());
    order.visit_pattern(pattern);
    assert_eq!(order.0, ["a", "A", "b", "c", "C", "D"]);
}

#[test]
fn statement_lists_walk_long_binary_chains_and_nested_closures() {
    let mut source = String::from("void f(int a) { ");
    for _ in 0..99_999 {
        source.push_str("a + ");
    }
    source.push_str("(() { marker(); return 1; })(); }");
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut lists = Vec::new();
    for_each_stmt_list(parsed.tree.program(&source), &mut |statements, kind| {
        lists.push((
            kind,
            statements
                .map(|statement| statement.span().start)
                .collect::<Vec<_>>(),
        ));
    });
    assert_eq!(
        lists,
        [
            (StatementListKind::Block, vec![source.find("a + ").unwrap()]),
            (
                StatementListKind::Block,
                vec![
                    source.find("marker").unwrap(),
                    source.find("return").unwrap()
                ],
            ),
        ]
    );
}

#[test]
fn expression_chain_entries_preserve_preorder_and_pruning() {
    struct Entries(Vec<String>);
    impl<'a> Visitor<'a> for Entries {
        fn visit_expr(&mut self, node: Expr<'a>) {
            walk_expr_chain(self, node, |visitor, node| {
                let text = node.syntax().text();
                visitor.0.push(text.to_owned());
                text != "skip.hidden"
            });
        }
    }
    let source = "var result = target.field + skip.hidden + tail;";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut visitor = Entries(Vec::new());
    visitor.visit_program(parsed.tree.program(source));
    assert_eq!(
        visitor.0,
        [
            "target.field + skip.hidden + tail",
            "target.field + skip.hidden",
            "target.field",
            "target",
            "skip.hidden",
            "tail",
        ],
    );
}

#[test]
fn shared_elements_preserve_nested_order_and_leaf_metadata() {
    #[derive(Default)]
    struct Elements {
        seen: std::collections::BTreeSet<(bool, NodeId)>,
        ints: Vec<String>,
        spreads: Vec<(bool, bool)>,
        loops: Vec<(bool, bool)>,
        entries: Vec<(bool, bool)>,
        null_aware: usize,
    }

    impl<'a> Visitor<'a> for Elements {
        fn visit_element<E: Element<'a>>(&mut self, element: E) {
            assert!(self.seen.insert((E::IS_MAP, element.syntax().id())));
            let syntax = match element.view() {
                ElementView::Collection(original) => {
                    assert!(!E::IS_MAP);
                    assert_eq!(original.span(), element.span());
                    original.syntax()
                }
                ElementView::Map(original) => {
                    assert!(E::IS_MAP);
                    assert_eq!(original.span(), element.span());
                    original.syntax()
                }
            };
            assert_eq!(syntax, element.syntax());
            let kind = element.kind();
            match kind {
                ElementKind::Spread { .. } => {
                    self.spreads
                        .push((E::IS_MAP, element.spread_is_null_aware()));
                }
                ElementKind::For { .. } => {
                    self.loops.push((E::IS_MAP, element.is_await()));
                }
                ElementKind::NullAware(_) => self.null_aware += 1,
                ElementKind::Expression(_) => assert!(!element.is_await()),
                ElementKind::Entry(_) | ElementKind::If { .. } => {}
            }
            walk_element(self, kind);
        }

        fn visit_map_entry(&mut self, entry: MapEntry<'a>) {
            self.entries
                .push((entry.key_null_aware(), entry.value_null_aware()));
            walk_map_entry(self, entry);
        }

        fn visit_expr(&mut self, expression: Expr<'a>) {
            if let Expr::IntLit(value) = expression {
                self.ints.push(value.value().to_owned());
            }
            walk_expr(self, expression);
        }
    }

    let source = r#"
        void f() async {
          var values = [
            if (100)
              for (var i = 101; 102; 103)
                if (104) ?105 else ...?[106]
            else ...[107],
            await for (var item in [108]) item,
            await future,
          ];
          var map = <int, int>{
            if (200)
              for (var j = 201; 202; 203)
                if (204) ?205: ?206 else ...?{207: 208}
            else ...{209: 210},
            await for (var entry in [211]) 212: entry,
          };
        }
    "#;
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut elements = Elements::default();
    elements.visit_program(parsed.tree.program(source));
    assert_eq!(
        elements.ints,
        [
            "100", "101", "102", "103", "104", "105", "106", "107", "108", "200", "201", "202",
            "203", "204", "205", "206", "207", "208", "209", "210", "211", "212",
        ],
    );
    assert_eq!(
        elements.spreads,
        [(false, true), (false, false), (true, true), (true, false)],
    );
    assert_eq!(
        elements.loops,
        [(false, false), (false, true), (true, false), (true, true)],
    );
    assert_eq!(
        elements.entries,
        [(true, true), (false, false), (false, false), (false, false)],
    );
    assert_eq!(elements.null_aware, 1);
    assert_eq!(elements.seen.len(), 23);
}

#[test]
fn shared_element_override_prunes_only_map_loops() {
    #[derive(Default)]
    struct Pruning {
        seen: std::collections::BTreeSet<(bool, NodeId)>,
        kinds: Vec<(bool, &'static str)>,
        names: Vec<String>,
    }

    impl<'a> Visitor<'a> for Pruning {
        fn visit_element<E: Element<'a>>(&mut self, element: E) {
            assert!(self.seen.insert((E::IS_MAP, element.syntax().id())));
            let shape = element.kind();
            let kind = match shape {
                ElementKind::Expression(_) => "expression",
                ElementKind::NullAware(_) => "null-aware",
                ElementKind::Entry(_) => "entry",
                ElementKind::Spread { .. } => "spread",
                ElementKind::If { .. } => "if",
                ElementKind::For { .. } => "for",
            };
            self.kinds.push((E::IS_MAP, kind));
            if E::IS_MAP && kind == "for" {
                return;
            }
            walk_element(self, shape);
        }

        fn visit_identifier(&mut self, identifier: Identifier<'a>) {
            self.names.push(identifier.name().to_owned());
        }
    }

    let source = r#"
        var values = [
          if (listCondition)
            for (var item in listItems)
              if (innerCondition) item else ?listFallback
          else ...listSpread,
        ];
        var map = {
          if (mapCondition)
            for (var hidden in hiddenItems) hiddenKey: hiddenValue
          else visibleKey: visibleValue,
          ...mapSpread,
        };
    "#;
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut visitor = Pruning::default();
    visitor.visit_program(parsed.tree.program(source));
    assert_eq!(
        visitor.names,
        [
            "values",
            "listCondition",
            "item",
            "listItems",
            "innerCondition",
            "item",
            "listFallback",
            "listSpread",
            "map",
            "mapCondition",
            "visibleKey",
            "visibleValue",
            "mapSpread",
        ],
    );
    assert_eq!(
        visitor.kinds,
        [
            (false, "if"),
            (false, "for"),
            (false, "if"),
            (false, "expression"),
            (false, "null-aware"),
            (false, "spread"),
            (true, "if"),
            (true, "for"),
            (true, "entry"),
            (true, "spread"),
        ],
    );
}
