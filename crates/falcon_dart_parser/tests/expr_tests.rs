//! Regression tests for expression-level parser gaps (group `exprs`): each
//! construct must parse with zero errors (unless the gap is specifically about a
//! *missing* diagnostic) and produce a faithful AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;
#[path = "common/only_class.rs"]
mod common_class;
use common_class::only_class;

/// Parse `var x = <expr>;` and retain its source and concrete tree. Wrapping in a top-level variable is the simplest way to drive the
/// expression parser through the public `parse` entry point.
fn parse_expr(expr_src: &str) -> (String, falcon_dart_parser::ParseResult) {
    let source = format!("var x = {expr_src};");
    let parsed = parse(&source);
    (source, parsed)
}

fn initializer(program: Program<'_>) -> Expr<'_> {
    match program
        .declarations()
        .next()
        .expect("top-level declaration")
    {
        TopLevelDecl::Variable(v) => v
            .declarators()
            .next()
            .expect("declarator")
            .initializer()
            .expect("initializer present"),
        other => panic!("expected top-level variable, got {other:?}"),
    }
}

fn parse_ok(expr_src: &str) -> (String, falcon_dart_parser::ParseResult) {
    let (source, parsed) = parse_expr(expr_src);
    let errors = &parsed.errors;
    assert!(
        errors.is_empty(),
        "unexpected errors for `{expr_src}`: {errors:?}"
    );
    (source, parsed)
}

mod common;
#[path = "common/errs.rs"]
mod common_errs;
#[path = "common/first_stmt_expr.rs"]
mod common_expr;
use common::body_stmts;
use common_errs::errs;

#[test]
fn test_leading_spread_then_entry_is_map() {
    let (expr_source, parsed) = parse_ok("{...a, 'k': 1}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Map(_)), "got {expr:?}");
}

#[test]
fn test_leading_spread_then_plain_is_set() {
    let (expr_source, parsed) = parse_ok("{...a, b}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Set(_)), "got {expr:?}");
}

#[test]
fn test_spread_only_defaults_to_set() {
    // `{...a}` is genuinely ambiguous; with no type args it defaults to a Set.
    let (expr_source, parsed) = parse_ok("{...a}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Set(_)), "got {expr:?}");
}

#[test]
fn test_spread_only_two_type_args_is_map() {
    let (expr_source, parsed) = parse_ok("<K, V>{...a}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Map(_)), "got {expr:?}");
}

#[test]
fn test_spread_only_one_type_arg_is_set() {
    let (expr_source, parsed) = parse_ok("<int>{...a}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Set(_)), "got {expr:?}");
}

#[test]
fn test_multiple_spreads_default_to_set() {
    let (expr_source, parsed) = parse_ok("{...a, ...b}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Set(_)), "got {expr:?}");
}

#[test]
fn map_entries_and_control_elements_share_source_order() {
    let (expr_source, parsed) = parse_ok("{'a': 1, ...other, if (ready) 'b': 2, 'c': 3}");
    let Expr::Map(node) = initializer(parsed.tree.program(&expr_source)) else {
        panic!("expected map");
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 4);
    assert!(matches!(
        [
            elements.clone().next().unwrap(),
            elements.clone().nth(1).unwrap(),
            elements.clone().nth(2).unwrap(),
            elements.clone().nth(3).unwrap()
        ],
        [
            MapElement::Entry(_),
            MapElement::Spread(_),
            MapElement::If(_),
            MapElement::Entry(_)
        ]
    ));
    let (expr_source, parsed) = parse_ok("{'a': 1, 'b': 2}");
    let Expr::Map(node) = initializer(parsed.tree.program(&expr_source)) else {
        panic!("expected map");
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 2);
    assert!(matches!(
        [
            elements.clone().next().unwrap(),
            elements.clone().nth(1).unwrap()
        ],
        [MapElement::Entry(_), MapElement::Entry(_)]
    ));
}

#[test]
fn collection_for_headers_preserve_declarations_and_clauses() {
    let (expr_source, parsed) = parse_ok("[for (final int item in items) item]");
    let Expr::List(list) = initializer(parsed.tree.program(&expr_source)) else {
        panic!("expected list");
    };
    assert!(
        matches!(list.elements().next().unwrap(), CollectionElement::For(element)
        if matches!(element.header().init(), Some(ForInit::ForIn(init))
            if init.keyword() == VarKeyword::Final && init.var_type().is_some()))
    );
    let (expr_source, parsed) = parse_ok("[for (final (key, value) in pairs) value]");
    let Expr::List(list) = initializer(parsed.tree.program(&expr_source)) else {
        panic!("expected list");
    };
    assert!(
        matches!(list.elements().next().unwrap(), CollectionElement::For(element)
        if matches!(element.header().init(), Some(ForInit::PatternForIn(init))
            if init.keyword() == VarKeyword::Final))
    );
    let (expr_source, parsed) = parse_ok("{for (var i = 0; i < n; i++) i: i}");
    let Expr::Map(map) = initializer(parsed.tree.program(&expr_source)) else {
        panic!("expected map");
    };
    let MapElement::For(element) = map.elements().next().unwrap() else {
        panic!("expected C-style map for");
    };
    let header = element.header();
    assert!(matches!(header.init(), Some(ForInit::VarDecl(_))));
    assert!(header.condition().is_some());
    assert_eq!(header.updates().len(), 1);
}

#[test]
fn test_single_type_arg_empty_braces_is_set() {
    let (expr_source, parsed) = parse_ok("<int>{}");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Set(node) => assert!(node.elements().is_empty()),
        other => panic!("expected empty Set, got {other:?}"),
    }
}

#[test]
fn test_two_type_args_empty_braces_is_map() {
    let (expr_source, parsed) = parse_ok("<K, V>{}");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Map(node) => {
            let elements = node.elements();
            assert!(elements.is_empty());
        }
        other => panic!("expected empty Map, got {other:?}"),
    }
}

#[test]
fn test_no_type_args_empty_braces_is_map() {
    let (expr_source, parsed) = parse_ok("{}");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(matches!(expr, Expr::Map(_)), "got {expr:?}");
}

#[test]
fn test_generic_instantiation_tearoff() {
    let (expr_source, parsed) = parse_ok("identity<int>");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::GenericInstantiation(node) => {
            let target = node.target();
            let type_args = node.type_args();
            assert!(matches!(target, Expr::Ident(_)));
            assert_eq!(type_args.len(), 1);
        }
        other => panic!("expected GenericInstantiation, got {other:?}"),
    }
}

#[test]
fn test_generic_instantiation_function_type_annotations() {
    for source in [
        "identity<void Function(@deprecated int)>",
        "identity<void Function(@Meta('value', 1 > 0) int)>",
        "identity<void Function(@Meta<int>.named('value') int)>",
    ] {
        let (expr_source, parsed) = parse_ok(source);
        let expr = initializer(parsed.tree.program(&expr_source));
        let Expr::GenericInstantiation(node) = expr else {
            panic!("expected GenericInstantiation, got {expr:?}");
        };
        let type_args = node.type_args();
        let DartType::Function(function) = type_args.clone().next().unwrap() else {
            panic!(
                "expected function type, got {:?}",
                type_args.clone().next().unwrap()
            );
        };
        assert_eq!(function.params().next().unwrap().annotations().len(), 1);
        let (expr_source, parsed) = parse_ok(&format!("{source}()"));
        let invocation = initializer(parsed.tree.program(&expr_source));
        assert!(
            matches!(invocation, Expr::Call(call) if call.type_args().len() == 1),
            "got {invocation:?}"
        );
    }
}

#[test]
fn test_generic_instantiation_multiple_type_args() {
    let (expr_source, parsed) = parse_ok("pair<int, String>");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::GenericInstantiation(node) => assert_eq!(node.type_args().len(), 2),
        other => panic!("expected GenericInstantiation, got {other:?}"),
    }
}

#[test]
fn test_generic_instantiation_as_call_argument() {
    let (expr_source, parsed) = parse_ok("list.map(identity<int>)");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Call(node) => {
            let args = node.args();
            assert_eq!(args.positional().len(), 1);
            assert!(
                matches!(
                    args.positional().next().unwrap(),
                    Expr::GenericInstantiation(_)
                ),
                "arg was {:?}",
                args.positional().next().unwrap()
            );
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn test_less_than_stays_comparison() {
    let (expr_source, parsed) = parse_ok("a < b");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(
        matches!(
            expr,
            Expr::Binary(binary) if binary.op() == BinaryOp::Lt
        ),
        "got {expr:?}"
    );
}

#[test]
fn test_relational_pair_in_args_stay_comparisons() {
    // `f(a < b, c > d)` — both args are comparisons, not generic instantiations.
    let (expr_source, parsed) = parse_ok("f(a < b, c > d)");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Call(node) => {
            let args = node.args();
            assert_eq!(args.positional().len(), 2, "args: {:?}", args.positional());
            assert!(
                matches!(
                    args.positional().next().unwrap(),
                    Expr::Binary(binary) if binary.op() == BinaryOp::Lt
                ),
                "first arg: {:?}",
                args.positional().next().unwrap()
            );
            assert!(
                matches!(
                    args.positional().nth(1).unwrap(),
                    Expr::Binary(binary) if binary.op() == BinaryOp::Gt
                ),
                "second arg: {:?}",
                args.positional().nth(1).unwrap()
            );
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn test_generic_call_still_parses() {
    // `<T>(args)` following the type args must remain a generic Call, not an
    // instantiation.
    let (expr_source, parsed) = parse_ok("identity<int>(3)");
    let expr = initializer(parsed.tree.program(&expr_source));
    assert!(
        matches!(expr, Expr::Call(call) if call.type_args().len() == 1),
        "got {expr:?}"
    );
}

#[test]
fn test_leading_null_aware_cascade() {
    let (expr_source, parsed) = parse_ok("a?..b()");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => assert!(node.is_null_aware()),
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_plain_cascade_not_null_aware() {
    let (expr_source, parsed) = parse_ok("a..b()");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => assert!(!node.is_null_aware()),
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_null_aware_cascade_marks_first_section_only() {
    let (expr_source, parsed) = parse_ok("a?..b..c");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => {
            let is_null_aware = node.is_null_aware();
            let sections = node.sections();
            assert!(is_null_aware);
            assert_eq!(sections.len(), 2);
            match sections.clone().next().unwrap().ops().next().unwrap() {
                CascadeOp::Field(node) => {
                    let is_null_aware = node.is_null_aware();
                    assert!(is_null_aware, "first section should be null-aware")
                }
                other => panic!("expected Field, got {other:?}"),
            }
            match sections.clone().nth(1).unwrap().ops().next().unwrap() {
                CascadeOp::Field(node) => {
                    let is_null_aware = node.is_null_aware();
                    assert!(!is_null_aware, "second section is plain `..`")
                }
                other => panic!("expected Field, got {other:?}"),
            }
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_null_aware_cascade_index_section() {
    let (expr_source, parsed) = parse_ok("a?..[0]");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => {
            let sections = node.sections();
            match sections.clone().next().unwrap().ops().next().unwrap() {
                CascadeOp::Index(node) => {
                    let is_null_aware = node.is_null_aware();
                    assert!(is_null_aware, "index section should be null-aware")
                }
                other => panic!("expected Index, got {other:?}"),
            }
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_plain_cascade_index_section_not_null_aware() {
    let (expr_source, parsed) = parse_ok("a..[0]");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => {
            let sections = node.sections();
            match sections.clone().next().unwrap().ops().next().unwrap() {
                CascadeOp::Index(node) => assert!(!node.is_null_aware()),
                other => panic!("expected Index, got {other:?}"),
            }
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_null_aware_index_selector_cascade_section() {
    // `..?[0]` — the `?[` null-aware index selector must be accepted and marked.
    let (expr_source, parsed) = parse_ok("a..?[0]");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => {
            let sections = node.sections();
            match sections.clone().next().unwrap().ops().next().unwrap() {
                CascadeOp::Index(node) => {
                    let is_null_aware = node.is_null_aware();
                    assert!(is_null_aware, "`?[` index should be null-aware")
                }
                other => panic!("expected Index, got {other:?}"),
            }
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn cascade_index_selector_spans_include_closing_delimiters() {
    for source in ["a..[0]", "a..?[0]", "a..items[ (0) ].field"] {
        let (expr_source, parsed) = parse_ok(source);
        let expr = initializer(parsed.tree.program(&expr_source));
        let Expr::Cascade(node) = expr else {
            panic!("expected cascade");
        };
        let sections = node.sections();
        let span = sections
            .clone()
            .next()
            .unwrap()
            .ops()
            .find_map(|operation| match operation {
                CascadeOp::Index(node) => Some(node.span()),
                _ => None,
            })
            .expect("index selector");
        let prefix_len = "var x = ".len();
        let start = prefix_len + source.find("?[").or_else(|| source.find('[')).unwrap();
        let end = prefix_len + source.find(']').unwrap() + 1;
        assert_eq!((span.start, span.end), (start, end), "{source}");
    }
}

#[test]
fn test_const_type_args_without_collection_records_error() {
    let (_source, parsed) = parse_expr("const <int>");
    let errors = parsed.errors;
    assert!(
        !errors.is_empty(),
        "expected a diagnostic for `const <int>` with no collection literal"
    );
}

#[test]
fn test_collection_if_case_guard_retained() {
    let (expr_source, parsed) = parse_ok("[if (x case int y when y > 0) y]");
    let expr = initializer(parsed.tree.program(&expr_source));
    let elements = match expr {
        Expr::List(node) => node.elements(),
        other => panic!("expected List, got {other:?}"),
    };
    match elements.clone().next().unwrap() {
        CollectionElement::If(node) => {
            let condition = node.condition();
            match condition {
                IfCondition::Case(case) => {
                    let guard = case.guard();
                    assert!(guard.is_some(), "when-guard must be retained");
                }
                other => panic!("expected IfCondition::Case, got {other:?}"),
            }
        }
        other => panic!("expected CollectionElement::If, got {other:?}"),
    }
}

#[test]
fn test_collection_if_case_without_guard() {
    let (expr_source, parsed) = parse_ok("[if (x case int y) y]");
    let expr = initializer(parsed.tree.program(&expr_source));
    let elements = match expr {
        Expr::List(node) => node.elements(),
        other => panic!("expected List, got {other:?}"),
    };
    match elements.clone().next().unwrap() {
        CollectionElement::If(element) => {
            let IfCondition::Case(case) = element.condition() else {
                panic!("expected if-case element");
            };
            let guard = case.guard();
            assert!(guard.is_none());
        }
        other => panic!("expected if-case element, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_identifier() {
    let (expr_source, parsed) = parse_ok("#foo");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::SymbolLit(node) => assert_eq!(node.raw(), "#foo"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_dotted() {
    let (expr_source, parsed) = parse_ok("#bar.baz");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::SymbolLit(node) => assert_eq!(node.raw(), "#bar.baz"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_operator() {
    let (expr_source, parsed) = parse_ok("#+");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::SymbolLit(node) => assert_eq!(node.raw(), "#+"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_index_operator() {
    let (expr_source, parsed) = parse_ok("#[]");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::SymbolLit(node) => assert_eq!(node.raw(), "#[]"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_index_assign_operator() {
    let (expr_source, parsed) = parse_ok("#[]=");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::SymbolLit(node) => assert_eq!(node.raw(), "#[]="),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn cast_binds_tighter_than_less_than() {
    let source = format!("void f() {} {{ {} }}", "async", "var r = a < b as int;");
    let parsed = parse(&source);
    let stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0, "stmts: {stmts:?}");
    // Expect `a < (b as int)`.
    let init = match stmts.clone().next().unwrap() {
        Stmt::LocalVar(v) => v.declarators().next().unwrap().initializer().unwrap(),
        other => panic!("expected LocalVar, got {other:?}"),
    };
    match init {
        Expr::Binary(node) => {
            let op = node.op();
            let right = node.right();
            assert!(matches!(op, BinaryOp::Lt));
            assert!(matches!(right, Expr::As(_)), "right: {right:?}");
        }
        other => panic!("expected Binary(<), got {other:?}"),
    }
}

#[test]
fn cast_then_comparison() {
    // `(x as int) <= y`.
    let source = format!("void f() {} {{ {} }}", "async", "var r = x as int <= y;");
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn is_test_then_typed_collection_ternary() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "var y = a is Foo ? <int>[] : b;"
    );
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn await_for_collection_element() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "var b = [await for (var c in s) ...c];"
    );
    let parsed = parse(&source);
    let stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0, "stmts: {stmts:?}");
    let init = match stmts.clone().next().unwrap() {
        Stmt::LocalVar(v) => v.declarators().next().unwrap().initializer().unwrap(),
        other => panic!("expected LocalVar, got {other:?}"),
    };
    match init {
        Expr::List(node) => {
            let elements = node.elements();
            match elements.clone().next().unwrap() {
                CollectionElement::For(node) => assert!(node.is_await(), "expected await for"),
                other => panic!("expected For element, got {other:?}"),
            }
        }
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn is_not_nullable_function_type() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "if (a is! Object? Function()) return;"
    );
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn const_record_literal() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "var x = const (\"\", Y.Z);"
    );
    let parsed = parse(&source);
    let stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0, "stmts: {stmts:?}");
    let init = match stmts.clone().next().unwrap() {
        Stmt::LocalVar(v) => v.declarators().next().unwrap().initializer().unwrap(),
        other => panic!("expected LocalVar, got {other:?}"),
    };
    match init {
        Expr::Record(node) => {
            let is_const = node.is_const();
            let fields = node.fields();
            assert!(is_const, "expected const record");
            assert_eq!(fields.len(), 2);
        }
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn relational_lt_with_shift_in_ternary() {
    assert_eq!(errs("var q = a < b ? (d >> (e)) : 0;"), 0);
}

#[test]
fn method_type_args_nested_generic_in_function_type_param() {
    // `Pointer<Uint32> count` is a generic-typed named parameter inside the
    // function type, and the list closes with a `>>>` shift token — both of which
    // used to make the type-arg scan fall back to a `<` comparison.
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "p.cast<Pointer<NativeFunction<Int32 Function(Pointer<Uint32> count)>>>();"
    );
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn typed_collections_do_not_enter_generic_function_lookahead() {
    let (expr_source, parsed) = parse_ok("f(<int>[1], () {})");
    let Expr::Call(node) = initializer(parsed.tree.program(&expr_source)) else {
        panic!("expected call");
    };
    let args = node.args();
    assert_eq!(args.positional().len(), 2);
    assert!(matches!(args.positional().next().unwrap(), Expr::List(_)));
    assert!(matches!(
        args.positional().nth(1).unwrap(),
        Expr::FuncExpr(_)
    ));
    let (expr_source, parsed) = parse_ok("<List<int>>[]");
    assert!(matches!(
        initializer(parsed.tree.program(&expr_source)),
        Expr::List(_)
    ));
}

#[test]
fn method_type_args_survive_speculative_typed_decl_rollback() {
    // As a bare expression statement the `>>>` close is first scanned by the
    // speculative typed-declaration path; its rolled-back type parse must restore
    // the split shift token so the committed re-parse still sees it.
    assert_eq!(
        errs("void f() { p.cast<Pointer<NativeFunction<Int32 Function(Pointer<Uint32> c)>>>(); }"),
        0
    );
}

#[test]
fn multi_arg_method_type_args() {
    let source = format!("void f() {} {{ {} }}", "async", "x.cast<A, B>();");
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn record_type_with_generic_field_as_type_arg() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "var q = Queue<(Uri sourceDoc, Ref<Parseable> ref)>();"
    );
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn record_type_type_arg_with_nested_generic() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "isA<(String, List<String>)>();"
    );
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn cascade_on_as_cast() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "y = x as web.HTMLMetaElement ..name = 'n';"
    );
    let parsed = parse(&source);
    let stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0, "stmts: {stmts:?}");
    // The cascade wraps the whole `(x as T)`.
    let value = match stmts.clone().next().unwrap() {
        Stmt::Expr(s) => match s.expr() {
            Expr::Assign(node) => node.value(),
            other => panic!("expected Assign, got {other:?}"),
        },
        other => panic!("expected Expr stmt, got {other:?}"),
    };
    match value {
        Expr::Cascade(node) => {
            let object = node.object();
            assert!(matches!(object, Expr::As(_)), "object: {object:?}");
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn plain_cascade_still_parses() {
    let source = format!("void f() {} {{ {} }}", "async", "y..a()..b = 1;");
    let parsed = parse(&source);
    let _stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
}

#[test]
fn const_generic_constructor_statement() {
    let source = format!(
        "void f() {} {{ {} }}",
        "async", "const Optional<int>.absent();"
    );
    let parsed = parse(&source);
    let stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0, "stmts: {stmts:?}");
    let expr = match stmts.clone().next().unwrap() {
        Stmt::Expr(s) => s.expr(),
        other => panic!("expected Expr stmt, got {other:?}"),
    };
    assert!(
        matches!(expr, Expr::New(node) if node.is_const()),
        "expr: {expr:?}"
    );
}

#[test]
fn const_generic_typed_local_decl_still_parses() {
    // Regression guard: `const List<int> xs = []` stays a const var declaration.
    let source = format!("void f() {} {{ {} }}", "async", "const List<int> xs = [];");
    let parsed = parse(&source);
    let stmts = body_stmts(parsed.tree.program(&source));
    let e = parsed.errors.len();
    assert_eq!(e, 0);
    assert!(
        matches!(stmts.clone().next().unwrap(), Stmt::LocalVar(v) if v.keyword() == VarKeyword::Const)
    );
}

#[test]
fn cascade_section_parses_full_selector_chain() {
    // A cascade section is a whole selector chain, not just its first selector:
    // `..b.c()` is one section holding a Field then a Call. The chain tail used to
    // be dropped, so this only "worked" at statement level by mis-splitting into a
    // separate dot-shorthand statement.
    let (expr_source, parsed) = parse_ok("a..b.c()");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => {
            let sections = node.sections();
            assert_eq!(sections.len(), 1);
            assert_eq!(sections.clone().next().unwrap().ops().len(), 2);
            assert!(
                matches!(
                    [sections.clone().next().unwrap().ops().next().unwrap(), sections.clone().next().unwrap().ops().nth(1).unwrap()],
                    [CascadeOp::Field(f), CascadeOp::Call(c)]
                        if f.name().name() == "b" && c.name().name() == "c"
                ),
                "ops: {:?}",
                sections.clone().next().unwrap().ops()
            );
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn cascade_selector_chain_parses_in_nested_positions() {
    // The chain must parse identically inside parentheses, call arguments, and
    // collection literals — the nested positions where the restricted cascade
    // grammar used to error with "expected RParen, got Dot".
    assert_eq!(errs("void f() { (x..a.b()); }"), 0);
    assert_eq!(errs("void f() { g(x..a.b()); }"), 0);
    assert_eq!(errs("void f() { var l = [x..a.b()]; }"), 0);
    assert_eq!(errs("void f() { (x..m().n()); }"), 0);
    assert_eq!(errs("void f() { (x..a[i] = 1); }"), 0);
}

#[test]
fn cascade_null_aware_selector_within_chain() {
    // `?.`/`?[` selectors are allowed *within* a cascade section chain, distinct
    // from a leading `?..`. The null-aware field selector is marked.
    assert_eq!(errs("void f() { x..a?.b(); }"), 0);
    let (expr_source, parsed) = parse_ok("a..b?.c");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Cascade(node) => {
            let sections = node.sections();
            assert_eq!(sections.clone().next().unwrap().ops().len(), 2);
            assert!(
                matches!(
                    [sections.clone().next().unwrap().ops().next().unwrap(), sections.clone().next().unwrap().ops().nth(1).unwrap()],
                    [CascadeOp::Field(b), CascadeOp::Field(c)]
                        if b.name().name() == "b" && !b.is_null_aware() && c.name().name() == "c" && c.is_null_aware()
                ),
                "ops: {:?}",
                sections.clone().next().unwrap().ops()
            )
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn cascade_null_assertions_continue_into_each_selector_kind() {
    for source in [
        "c..parent!.go(c)",
        "c..parent![0]",
        "c..callback!()",
        "c..callback!<int>()",
        "c..m()!.x",
        "c..[0]!.x",
        "c..parent!.x = 1",
        "c..parent![0] = 1",
    ] {
        let (expr_source, parsed) = parse_ok(source);
        let Expr::Cascade(node) = initializer(parsed.tree.program(&expr_source)) else {
            panic!("expected cascade");
        };
        let sections = node.sections();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections.clone().next().unwrap().ops().len(), 3);
        assert!(matches!(
            sections.clone().next().unwrap().ops().nth(1).unwrap(),
            CascadeOp::NullAssert(_)
        ));
        match source {
            "c..m()!.x" => {
                assert!(matches!(
                    sections.clone().next().unwrap().ops().next().unwrap(),
                    CascadeOp::Call(..)
                ));
                assert!(matches!(
                    sections.clone().next().unwrap().ops().nth(2).unwrap(),
                    CascadeOp::Field(_)
                ));
            }
            "c..[0]!.x" => {
                assert!(matches!(
                    sections.clone().next().unwrap().ops().next().unwrap(),
                    CascadeOp::Index(_)
                ));
                assert!(matches!(
                    sections.clone().next().unwrap().ops().nth(2).unwrap(),
                    CascadeOp::Field(_)
                ));
            }
            "c..parent!.x = 1" => {
                assert!(matches!(
                    sections.clone().next().unwrap().ops().nth(2).unwrap(),
                    CascadeOp::Assign(..)
                ));
            }
            "c..parent![0] = 1" => {
                assert!(matches!(
                    sections.clone().next().unwrap().ops().nth(2).unwrap(),
                    CascadeOp::IndexAssign(..)
                ));
            }
            _ => {}
        }
    }
    for source in [
        "void f() { g(c..parent!.go(c)); }",
        "void f() { var x = [c..parent!.go(c)]; }",
    ] {
        assert_eq!(errs(source), 0, "{source}");
    }
}

#[test]
fn paren_expr_initializer_before_block_body_is_not_a_lambda() {
    // `C(): g = (() => b) {}` — the trailing `{}` is the constructor body, so the
    // `(...)` is a parenthesized expression, not a lambda parameter list. The
    // `(...) {` shape used to be mis-detected as a function literal.
    let source = "class C { var g; C(): g = (() => b) {} }";
    let parsed = parse("class C { var g; C(): g = (() => b) {} }");
    let errors = &parsed.errors;
    let prog = parsed.tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let class = only_class(prog);
    let ctor = class
        .members()
        .find_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .expect("constructor present");
    assert!(matches!(ctor.body(), Some(FunctionBody::Block(_))));
    match ctor.initializers().next().unwrap() {
        ConstructorInitializer::FieldInit(node) => {
            let value = node.value();
            assert!(
                matches!(value.unparenthesized(), Expr::FuncExpr(_)),
                "value: {value:?}"
            );
        }
        other => panic!("expected field initializer, got {other:?}"),
    }
}

#[test]
fn nested_paren_arithmetic_initializer_before_block_body() {
    // The same rollback must cover a plain parenthesized arithmetic expression.
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("class C { var g; C(): g = (((o + 1) << 8) | 1) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn valid_param_list_initializer_before_block_body_is_not_a_lambda() {
    // `C(): g = (e) {}` — `(e)` is a valid formal-parameter list, so the
    // speculative lambda parse *succeeds*, yet the trailing `{}` is the
    // constructor body: the initializer value is the parenthesized expression
    // `(e)`, not a lambda `(e) {}`.
    let source = "class C { var g; C(): g = (e) {} }";
    let parsed = parse("class C { var g; C(): g = (e) {} }");
    let errors = &parsed.errors;
    let prog = parsed.tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let class = only_class(prog);
    let ctor = class
        .members()
        .find_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .expect("constructor present");
    assert!(matches!(ctor.body(), Some(FunctionBody::Block(_))));
    match ctor.initializers().next().unwrap() {
        ConstructorInitializer::FieldInit(node) => {
            let value = node.value();
            assert!(
                !matches!(value, Expr::FuncExpr(_)),
                "value should be a parenthesized expr, not a lambda: {value:?}"
            );
        }
        other => panic!("expected field initializer, got {other:?}"),
    }
}

#[test]
fn lambda_in_ctor_init_argument_is_still_a_lambda() {
    // The ctor-body disambiguation must not leak into bracketed sub-parses:
    // `foo((e) {})` inside an initializer is a genuine lambda argument.
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("class C { var g; C(): g = foo((e) {}) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

// The Dart SDK does not permit an *unparenthesized* function literal anywhere in
// a constructor field-initializer expression: a `(...)` there is always a
// parenthesized/record expression, and a trailing `{`/`=>` opens the constructor
// *body*. So the ctor-body disambiguation is honored across the whole initializer
// expression — conditional branches, binary RHS, and the outermost position — not
// just the first primary. The three tests below lock in that SDK-matching
// strictness (verified against Dart 3.11.5 `dart analyze`, which reports syntax
// errors for each). To use a closure in an initializer it must be bracketed, e.g.
// `g = (() {})` or `g = [() {}]` — those remain accepted (tests above).

#[test]
fn bare_block_lambda_conditional_branch_in_ctor_init_matches_sdk_rejection() {
    // `C(): g = c ? () {} : null;` — the SDK parses `c ? ()` (empty record) and then
    // treats `{` as the constructor body, leaving the conditional dangling: a syntax
    // error. `() {}` is *not* accepted as the then-branch closure. Falcon must agree.
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("class C { var g; C(): g = c ? () {} : null; }");
    assert!(
        !errors.is_empty(),
        "SDK rejects a bare block closure in a ctor-init conditional branch; \
         falcon must too, got no errors"
    );
}

#[test]
fn bare_block_lambda_binary_rhs_in_ctor_init_matches_sdk_rejection() {
    // `C(): g = f + () {};` — the SDK parses `f + ()` and takes `{}` as the ctor body,
    // leaving `;` stranded: a syntax error. Not a `f + <closure>` binary expression.
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("class C { var g; C(): g = f + () {}; }");
    assert!(
        !errors.is_empty(),
        "SDK rejects a bare block closure on a ctor-init binary RHS; \
         falcon must too, got no errors"
    );
}

#[test]
fn bare_block_lambda_outermost_in_ctor_init_is_ctor_body_then_stray_semicolon() {
    // `C(): g = () {};` — even outermost, `()` is a record and `{}` is the ctor body,
    // so the trailing `;` is a stray token: the SDK reports a syntax error, as must we.
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("class C { var g; C(): g = () {}; }");
    assert!(
        !errors.is_empty(),
        "SDK treats `() {{}}` as record + ctor body, stranding `;`; \
         falcon must report the same, got no errors"
    );
}

#[test]
fn test_assignment_inside_parens() {
    // `(v = 0)` is an assignment expression, valid as a parenthesised subject.
    let (expr_source, parsed) = parse_ok("(v = 0) ?? 0");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Binary(node) if matches!(node.op(), BinaryOp::NullCoalesce) => {
            let left = node.left();
            assert!(
                matches!(left.unparenthesized(), Expr::Assign(_)),
                "left of ?? should be an assignment, got {left:?}"
            )
        }
        other => panic!("expected `?? ` binary, got {other:?}"),
    }
}

#[test]
fn test_assignment_in_both_conditional_branches() {
    // `a ? b = c : d = e` — both ternary branches are assignments.
    let (expr_source, parsed) = parse_ok("a ? b = c : d = e");
    let expr = initializer(parsed.tree.program(&expr_source));
    match expr {
        Expr::Conditional(node) => {
            let then_expr = node.then_expr();
            let else_expr = node.else_expr();
            assert!(matches!(then_expr, Expr::Assign(_)), "then: {then_expr:?}");
            assert!(matches!(else_expr, Expr::Assign(_)), "else: {else_expr:?}");
        }
        other => panic!("expected conditional, got {other:?}"),
    }
}

/// Parse `void f(o) { <src> }` and retain its source and concrete tree.
fn first_stmt_expr(src: &str) -> (String, falcon_dart_parser::ParseResult) {
    let source = format!("void f(o) {{ {src} }}");
    let parsed = parse(&source);
    (source, parsed)
}

#[test]
fn test_cascade_assign_rhs_excludes_following_cascade_section() {
    // `o..a = 1..b = 2` is TWO sections on `o` (o.a=1; o.b=2), not a single
    // section whose value is a nested cascade `1..b=2`. The assignment RHS is
    // parsed as `expressionWithoutCascade`, so the second `..` reattaches to `o`.
    let (source, parsed) = first_stmt_expr("o..a = 1..b = 2;");
    let expr = common_expr::first_stmt_expr(parsed.tree.program(&source));
    let errors = parsed.errors.len();
    assert_eq!(errors, 0, "unexpected errors: {expr:?}");
    let (object, sections) = match expr {
        Expr::Cascade(node) => {
            let object = node.object();
            let sections = node.sections();
            (object, sections)
        }
        other => panic!("expected cascade, got {other:?}"),
    };
    assert!(matches!(object, Expr::Ident(id) if id.name() == "o"));
    assert_eq!(sections.len(), 2, "expected two sections: {sections:?}");
    for section in sections {
        match section.ops().next().unwrap() {
            // Each section is a bare `field = intLit` assignment; the RHS must be
            // the int literal, never a nested Cascade.
            CascadeOp::Assign(assignment) => {
                let value = assignment.value();
                assert!(
                    matches!(value, Expr::IntLit(_)),
                    "RHS should be an int literal, got {value:?}"
                );
            }
            other => panic!("expected assign op, got {other:?}"),
        }
    }
}

#[test]
fn test_cascade_index_assign_rhs_excludes_following_section() {
    // Same rule for index-selector assignments: `o..[0] = 1..[1] = 2`.
    let (source, parsed) = first_stmt_expr("o..[0] = 1..[1] = 2;");
    let expr = common_expr::first_stmt_expr(parsed.tree.program(&source));
    let errors = parsed.errors.len();
    assert_eq!(errors, 0, "unexpected errors: {expr:?}");
    match expr {
        Expr::Cascade(node) => {
            let sections = node.sections();
            assert_eq!(sections.len(), 2, "expected two sections: {sections:?}")
        }
        other => panic!("expected cascade, got {other:?}"),
    }
}

#[test]
fn test_deep_unary_chain_does_not_overflow() {
    // A long prefix-operator chain recurses once per operator through
    // `parse_unary`; without the depth guard this overflows the stack and
    // aborts. The guard must instead emit a "nesting too deep" error. The chain
    // trips the guard at a bounded depth, so it fits the default test stack.
    let src = format!("var x = {}1;", "-".repeat(1000));
    let falcon_dart_parser::ParseResult { errors, .. } = parse(&src);
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("nesting too deep")),
        "expected a nesting-too-deep error, got {errors:?}"
    );
}

#[test]
fn test_deep_parens_do_not_overflow() {
    // Deeply nested parentheses recurse through the full expression ladder. Run
    // on a large-stack worker so the guard (which fires well above realistic
    // nesting) is reached before this debug build's small default test stack is
    // exhausted — the point is that parsing terminates with an error, never a
    // process abort.
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let n = 3000;
            let src = format!("var x = {}1{};", "(".repeat(n), ")".repeat(n));
            let falcon_dart_parser::ParseResult { errors, .. } = parse(&src);
            errors
                .iter()
                .any(|e| e.message.contains("nesting too deep"))
        })
        .unwrap();
    assert!(
        handle.join().expect("parser thread must not panic/abort"),
        "expected a nesting-too-deep error for deeply nested parens"
    );
}
