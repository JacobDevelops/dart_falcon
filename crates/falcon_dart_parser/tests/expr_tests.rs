//! Regression tests for expression-level parser gaps (group `exprs`): each
//! construct must parse with zero errors (unless the gap is specifically about a
//! *missing* diagnostic) and produce a faithful AST shape.

use falcon_dart_parser::parser::{ParseError, parse};
use falcon_syntax::ast::*;
#[path = "common/only_class.rs"]
mod common_class;
use common_class::only_class;

/// Parse `var x = <expr>;` and return the initializer expression plus any parse
/// errors. Wrapping in a top-level variable is the simplest way to drive the
/// expression parser through the public `parse` entry point.
fn parse_expr(expr_src: &str) -> (Expr, Vec<ParseError>) {
    let src = format!("var x = {expr_src};");
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(&src);
    let init = match &prog.declarations[0] {
        TopLevelDecl::Variable(v) => v.declarators[0]
            .initializer
            .clone()
            .expect("initializer present"),
        other => panic!("expected top-level variable, got {other:?}"),
    };
    (init, errors)
}

fn parse_ok(expr_src: &str) -> Expr {
    let (expr, errors) = parse_expr(expr_src);
    assert!(
        errors.is_empty(),
        "unexpected errors for `{expr_src}`: {errors:?}"
    );
    expr
}

mod common;
#[path = "common/errs.rs"]
mod common_errs;
#[path = "common/first_stmt_expr.rs"]
mod common_expr;
use common::parse_body;
use common_errs::errs;

#[test]
fn test_leading_spread_then_entry_is_map() {
    let expr = parse_ok("{...a, 'k': 1}");
    assert!(matches!(expr, Expr::Map { .. }), "got {expr:?}");
}

#[test]
fn test_leading_spread_then_plain_is_set() {
    let expr = parse_ok("{...a, b}");
    assert!(matches!(expr, Expr::Set { .. }), "got {expr:?}");
}

#[test]
fn test_spread_only_defaults_to_set() {
    // `{...a}` is genuinely ambiguous; with no type args it defaults to a Set.
    let expr = parse_ok("{...a}");
    assert!(matches!(expr, Expr::Set { .. }), "got {expr:?}");
}

#[test]
fn test_spread_only_two_type_args_is_map() {
    let expr = parse_ok("<K, V>{...a}");
    assert!(matches!(expr, Expr::Map { .. }), "got {expr:?}");
}

#[test]
fn test_spread_only_one_type_arg_is_set() {
    let expr = parse_ok("<int>{...a}");
    assert!(matches!(expr, Expr::Set { .. }), "got {expr:?}");
}

#[test]
fn test_multiple_spreads_default_to_set() {
    let expr = parse_ok("{...a, ...b}");
    assert!(matches!(expr, Expr::Set { .. }), "got {expr:?}");
}

#[test]
fn map_entries_and_control_elements_share_source_order() {
    let Expr::Map { elements, .. } = parse_ok("{'a': 1, ...other, if (ready) 'b': 2, 'c': 3}")
    else {
        panic!("expected map");
    };
    assert!(matches!(
        &elements[..],
        [
            MapElement::Entry(_),
            MapElement::Spread { .. },
            MapElement::If { .. },
            MapElement::Entry(_)
        ]
    ));
    let Expr::Map { elements, .. } = parse_ok("{'a': 1, 'b': 2}") else {
        panic!("expected map");
    };
    assert!(matches!(
        &elements[..],
        [MapElement::Entry(_), MapElement::Entry(_)]
    ));
}

#[test]
fn collection_for_headers_preserve_declarations_and_clauses() {
    let Expr::List { elements, .. } = parse_ok("[for (final int item in items) item]") else {
        panic!("expected list");
    };
    assert!(matches!(
        &elements[0],
        CollectionElement::For { header, .. }
            if matches!(
                &**header,
                ForHeader {
                    init: Some(ForInit::ForIn {
                        keyword: VarKeyword::Final,
                        var_type: Some(_),
                        ..
                    }),
                    ..
                }
            )
    ));
    let Expr::List { elements, .. } = parse_ok("[for (final (key, value) in pairs) value]") else {
        panic!("expected list");
    };
    assert!(matches!(
        &elements[0],
        CollectionElement::For { header, .. }
            if matches!(
                &**header,
                ForHeader {
                    init: Some(ForInit::PatternForIn {
                        keyword: VarKeyword::Final,
                        ..
                    }),
                    ..
                }
            )
    ));
    let Expr::Map { elements, .. } = parse_ok("{for (var i = 0; i < n; i++) i: i}") else {
        panic!("expected map");
    };
    let MapElement::For {
        header: ForHeader {
            init,
            condition,
            updates,
        },
        ..
    } = &elements[0]
    else {
        panic!("expected C-style map for");
    };
    assert!(matches!(init.as_ref(), Some(ForInit::VarDecl(_))));
    assert!(condition.is_some());
    assert_eq!(updates.len(), 1);
}

#[test]
fn test_single_type_arg_empty_braces_is_set() {
    let expr = parse_ok("<int>{}");
    match expr {
        Expr::Set { elements, .. } => assert!(elements.is_empty()),
        other => panic!("expected empty Set, got {other:?}"),
    }
}

#[test]
fn test_two_type_args_empty_braces_is_map() {
    let expr = parse_ok("<K, V>{}");
    match expr {
        Expr::Map { elements, .. } => {
            assert!(elements.is_empty());
        }
        other => panic!("expected empty Map, got {other:?}"),
    }
}

#[test]
fn test_no_type_args_empty_braces_is_map() {
    let expr = parse_ok("{}");
    assert!(matches!(expr, Expr::Map { .. }), "got {expr:?}");
}

#[test]
fn test_generic_instantiation_tearoff() {
    let expr = parse_ok("identity<int>");
    match expr {
        Expr::GenericInstantiation {
            target, type_args, ..
        } => {
            assert!(matches!(*target, Expr::Ident(_)));
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
        let expr = parse_ok(source);
        let Expr::GenericInstantiation { type_args, .. } = expr else {
            panic!("expected GenericInstantiation, got {expr:?}");
        };
        let DartType::Function(function) = &type_args[0] else {
            panic!("expected function type, got {:?}", type_args[0]);
        };
        assert_eq!(function.params[0].annotations.len(), 1);
        let invocation = parse_ok(&format!("{source}()"));
        assert!(
            matches!(invocation, Expr::Call { ref type_args, .. } if type_args.len() == 1),
            "got {invocation:?}"
        );
    }
}

#[test]
fn test_generic_instantiation_multiple_type_args() {
    let expr = parse_ok("pair<int, String>");
    match expr {
        Expr::GenericInstantiation { type_args, .. } => assert_eq!(type_args.len(), 2),
        other => panic!("expected GenericInstantiation, got {other:?}"),
    }
}

#[test]
fn test_generic_instantiation_as_call_argument() {
    let expr = parse_ok("list.map(identity<int>)");
    match expr {
        Expr::Call { args, .. } => {
            assert_eq!(args.positional.len(), 1);
            assert!(
                matches!(args.positional[0], Expr::GenericInstantiation { .. }),
                "arg was {:?}",
                args.positional[0]
            );
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn test_less_than_stays_comparison() {
    let expr = parse_ok("a < b");
    assert!(
        matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::Lt,
                ..
            }
        ),
        "got {expr:?}"
    );
}

#[test]
fn test_relational_pair_in_args_stay_comparisons() {
    // `f(a < b, c > d)` — both args are comparisons, not generic instantiations.
    let expr = parse_ok("f(a < b, c > d)");
    match expr {
        Expr::Call { args, .. } => {
            assert_eq!(args.positional.len(), 2, "args: {:?}", args.positional);
            assert!(
                matches!(
                    args.positional[0],
                    Expr::Binary {
                        op: BinaryOp::Lt,
                        ..
                    }
                ),
                "first arg: {:?}",
                args.positional[0]
            );
            assert!(
                matches!(
                    args.positional[1],
                    Expr::Binary {
                        op: BinaryOp::Gt,
                        ..
                    }
                ),
                "second arg: {:?}",
                args.positional[1]
            );
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn test_generic_call_still_parses() {
    // `<T>(args)` following the type args must remain a generic Call, not an
    // instantiation.
    let expr = parse_ok("identity<int>(3)");
    assert!(
        matches!(expr, Expr::Call { ref type_args, .. } if type_args.len() == 1),
        "got {expr:?}"
    );
}

#[test]
fn test_leading_null_aware_cascade() {
    let expr = parse_ok("a?..b()");
    match expr {
        Expr::Cascade { is_null_aware, .. } => assert!(is_null_aware),
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_plain_cascade_not_null_aware() {
    let expr = parse_ok("a..b()");
    match expr {
        Expr::Cascade { is_null_aware, .. } => assert!(!is_null_aware),
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_null_aware_cascade_marks_first_section_only() {
    let expr = parse_ok("a?..b..c");
    match expr {
        Expr::Cascade {
            is_null_aware,
            sections,
            ..
        } => {
            assert!(is_null_aware);
            assert_eq!(sections.len(), 2);
            match &sections[0].ops[0] {
                CascadeOp::Field { is_null_aware, .. } => {
                    assert!(*is_null_aware, "first section should be null-aware")
                }
                other => panic!("expected Field, got {other:?}"),
            }
            match &sections[1].ops[0] {
                CascadeOp::Field { is_null_aware, .. } => {
                    assert!(!*is_null_aware, "second section is plain `..`")
                }
                other => panic!("expected Field, got {other:?}"),
            }
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_null_aware_cascade_index_section() {
    let expr = parse_ok("a?..[0]");
    match expr {
        Expr::Cascade { sections, .. } => match &sections[0].ops[0] {
            CascadeOp::Index { is_null_aware, .. } => {
                assert!(*is_null_aware, "index section should be null-aware")
            }
            other => panic!("expected Index, got {other:?}"),
        },
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_plain_cascade_index_section_not_null_aware() {
    let expr = parse_ok("a..[0]");
    match expr {
        Expr::Cascade { sections, .. } => match &sections[0].ops[0] {
            CascadeOp::Index { is_null_aware, .. } => assert!(!*is_null_aware),
            other => panic!("expected Index, got {other:?}"),
        },
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_null_aware_index_selector_cascade_section() {
    // `..?[0]` — the `?[` null-aware index selector must be accepted and marked.
    let expr = parse_ok("a..?[0]");
    match expr {
        Expr::Cascade { sections, .. } => match &sections[0].ops[0] {
            CascadeOp::Index { is_null_aware, .. } => {
                assert!(*is_null_aware, "`?[` index should be null-aware")
            }
            other => panic!("expected Index, got {other:?}"),
        },
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn cascade_index_selector_spans_include_closing_delimiters() {
    for source in ["a..[0]", "a..?[0]", "a..items[ (0) ].field"] {
        let expr = parse_ok(source);
        let Expr::Cascade { sections, .. } = expr else {
            panic!("expected cascade");
        };
        let span = sections[0]
            .ops
            .iter()
            .find_map(|operation| match operation {
                CascadeOp::Index { span, .. } => Some(*span),
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
    let (_expr, errors) = parse_expr("const <int>");
    assert!(
        !errors.is_empty(),
        "expected a diagnostic for `const <int>` with no collection literal"
    );
}

#[test]
fn test_collection_if_case_guard_retained() {
    let expr = parse_ok("[if (x case int y when y > 0) y]");
    let elements = match expr {
        Expr::List { elements, .. } => elements,
        other => panic!("expected List, got {other:?}"),
    };
    match &elements[0] {
        CollectionElement::If { condition, .. } => match condition {
            IfCondition::Case(_, _, guard) => {
                assert!(guard.is_some(), "when-guard must be retained");
            }
            other => panic!("expected IfCondition::Case, got {other:?}"),
        },
        other => panic!("expected CollectionElement::If, got {other:?}"),
    }
}

#[test]
fn test_collection_if_case_without_guard() {
    let expr = parse_ok("[if (x case int y) y]");
    let elements = match expr {
        Expr::List { elements, .. } => elements,
        other => panic!("expected List, got {other:?}"),
    };
    match &elements[0] {
        CollectionElement::If {
            condition: IfCondition::Case(_, _, guard),
            ..
        } => {
            assert!(guard.is_none());
        }
        other => panic!("expected if-case element, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_identifier() {
    let expr = parse_ok("#foo");
    match expr {
        Expr::SymbolLit { raw, .. } => assert_eq!(raw, "#foo"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_dotted() {
    let expr = parse_ok("#bar.baz");
    match expr {
        Expr::SymbolLit { raw, .. } => assert_eq!(raw, "#bar.baz"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_operator() {
    let expr = parse_ok("#+");
    match expr {
        Expr::SymbolLit { raw, .. } => assert_eq!(raw, "#+"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_index_operator() {
    let expr = parse_ok("#[]");
    match expr {
        Expr::SymbolLit { raw, .. } => assert_eq!(raw, "#[]"),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn test_symbol_literal_index_assign_operator() {
    let expr = parse_ok("#[]=");
    match expr {
        Expr::SymbolLit { raw, .. } => assert_eq!(raw, "#[]="),
        other => panic!("expected SymbolLit, got {other:?}"),
    }
}

#[test]
fn cast_binds_tighter_than_less_than() {
    let (stmts, e) = parse_body("var r = a < b as int;", "async");
    assert_eq!(e, 0, "stmts: {stmts:?}");
    // Expect `a < (b as int)`.
    let init = match &stmts[0] {
        Stmt::LocalVar(v) => v.declarators[0].initializer.as_ref().unwrap(),
        other => panic!("expected LocalVar, got {other:?}"),
    };
    match init {
        Expr::Binary { op, right, .. } => {
            assert!(matches!(op, BinaryOp::Lt));
            assert!(
                matches!(right.as_ref(), Expr::As { .. }),
                "right: {right:?}"
            );
        }
        other => panic!("expected Binary(<), got {other:?}"),
    }
}

#[test]
fn cast_then_comparison() {
    // `(x as int) <= y`.
    let (_stmts, e) = parse_body("var r = x as int <= y;", "async");
    assert_eq!(e, 0);
}

#[test]
fn is_test_then_typed_collection_ternary() {
    let (_stmts, e) = parse_body("var y = a is Foo ? <int>[] : b;", "async");
    assert_eq!(e, 0);
}

#[test]
fn await_for_collection_element() {
    let (stmts, e) = parse_body("var b = [await for (var c in s) ...c];", "async");
    assert_eq!(e, 0, "stmts: {stmts:?}");
    let init = match &stmts[0] {
        Stmt::LocalVar(v) => v.declarators[0].initializer.as_ref().unwrap(),
        other => panic!("expected LocalVar, got {other:?}"),
    };
    match init {
        Expr::List { elements, .. } => match &elements[0] {
            CollectionElement::For { is_await, .. } => assert!(is_await, "expected await for"),
            other => panic!("expected For element, got {other:?}"),
        },
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn is_not_nullable_function_type() {
    let (_stmts, e) = parse_body("if (a is! Object? Function()) return;", "async");
    assert_eq!(e, 0);
}

#[test]
fn const_record_literal() {
    let (stmts, e) = parse_body("var x = const (\"\", Y.Z);", "async");
    assert_eq!(e, 0, "stmts: {stmts:?}");
    let init = match &stmts[0] {
        Stmt::LocalVar(v) => v.declarators[0].initializer.as_ref().unwrap(),
        other => panic!("expected LocalVar, got {other:?}"),
    };
    match init {
        Expr::Record {
            is_const, fields, ..
        } => {
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
    let (_stmts, e) = parse_body(
        "p.cast<Pointer<NativeFunction<Int32 Function(Pointer<Uint32> count)>>>();",
        "async",
    );
    assert_eq!(e, 0);
}

#[test]
fn typed_collections_do_not_enter_generic_function_lookahead() {
    let Expr::Call { args, .. } = parse_ok("f(<int>[1], () {})") else {
        panic!("expected call");
    };
    assert_eq!(args.positional.len(), 2);
    assert!(matches!(args.positional[0], Expr::List { .. }));
    assert!(matches!(args.positional[1], Expr::FuncExpr { .. }));
    assert!(matches!(parse_ok("<List<int>>[]"), Expr::List { .. }));
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
    let (_stmts, e) = parse_body("x.cast<A, B>();", "async");
    assert_eq!(e, 0);
}

#[test]
fn record_type_with_generic_field_as_type_arg() {
    let (_stmts, e) = parse_body(
        "var q = Queue<(Uri sourceDoc, Ref<Parseable> ref)>();",
        "async",
    );
    assert_eq!(e, 0);
}

#[test]
fn record_type_type_arg_with_nested_generic() {
    let (_stmts, e) = parse_body("isA<(String, List<String>)>();", "async");
    assert_eq!(e, 0);
}

#[test]
fn cascade_on_as_cast() {
    let (stmts, e) = parse_body("y = x as web.HTMLMetaElement ..name = 'n';", "async");
    assert_eq!(e, 0, "stmts: {stmts:?}");
    // The cascade wraps the whole `(x as T)`.
    let value = match &stmts[0] {
        Stmt::Expr(s) => match &s.expr {
            Expr::Assign { value, .. } => value.as_ref(),
            other => panic!("expected Assign, got {other:?}"),
        },
        other => panic!("expected Expr stmt, got {other:?}"),
    };
    match value {
        Expr::Cascade { object, .. } => {
            assert!(
                matches!(object.as_ref(), Expr::As { .. }),
                "object: {object:?}"
            );
        }
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn plain_cascade_still_parses() {
    let (_stmts, e) = parse_body("y..a()..b = 1;", "async");
    assert_eq!(e, 0);
}

#[test]
fn const_generic_constructor_statement() {
    let (stmts, e) = parse_body("const Optional<int>.absent();", "async");
    assert_eq!(e, 0, "stmts: {stmts:?}");
    let expr = match &stmts[0] {
        Stmt::Expr(s) => &s.expr,
        other => panic!("expected Expr stmt, got {other:?}"),
    };
    assert!(
        matches!(expr, Expr::New { is_const: true, .. }),
        "expr: {expr:?}"
    );
}

#[test]
fn const_generic_typed_local_decl_still_parses() {
    // Regression guard: `const List<int> xs = []` stays a const var declaration.
    let (stmts, e) = parse_body("const List<int> xs = [];", "async");
    assert_eq!(e, 0);
    assert!(matches!(&stmts[0], Stmt::LocalVar(v) if v.keyword == VarKeyword::Const));
}

#[test]
fn cascade_section_parses_full_selector_chain() {
    // A cascade section is a whole selector chain, not just its first selector:
    // `..b.c()` is one section holding a Field then a Call. The chain tail used to
    // be dropped, so this only "worked" at statement level by mis-splitting into a
    // separate dot-shorthand statement.
    let expr = parse_ok("a..b.c()");
    match expr {
        Expr::Cascade { sections, .. } => {
            assert_eq!(sections.len(), 1);
            assert!(
                matches!(
                    sections[0].ops.as_slice(),
                    [CascadeOp::Field { name: f, .. }, CascadeOp::Call(c, _, _)]
                        if f.name == "b" && c.name == "c"
                ),
                "ops: {:?}",
                sections[0].ops
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
    let expr = parse_ok("a..b?.c");
    match expr {
        Expr::Cascade { sections, .. } => assert!(
            matches!(
                sections[0].ops.as_slice(),
                [CascadeOp::Field { name: b, is_null_aware: false }, CascadeOp::Field { name: c, is_null_aware: true }]
                    if b.name == "b" && c.name == "c"
            ),
            "ops: {:?}",
            sections[0].ops
        ),
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
        let Expr::Cascade { sections, .. } = parse_ok(source) else {
            panic!("expected cascade");
        };
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].ops.len(), 3);
        assert!(matches!(sections[0].ops[1], CascadeOp::NullAssert { .. }));
        match source {
            "c..m()!.x" => {
                assert!(matches!(sections[0].ops[0], CascadeOp::Call(..)));
                assert!(matches!(sections[0].ops[2], CascadeOp::Field { .. }));
            }
            "c..[0]!.x" => {
                assert!(matches!(sections[0].ops[0], CascadeOp::Index { .. }));
                assert!(matches!(sections[0].ops[2], CascadeOp::Field { .. }));
            }
            "c..parent!.x = 1" => {
                assert!(matches!(sections[0].ops[2], CascadeOp::Assign(..)));
            }
            "c..parent![0] = 1" => {
                assert!(matches!(sections[0].ops[2], CascadeOp::IndexAssign(..)));
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
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = (() => b) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let class = only_class(&prog);
    let ctor = class
        .members
        .iter()
        .find_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .expect("constructor present");
    assert!(matches!(ctor.body, Some(FunctionBody::Block(_))));
    match &ctor.initializers[0] {
        ConstructorInitializer::FieldInit { value, .. } => {
            assert!(
                matches!(value.unparenthesized(), Expr::FuncExpr { .. }),
                "value: {value:?}"
            );
        }
        other => panic!("expected field initializer, got {other:?}"),
    }
}

#[test]
fn nested_paren_arithmetic_initializer_before_block_body() {
    // The same rollback must cover a plain parenthesized arithmetic expression.
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = (((o + 1) << 8) | 1) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn valid_param_list_initializer_before_block_body_is_not_a_lambda() {
    // `C(): g = (e) {}` — `(e)` is a valid formal-parameter list, so the
    // speculative lambda parse *succeeds*, yet the trailing `{}` is the
    // constructor body: the initializer value is the parenthesized expression
    // `(e)`, not a lambda `(e) {}`.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = (e) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let class = only_class(&prog);
    let ctor = class
        .members
        .iter()
        .find_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .expect("constructor present");
    assert!(matches!(ctor.body, Some(FunctionBody::Block(_))));
    match &ctor.initializers[0] {
        ConstructorInitializer::FieldInit { value, .. } => {
            assert!(
                !matches!(value, Expr::FuncExpr { .. }),
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
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = foo((e) {}) {} }");
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
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = c ? () {} : null; }");
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
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = f + () {}; }");
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
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("class C { var g; C(): g = () {}; }");
    assert!(
        !errors.is_empty(),
        "SDK treats `() {{}}` as record + ctor body, stranding `;`; \
         falcon must report the same, got no errors"
    );
}

#[test]
fn test_assignment_inside_parens() {
    // `(v = 0)` is an assignment expression, valid as a parenthesised subject.
    let expr = parse_ok("(v = 0) ?? 0");
    match expr {
        Expr::Binary {
            op: BinaryOp::NullCoalesce,
            left,
            ..
        } => assert!(
            matches!(left.unparenthesized(), Expr::Assign { .. }),
            "left of ?? should be an assignment, got {left:?}"
        ),
        other => panic!("expected `?? ` binary, got {other:?}"),
    }
}

#[test]
fn test_assignment_in_both_conditional_branches() {
    // `a ? b = c : d = e` — both ternary branches are assignments.
    let expr = parse_ok("a ? b = c : d = e");
    match expr {
        Expr::Conditional {
            then_expr,
            else_expr,
            ..
        } => {
            assert!(
                matches!(*then_expr, Expr::Assign { .. }),
                "then: {then_expr:?}"
            );
            assert!(
                matches!(*else_expr, Expr::Assign { .. }),
                "else: {else_expr:?}"
            );
        }
        other => panic!("expected conditional, got {other:?}"),
    }
}

/// Parse `void f(o) { <src> }` and return the first statement's expression.
fn first_stmt_expr(src: &str) -> (Expr, usize) {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(&format!("void f(o) {{ {src} }}"));
    let expr = common_expr::first_stmt_expr(&prog);
    (expr, errors.len())
}

#[test]
fn test_cascade_assign_rhs_excludes_following_cascade_section() {
    // `o..a = 1..b = 2` is TWO sections on `o` (o.a=1; o.b=2), not a single
    // section whose value is a nested cascade `1..b=2`. The assignment RHS is
    // parsed as `expressionWithoutCascade`, so the second `..` reattaches to `o`.
    let (expr, errors) = first_stmt_expr("o..a = 1..b = 2;");
    assert_eq!(errors, 0, "unexpected errors: {expr:?}");
    let (object, sections) = match &expr {
        Expr::Cascade {
            object, sections, ..
        } => (object, sections),
        other => panic!("expected cascade, got {other:?}"),
    };
    assert!(matches!(**object, Expr::Ident(ref id) if id.name == "o"));
    assert_eq!(sections.len(), 2, "expected two sections: {sections:?}");
    for section in sections {
        match &section.ops[0] {
            // Each section is a bare `field = intLit` assignment; the RHS must be
            // the int literal, never a nested Cascade.
            CascadeOp::Assign(_, _, value) => assert!(
                matches!(**value, Expr::IntLit { .. }),
                "RHS should be an int literal, got {value:?}"
            ),
            other => panic!("expected assign op, got {other:?}"),
        }
    }
}

#[test]
fn test_cascade_index_assign_rhs_excludes_following_section() {
    // Same rule for index-selector assignments: `o..[0] = 1..[1] = 2`.
    let (expr, errors) = first_stmt_expr("o..[0] = 1..[1] = 2;");
    assert_eq!(errors, 0, "unexpected errors: {expr:?}");
    match &expr {
        Expr::Cascade { sections, .. } => {
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
