//! Regression tests for expression-level parser gaps (group `exprs`): each
//! construct must parse with zero errors (unless the gap is specifically about a
//! *missing* diagnostic) and produce a faithful AST shape.

use falcon_dart_parser::parser::{ParseError, parse};
use falcon_syntax::ast::*;

/// Parse `var x = <expr>;` and return the initializer expression plus any parse
/// errors. Wrapping in a top-level variable is the simplest way to drive the
/// expression parser through the public `parse` entry point.
fn parse_expr(expr_src: &str) -> (Expr, Vec<ParseError>) {
    let src = format!("var x = {expr_src};");
    let (prog, errors) = parse(&src);
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

// ── Item (a): leading-spread map/set disambiguation ───────────────────────────

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

// ── Item (b): `<T>{}` empty-brace arity ───────────────────────────────────────

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
        Expr::Map {
            entries, elements, ..
        } => {
            assert!(entries.is_empty() && elements.is_empty());
        }
        other => panic!("expected empty Map, got {other:?}"),
    }
}

#[test]
fn test_no_type_args_empty_braces_is_map() {
    let expr = parse_ok("{}");
    assert!(matches!(expr, Expr::Map { .. }), "got {expr:?}");
}

// ── Item (c): generic-instantiation tear-off ──────────────────────────────────

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

// ── Item (d): cascade null-awareness ──────────────────────────────────────────

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
            match &sections[0].op {
                CascadeOp::Field(_, na) => assert!(*na, "first section should be null-aware"),
                other => panic!("expected Field, got {other:?}"),
            }
            match &sections[1].op {
                CascadeOp::Field(_, na) => assert!(!*na, "second section is plain `..`"),
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
        Expr::Cascade { sections, .. } => match &sections[0].op {
            CascadeOp::Index(_, na) => assert!(*na, "index section should be null-aware"),
            other => panic!("expected Index, got {other:?}"),
        },
        other => panic!("expected Cascade, got {other:?}"),
    }
}

#[test]
fn test_plain_cascade_index_section_not_null_aware() {
    let expr = parse_ok("a..[0]");
    match expr {
        Expr::Cascade { sections, .. } => match &sections[0].op {
            CascadeOp::Index(_, na) => assert!(!*na),
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
        Expr::Cascade { sections, .. } => match &sections[0].op {
            CascadeOp::Index(_, na) => assert!(*na, "`?[` index should be null-aware"),
            other => panic!("expected Index, got {other:?}"),
        },
        other => panic!("expected Cascade, got {other:?}"),
    }
}

// ── Item (e): `const <T>` without `[`/`{` records a diagnostic ────────────────

#[test]
fn test_const_type_args_without_collection_records_error() {
    let (_expr, errors) = parse_expr("const <int>");
    assert!(
        !errors.is_empty(),
        "expected a diagnostic for `const <int>` with no collection literal"
    );
}

// ── Item (f): collection if-case guard retained ───────────────────────────────

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

// ── Item (g): symbol literals ─────────────────────────────────────────────────

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
