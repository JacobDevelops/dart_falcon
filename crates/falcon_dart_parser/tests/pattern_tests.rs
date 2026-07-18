//! Minimal-repro tests for the Dart 3 pattern parsing gaps closed in the
//! `patterns` group: `when` guards after constant/dotted patterns, const
//! patterns beyond dotted idents, typed collection patterns, map-pattern rest,
//! and object-pattern getter shorthand binding. Also covers map-pattern
//! assignment (`{'k': a} = e;`) and the LBrace statement-start disambiguation
//! between blocks and map/set literal expression statements.
//!
//! Each test asserts ZERO parse errors and the exact AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

/// Parse `src` as the body of a function and return `(statements, error_count)`.
fn parse_body(src: &str) -> (Vec<Stmt>, usize) {
    let wrapped = format!("void f() {{ {src} }}");
    let (prog, errors) = parse(&wrapped);
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}\nerrors: {errors:?}"),
    };
    let block = match func.body.as_ref().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("expected block, got {other:?}"),
    };
    (block.stmts.clone(), errors.len())
}

/// Parse a single `switch (x) { <cases> }` statement and return it with the
/// error count.
fn parse_switch(cases: &str) -> (SwitchStmt, usize) {
    let (stmts, errs) = parse_body(&format!("switch (x) {{ {cases} }}"));
    let sw = stmts
        .into_iter()
        .find_map(|s| match s {
            Stmt::Switch(sw) => Some(sw),
            _ => None,
        })
        .expect("expected a switch statement");
    (sw, errs)
}

/// Pattern + guard of the first `case` label in the first case group.
fn first_case(sw: &SwitchStmt) -> (&Pattern, &Option<Expr>) {
    match &sw.cases[0].cases[0] {
        SwitchCaseKind::Pattern(p, g) => (p, g),
        other => panic!("expected a pattern case, got {other:?}"),
    }
}

// ── (a) `when` guard after constant / dotted patterns ─────────────────────────

#[test]
fn guard_after_dotted_constant_pattern() {
    let (sw, errs) = parse_switch("case State.a when c: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, guard) = first_case(&sw);
    match pat {
        Pattern::Const(cp) => {
            let segs: Vec<&str> = cp.name.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(segs, ["State", "a"]);
            assert!(cp.expr.is_none(), "dotted reference must not carry an expr");
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
    assert!(
        guard.is_some(),
        "the `when` guard must be parsed, not consumed"
    );
}

#[test]
fn guard_after_bare_constant_reference() {
    // A lowercase bare identifier in a case is a constant reference; the `when`
    // guard must still be recognised.
    let (sw, errs) = parse_switch("case foo when c: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (_pat, guard) = first_case(&sw);
    assert!(guard.is_some());
}

#[test]
fn typed_variable_pattern_still_takes_its_name() {
    // Regression guard: a genuine typed variable pattern must keep binding its
    // name; only `when` is special.
    let (sw, errs) = parse_switch("case int x when x > 0: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, guard) = first_case(&sw);
    assert!(
        matches!(pat, Pattern::Variable { name, .. } if name.name == "x"),
        "got {pat:?}"
    );
    assert!(guard.is_some());
}

// ── (b) const patterns beyond dotted idents ───────────────────────────────────

#[test]
fn const_constructor_pattern() {
    let (sw, errs) = parse_switch("case const C(1, 2): break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Const(cp) => {
            assert!(cp.name.is_empty());
            assert!(
                matches!(cp.expr.as_deref(), Some(Expr::New { is_const: true, .. })),
                "expected a const-constructor expr, got {:?}",
                cp.expr
            );
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
}

#[test]
fn const_named_constructor_pattern() {
    let (sw, errs) = parse_switch("case const C.named(1): break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    assert!(
        matches!(pat, Pattern::Const(cp) if cp.name.is_empty() && cp.expr.is_some()),
        "got {pat:?}"
    );
}

#[test]
fn const_parenthesized_expression_pattern() {
    let (sw, errs) = parse_switch("case const (1 + 2): break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Const(cp) => {
            assert!(cp.name.is_empty());
            assert!(
                matches!(cp.expr.as_deref(), Some(Expr::Binary { .. })),
                "expected a binary expr, got {:?}",
                cp.expr
            );
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
}

#[test]
fn const_list_pattern() {
    let (sw, errs) = parse_switch("case const [1, 2]: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    assert!(
        matches!(pat, Pattern::Const(cp) if cp.name.is_empty() && cp.expr.is_some()),
        "got {pat:?}"
    );
}

#[test]
fn const_set_pattern() {
    let (sw, errs) = parse_switch("case const {1}: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    assert!(
        matches!(pat, Pattern::Const(cp) if cp.name.is_empty() && cp.expr.is_some()),
        "got {pat:?}"
    );
}

#[test]
fn dotted_constant_reference_stays_name_form() {
    // Regression guard for (b): a bare dotted reference keeps `expr: None`.
    let (sw, errs) = parse_switch("case Color.red: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Const(cp) => {
            let segs: Vec<&str> = cp.name.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(segs, ["Color", "red"]);
            assert!(cp.expr.is_none());
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
}

// ── (c) typed collection patterns + map rest ──────────────────────────────────

#[test]
fn typed_list_pattern() {
    let (sw, errs) = parse_switch("case <int>[a, b]: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::List(lp) => {
            assert_eq!(lp.elements.len(), 2);
            match &lp.type_arg {
                Some(DartType::Named(nt)) => assert_eq!(nt.segments[0].name, "int"),
                other => panic!("expected `int` type arg, got {other:?}"),
            }
        }
        other => panic!("expected List pattern, got {other:?}"),
    }
}

#[test]
fn typed_map_pattern() {
    let (sw, errs) = parse_switch("case <String, int>{'k': v}: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Map(mp) => {
            assert_eq!(mp.type_args.len(), 2);
            assert_eq!(mp.entries.len(), 1);
            assert!(!mp.has_rest);
        }
        other => panic!("expected Map pattern, got {other:?}"),
    }
}

#[test]
fn map_pattern_rest_sets_has_rest() {
    let (sw, errs) = parse_switch("case {'a': 1, ...}: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Map(mp) => {
            assert_eq!(mp.entries.len(), 1);
            assert!(mp.has_rest, "the `...` rest must be recorded");
        }
        other => panic!("expected Map pattern, got {other:?}"),
    }
}

#[test]
fn relational_lt_pattern_still_works() {
    // Regression guard: a real relational `< expr` pattern must not be mistaken
    // for a typed collection.
    let (sw, errs) = parse_switch("case < 5: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    assert!(
        matches!(
            pat,
            Pattern::Relational {
                op: RelationalPatternOp::Lt,
                ..
            }
        ),
        "got {pat:?}"
    );
}

// ── (d) object-pattern getter shorthand binding ───────────────────────────────

#[test]
fn object_pattern_getter_shorthand_binds_variable() {
    let (sw, errs) = parse_switch("case Foo(:field): break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Object(op) => {
            assert_eq!(op.fields.len(), 1);
            let f = &op.fields[0];
            assert_eq!(f.name.name, "field", "getter name");
            match &f.pattern {
                Some(Pattern::Variable { name, .. }) => assert_eq!(name.name, "field"),
                other => panic!("expected a bound Variable, got {other:?}"),
            }
        }
        other => panic!("expected Object pattern, got {other:?}"),
    }
}

#[test]
fn object_pattern_multiple_shorthands() {
    // Pattern layer that the for-in case (item f) relies on.
    let (sw, errs) = parse_switch("case MapEntry(:key, :value): break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::Object(op) => {
            let names: Vec<&str> = op.fields.iter().map(|f| f.name.name.as_str()).collect();
            assert_eq!(names, ["key", "value"]);
            for f in &op.fields {
                assert!(
                    matches!(&f.pattern, Some(Pattern::Variable { name, .. }) if name.name == f.name.name),
                    "field {} must bind a variable, got {:?}",
                    f.name.name,
                    f.pattern
                );
            }
        }
        other => panic!("expected Object pattern, got {other:?}"),
    }
}

// ── (e) relational operand over-consume ───────────────────────────────────────
//
// `case > 3 && < 5:` parses as LogicalAnd(Relational(> 3), Relational(< 5)): the
// operand is parsed at the bitwise-or tier so `&&`/`||` stay pattern-level.
#[test]
fn relational_operand_stops_before_logical_and() {
    let (sw, errs) = parse_switch("case > 3 && < 5: break;");
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(&sw);
    match pat {
        Pattern::LogicalAnd { left, right, .. } => {
            assert!(
                matches!(
                    left.as_ref(),
                    Pattern::Relational {
                        op: RelationalPatternOp::Gt,
                        ..
                    }
                ),
                "left: {left:?}"
            );
            assert!(
                matches!(
                    right.as_ref(),
                    Pattern::Relational {
                        op: RelationalPatternOp::Lt,
                        ..
                    }
                ),
                "right: {right:?}"
            );
        }
        other => panic!("expected LogicalAnd pattern, got {other:?}"),
    }
}

// ── (f) object pattern in for-in — BLOCKED on a cross-file change ──────────────
//
// `for (final MapEntry(:key, :value) in m.entries) {}`. The pattern layer (the
// object pattern with `:key`/`:value` bindings) is fixed by item (d) and is
// exercised by `object_pattern_multiple_shorthands` above. What remains is the
// pattern-for-in detector in stmt.rs `parse_for_clauses`, which only fires when
// `final`/`var` is directly followed by `(`/`[`/`{` and so never recognises an
// object pattern that starts with a type name. That gate lives in a file this
// group does not own. Un-ignore once the stmt.rs detector is broadened.
#[test]
fn object_pattern_for_in() {
    let (stmts, errs) = parse_body("for (final MapEntry(:key, :value) in m.entries) {}");
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    let for_stmt = stmts
        .iter()
        .find_map(|s| match s {
            Stmt::For(f) => Some(f),
            _ => None,
        })
        .expect("expected a for statement");
    match for_stmt.init.as_ref() {
        Some(ForInit::PatternForIn { pattern, .. }) => match pattern.as_ref() {
            Pattern::Object(op) => {
                let names: Vec<&str> = op.fields.iter().map(|f| f.name.name.as_str()).collect();
                assert_eq!(names, ["key", "value"]);
            }
            other => panic!("expected Object pattern, got {other:?}"),
        },
        other => panic!("expected PatternForIn, got {other:?}"),
    }
}
// ── map-pattern assignment ────────────────────────────────────────────────────

#[test]
fn map_pattern_assignment_binds_variable() {
    let (stmts, errs) = parse_body("{'a': a} = e;");
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    assert_eq!(stmts.len(), 1, "one statement expected: {stmts:?}");
    match &stmts[0] {
        Stmt::PatternAssign(pa) => match &pa.pattern {
            Pattern::Map(mp) => {
                assert_eq!(mp.entries.len(), 1);
                assert!(!mp.has_rest);
                let entry = &mp.entries[0];
                // Key is the constant string literal `'a'`.
                assert!(
                    matches!(&entry.key, Expr::StringLit(_)),
                    "key: {:?}",
                    entry.key
                );
                // Value sub-pattern binds a variable (assignable target `a`).
                assert!(
                    matches!(&entry.pattern, Pattern::Variable { name, .. } if name.name == "a"),
                    "entry pattern: {:?}",
                    entry.pattern
                );
            }
            other => panic!("expected Map pattern, got {other:?}"),
        },
        other => panic!("expected PatternAssign, got {other:?}"),
    }
}

#[test]
fn map_pattern_assignment_multiple_entries() {
    let (stmts, errs) = parse_body("{'a': a, 'b': b} = e;");
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    match &stmts[0] {
        Stmt::PatternAssign(pa) => match &pa.pattern {
            Pattern::Map(mp) => {
                assert_eq!(mp.entries.len(), 2);
                let names: Vec<&str> = mp
                    .entries
                    .iter()
                    .map(|e| match &e.pattern {
                        Pattern::Variable { name, .. } => name.name.as_str(),
                        other => panic!("expected Variable, got {other:?}"),
                    })
                    .collect();
                assert_eq!(names, ["a", "b"]);
            }
            other => panic!("expected Map pattern, got {other:?}"),
        },
        other => panic!("expected PatternAssign, got {other:?}"),
    }
}

// ── regression guards: LBrace disambiguation is untouched ──────────────────────

#[test]
fn bare_block_still_a_block() {
    let (stmts, errs) = parse_body("{ f(); }");
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    assert_eq!(stmts.len(), 1, "stmts: {stmts:?}");
    match &stmts[0] {
        Stmt::Block(b) => assert_eq!(b.stmts.len(), 1, "block body: {:?}", b.stmts),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn map_literal_expression_statement_unchanged() {
    let (stmts, errs) = parse_body("{'k': 1};");
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    match &stmts[0] {
        Stmt::Expr(es) => assert!(
            matches!(&es.expr, Expr::Map { .. } | Expr::Set { .. }),
            "expected a map/set literal expression, got {:?}",
            es.expr
        ),
        other => panic!("expected Expr statement, got {other:?}"),
    }
}

// ── Corpus-found pattern gaps ───────────────────────────────────────────

#[test]
fn dot_shorthand_switch_pattern() {
    let (_stmts, e) = parse_body("var r = switch (x) { .build => 1, _ => 0 };");
    assert_eq!(e, 0);
}

#[test]
fn record_pattern_field_with_record_type() {
    let (_stmts, e) = parse_body("final (bool a, (int, int)? b) = x;");
    assert_eq!(e, 0);
}

#[test]
fn cast_pattern_in_switch_expression_arm() {
    let (stmts, e) = parse_body("var r = switch (o) { value as int => 1, _ => 0 };");
    assert_eq!(e, 0, "stmts: {stmts:?}");
}

#[test]
fn cast_pattern_in_switch_case() {
    let (_stmts, e) = parse_body("switch (o) { case value as int: break; }");
    assert_eq!(e, 0);
}

#[test]
fn cast_pattern_on_parenthesized_pattern() {
    let (_stmts, e) = parse_body("switch (o) { case (B() || C()) as B: break; }");
    assert_eq!(e, 0);
}

#[test]
fn cast_pattern_in_map_pattern_field() {
    let (_stmts, e) = parse_body("switch (o) { case {usesKey: final usage as String}: break; }");
    assert_eq!(e, 0);
}

#[test]
fn record_pattern_with_when_guard() {
    // The `when` after a record pattern is a guard, not a typed-variable name.
    let (_stmts, e) = parse_body("switch (o) { case (a, b) when a: break; }");
    assert_eq!(e, 0);
}

#[test]
fn dot_shorthand_in_record_pattern_with_when() {
    let (_stmts, e) = parse_body("switch (o) { case (.linux, _) when h: break; }");
    assert_eq!(e, 0);
}
