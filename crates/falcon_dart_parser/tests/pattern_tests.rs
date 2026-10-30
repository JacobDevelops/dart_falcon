//! Minimal-repro tests for the Dart 3 pattern parsing gaps closed in the
//! `patterns` group: `when` guards after constant/dotted patterns, const
//! patterns beyond dotted idents, typed collection patterns, map-pattern rest,
//! and object-pattern getter shorthand binding. Also covers map-pattern
//! assignment (`{'k': a} = e;`) and the `LBrace` statement-start disambiguation
//! between blocks and map/set literal expression statements.
//!
//! Each test asserts ZERO parse errors and the exact CST shape.

use falcon_syntax::cst::*;

mod common;
use common::body_stmts;
use falcon_dart_parser::parser::parse;

/// Return the switch statement in the function body.
fn parse_switch(program: Program<'_>) -> SwitchStmt<'_> {
    body_stmts(program)
        .find_map(|s| match s {
            Stmt::Switch(sw) => Some(sw),
            _ => None,
        })
        .expect("expected a switch statement")
}

/// Pattern + guard of the first `case` label in the first case group.
fn first_case(sw: SwitchStmt<'_>) -> (Pattern<'_>, Option<Expr<'_>>) {
    match sw.cases().next().unwrap().cases().next().unwrap() {
        SwitchCaseKind::Pattern(label) => (label.pattern(), label.guard()),
        other => panic!("expected a pattern case, got {other:?}"),
    }
}

#[test]
fn dynamic_type_literal_is_a_constant_pattern() {
    let source = format!("void f() {{ switch (x) {{ {} }} }}", "case dynamic: break;");
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    assert!(
        matches!(
            pat,
            Pattern::Const(constant) if matches!(constant.value(), ConstPatternValue::Name(name) if name.len() == 1 && name.first().is_some_and(|identifier| identifier.name() == "dynamic" && identifier.span().end - identifier.span().start == "dynamic".len()))
        ),
        "expected a dynamic constant pattern, got {pat:?}"
    );
}

#[test]
fn dynamic_typed_patterns_keep_their_bindings() {
    for (source, wildcard) in [
        ("case dynamic value: break;", false),
        ("case dynamic _: break;", true),
    ] {
        let source = format!("void f() {{ switch (x) {{ {source} }} }}");
        let parsed = parse(&source);
        let sw = parse_switch(parsed.tree.program(&source));
        let errs = parsed.errors.len();
        assert_eq!(errs, 0, "switch: {sw:?}");
        let (pat, _) = first_case(sw);
        if wildcard {
            assert!(
                matches!(
                    pat,
                    Pattern::Wildcard(wildcard) if matches!(wildcard.type_(), Some(DartType::Dynamic(_)))
                ),
                "got {pat:?}"
            );
        } else {
            assert!(
                matches!(pat, Pattern::Variable(variable) if matches!(variable.type_(), Some(DartType::Dynamic(_))) && variable.name().name() == "value"),
                "got {pat:?}"
            );
        }
    }
}

#[test]
fn guard_after_dotted_constant_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case State.a when c: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, guard) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Name(name) = constant.value() else {
                panic!("expected Name constant value")
            };
            let segs: Vec<&str> = name.map(falcon_syntax::cst::Identifier::name).collect();
            assert_eq!(segs, ["State", "a"]);
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
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case foo when c: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (_pat, guard) = first_case(sw);
    assert!(guard.is_some());
}

#[test]
fn typed_variable_pattern_still_takes_its_name() {
    // Regression guard: a genuine typed variable pattern must keep binding its
    // name; only `when` is special.
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case int x when x > 0: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, guard) = first_case(sw);
    assert!(
        matches!(pat, Pattern::Variable(variable) if variable.name().name() == "x"),
        "got {pat:?}"
    );
    assert!(guard.is_some());
}

#[test]
fn const_constructor_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const C(1, 2): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Expr(expr) = constant.value() else {
                panic!("expected Expr constant value")
            };
            assert!(
                matches!(expr, Expr::New(creation) if creation.is_const()),
                "expected a const-constructor expr, got {expr:?}"
            );
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
}

#[test]
fn const_named_constructor_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const C.named(1): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    assert!(
        matches!(
            pat,
            Pattern::Const(constant) if matches!(constant.value(), ConstPatternValue::Expr(_))
        ),
        "got {pat:?}"
    );
}

#[test]
fn const_parenthesized_expression_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const (1 + 2): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Expr(expr) = constant.value() else {
                panic!("expected Expr constant value")
            };
            assert!(
                matches!(expr.unparenthesized(), Expr::Binary(_)),
                "expected a binary expr, got {expr:?}"
            );
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
}

#[test]
fn const_list_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const [1, 2]: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    assert!(
        matches!(
            pat,
            Pattern::Const(constant) if matches!(constant.value(), ConstPatternValue::Expr(_))
        ),
        "got {pat:?}"
    );
}

#[test]
fn const_set_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const {1}: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    assert!(
        matches!(
            pat,
            Pattern::Const(constant) if matches!(constant.value(), ConstPatternValue::Expr(_))
        ),
        "got {pat:?}"
    );
}

#[test]
fn dotted_constant_reference_stays_name_form() {
    // A bare dotted reference keeps the name form.
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case Color.red: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Name(name) = constant.value() else {
                panic!("expected Name constant value")
            };
            let segs: Vec<&str> = name.map(falcon_syntax::cst::Identifier::name).collect();
            assert_eq!(segs, ["Color", "red"]);
        }
        other => panic!("expected Const pattern, got {other:?}"),
    }
}

#[test]
fn typed_list_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case <int>[a, b]: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::List(lp) => {
            assert_eq!(lp.elements().len(), 2);
            match lp.type_arg() {
                Some(DartType::Named(nt)) => {
                    assert_eq!(nt.segments().next().unwrap().name(), "int");
                }
                other => panic!("expected `int` type arg, got {other:?}"),
            }
        }
        other => panic!("expected List pattern, got {other:?}"),
    }
}

#[test]
fn typed_map_pattern() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case <String, int>{'k': v}: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Map(mp) => {
            assert_eq!(mp.type_args().len(), 2);
            assert_eq!(mp.entries().len(), 1);
            assert!(!mp.has_rest());
        }
        other => panic!("expected Map pattern, got {other:?}"),
    }
}

#[test]
fn map_pattern_rest_sets_has_rest() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case {'a': 1, ...}: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Map(mp) => {
            assert_eq!(mp.entries().len(), 1);
            assert!(mp.has_rest(), "the `...` rest must be recorded");
        }
        other => panic!("expected Map pattern, got {other:?}"),
    }
}

#[test]
fn relational_lt_pattern_still_works() {
    // Regression guard: a real relational `< expr` pattern must not be mistaken
    // for a typed collection.
    let source = format!("void f() {{ switch (x) {{ {} }} }}", "case < 5: break;");
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    assert!(
        matches!(
            pat,
            Pattern::Relational(relation) if relation.op() == RelationalPatternOp::Lt
        ),
        "got {pat:?}"
    );
}

#[test]
fn object_pattern_getter_shorthand_binds_variable() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case Foo(:field): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Object(op) => {
            assert_eq!(op.fields().len(), 1);
            let f = op.fields().next().unwrap();
            assert_eq!(f.name().name(), "field", "getter name");
            match f.pattern() {
                Some(Pattern::Variable(variable)) => assert_eq!(variable.name().name(), "field"),
                other => panic!("expected a bound Variable, got {other:?}"),
            }
        }
        other => panic!("expected Object pattern, got {other:?}"),
    }
}

#[test]
fn object_pattern_multiple_shorthands() {
    // Pattern layer that the for-in case (item f) relies on.
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case MapEntry(:key, :value): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Object(op) => {
            let names: Vec<&str> = op.fields().map(|f| f.name().name()).collect();
            assert_eq!(names, ["key", "value"]);
            for f in op.fields() {
                assert!(
                    matches!(f.pattern(), Some(Pattern::Variable(variable)) if variable.name().name() == f.name().name()),
                    "field {} must bind a variable, got {:?}",
                    f.name().name(),
                    f.pattern()
                );
            }
        }
        other => panic!("expected Object pattern, got {other:?}"),
    }
}

#[test]
fn object_pattern_shorthand_infers_name_through_unary_patterns() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case Foo(:final a?, :b!, :var c as int): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Object(op) => {
            let names: Vec<&str> = op.fields().map(|f| f.name().name()).collect();
            assert_eq!(names, ["a", "b", "c"]);
            assert!(
                matches!(op.fields().next().unwrap().pattern(), Some(Pattern::NullCheck(check))
                    if matches!(check.inner(), Pattern::Variable(variable) if variable.name().name() == "a")),
                "got {:?}",
                op.fields().next().unwrap().pattern()
            );
        }
        other => panic!("expected Object pattern, got {other:?}"),
    }
}

//
// `case > 3 && < 5:` parses as LogicalAnd(Relational(> 3), Relational(< 5)): the
// operand is parsed at the bitwise-or tier so `&&`/`||` stay pattern-level.
#[test]
fn relational_operand_stops_before_logical_and() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case > 3 && < 5: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::LogicalAnd(logical) => {
            let left = logical.left();
            let right = logical.right();
            assert!(
                matches!(
                    left,
                    Pattern::Relational(relation) if relation.op() == RelationalPatternOp::Gt
                ),
                "left: {left:?}"
            );
            assert!(
                matches!(
                    right,
                    Pattern::Relational(relation) if relation.op() == RelationalPatternOp::Lt
                ),
                "right: {right:?}"
            );
        }
        other => panic!("expected LogicalAnd pattern, got {other:?}"),
    }
}

#[test]
fn object_pattern_for_in() {
    let source = format!(
        "void f() {{ {} }}",
        "for (final MapEntry(:key, :value) in m.entries) {}"
    );
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    let for_stmt = stmts
        .clone()
        .find_map(|s| match s {
            Stmt::For(f) => Some(f),
            _ => None,
        })
        .expect("expected a for statement");
    match for_stmt.header().init() {
        Some(ForInit::PatternForIn(init)) => match init.pattern() {
            Pattern::Object(op) => {
                let names: Vec<&str> = op.fields().map(|f| f.name().name()).collect();
                assert_eq!(names, ["key", "value"]);
            }
            other => panic!("expected Object pattern, got {other:?}"),
        },
        other => panic!("expected PatternForIn, got {other:?}"),
    }
}

#[test]
fn map_pattern_assignment_binds_variable() {
    let source = format!("void f() {{ {} }}", "{'a': a} = e;");
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    assert_eq!(stmts.len(), 1, "one statement expected: {stmts:?}");
    match stmts.clone().next().unwrap() {
        Stmt::PatternAssign(pa) => match pa.pattern() {
            Pattern::Map(mp) => {
                assert_eq!(mp.entries().len(), 1);
                assert!(!mp.has_rest());
                let entry = mp.entries().next().unwrap();
                // Key is the constant string literal `'a'`.
                assert!(
                    matches!(entry.key(), Expr::StringLit(_)),
                    "key: {:?}",
                    entry.key()
                );
                // Value sub-pattern binds a variable (assignable target `a`).
                assert!(
                    matches!(entry.pattern(), Pattern::Variable(variable) if variable.name().name() == "a"),
                    "entry pattern: {:?}",
                    entry.pattern()
                );
            }
            other => panic!("expected Map pattern, got {other:?}"),
        },
        other => panic!("expected PatternAssign, got {other:?}"),
    }
}

#[test]
fn map_pattern_assignment_multiple_entries() {
    let source = format!("void f() {{ {} }}", "{'a': a, 'b': b} = e;");
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    match stmts.clone().next().unwrap() {
        Stmt::PatternAssign(pa) => match pa.pattern() {
            Pattern::Map(mp) => {
                assert_eq!(mp.entries().len(), 2);
                let names: Vec<&str> = mp
                    .entries()
                    .map(|e| match e.pattern() {
                        Pattern::Variable(variable) => variable.name().name(),
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

#[test]
fn bare_block_still_a_block() {
    let source = format!("void f() {{ {} }}", "{ f(); }");
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    assert_eq!(stmts.len(), 1, "stmts: {stmts:?}");
    match stmts.clone().next().unwrap() {
        Stmt::Block(b) => assert_eq!(b.stmts().len(), 1, "block body: {:?}", b.stmts()),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn map_literal_expression_statement_unchanged() {
    let source = format!("void f() {{ {} }}", "{'k': 1};");
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    match stmts.clone().next().unwrap() {
        Stmt::Expr(es) => assert!(
            matches!(es.expr(), Expr::Map(_) | Expr::Set(_)),
            "expected a map/set literal expression, got {:?}",
            es.expr()
        ),
        other => panic!("expected Expr statement, got {other:?}"),
    }
}

#[test]
fn dot_shorthand_switch_pattern() {
    let source = format!(
        "void f() {{ {} }}",
        "var r = switch (x) { .build => 1, _ => 0 };"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn record_pattern_field_with_record_type() {
    let source = format!("void f() {{ {} }}", "final (bool a, (int, int)? b) = x;");
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn cast_pattern_in_switch_expression_arm() {
    let source = format!(
        "void f() {{ {} }}",
        "var r = switch (o) { value as int => 1, _ => 0 };"
    );
    let parsed = parse(&source);
    let (stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0, "stmts: {stmts:?}");
}

#[test]
fn cast_pattern_in_switch_case() {
    let source = format!(
        "void f() {{ {} }}",
        "switch (o) { case value as int: break; }"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn cast_pattern_on_parenthesized_pattern() {
    let source = format!(
        "void f() {{ {} }}",
        "switch (o) { case (B() || C()) as B: break; }"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn cast_pattern_in_map_pattern_field() {
    let source = format!(
        "void f() {{ {} }}",
        "switch (o) { case {usesKey: final usage as String}: break; }"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn record_pattern_with_when_guard() {
    // The `when` after a record pattern is a guard, not a typed-variable name.
    let source = format!(
        "void f() {{ {} }}",
        "switch (o) { case (a, b) when a: break; }"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn dot_shorthand_in_record_pattern_with_when() {
    let source = format!(
        "void f() {{ {} }}",
        "switch (o) { case (.linux, _) when h: break; }"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn const_record_pattern() {
    // `const (1, 2)` — a const *record*, not a single parenthesised expression.
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const (1, 2): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Expr(expr) = constant.value() else {
                panic!("expected Expr constant value")
            };
            match expr {
                Expr::Record(record) => {
                    let fields = record.fields();
                    let is_const = record.is_const();
                    assert!(is_const);
                    assert_eq!(fields.len(), 2);
                }
                other => panic!("expected const record expr, got {other:?}"),
            }
        }
        other => panic!("expected const pattern, got {other:?}"),
    }
}

#[test]
fn symbol_literal_constant_pattern() {
    // A symbol literal (`#add`) is a constant expression, so it is a valid
    // constant pattern in a `case` — the SDK accepts it. falcon must not reject
    // the `#` token here.
    let source = format!("void f() {{ switch (x) {{ {} }} }}", "case #add: break;");
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Expr(expr) = constant.value() else {
                panic!("expected Expr constant value")
            };
            assert!(
                matches!(expr, Expr::SymbolLit(symbol) if symbol.raw() == "#add"),
                "expected symbol-literal const pattern, got {expr:?}"
            );
        }
        other => panic!("expected const pattern, got {other:?}"),
    }
}

#[test]
fn const_parenthesised_expr_pattern_still_single() {
    // Control: `const (1 + 2)` remains a single parenthesised const expression.
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case const (1 + 2): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    let (pat, _) = first_case(sw);
    match pat {
        Pattern::Const(constant) => {
            let ConstPatternValue::Expr(expr) = constant.value() else {
                panic!("expected Expr constant value")
            };
            assert!(
                matches!(expr.unparenthesized(), Expr::Binary(_)),
                "expected single binary expr, got {expr:?}"
            );
        }
        other => panic!("expected const pattern, got {other:?}"),
    }
}

fn string_value(pattern: Pattern<'_>) -> std::borrow::Cow<'_, str> {
    let Pattern::Literal(literal) = pattern else {
        panic!("expected string literal pattern, got {pattern:?}");
    };
    let LiteralPatternValue::String(node) = literal.value() else {
        panic!("expected string literal value, got {literal:?}");
    };
    node.value()
}

/// The string value of the first case's literal string pattern.
fn first_case_string(sw: SwitchStmt<'_>) -> std::borrow::Cow<'_, str> {
    string_value(first_case(sw).0)
}

#[test]
fn adjacent_string_case_label_folds() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case 'foo' 'bar': break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    assert_eq!(first_case_string(sw), "foobar");
}

#[test]
fn adjacent_string_switch_expr_arm_folds() {
    let source = format!(
        "void f() {{ {} }}",
        "var r = switch (x) { 'a' 'b' => 1, _ => 0 };"
    );
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
}

#[test]
fn adjacent_string_in_list_pattern_folds() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case ['a' 'b', _]: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    match first_case(sw).0 {
        Pattern::List(lp) => match lp.elements().next().unwrap() {
            ListPatternElement::Pattern(pattern) => assert_eq!(string_value(pattern), "ab"),
            other @ ListPatternElement::Rest(_) => panic!("expected string element, got {other:?}"),
        },
        other => panic!("expected list pattern, got {other:?}"),
    }
}

#[test]
fn adjacent_string_in_record_field_folds() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case (k: 'a' 'b'): break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    match first_case(sw).0 {
        Pattern::Record(rp) => match rp.fields().next().unwrap().pattern() {
            pattern @ Pattern::Literal(_) => assert_eq!(string_value(pattern), "ab"),
            other => panic!("expected string field, got {other:?}"),
        },
        other => panic!("expected record pattern, got {other:?}"),
    }
}

#[test]
fn adjacent_string_if_case_folds() {
    let source = format!("void f() {{ {} }}", "if (x case 'a' 'b') print('yes');");
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
}

#[test]
fn triple_adjacent_string_folds() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case 'foo' 'bar' 'baz': break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    assert_eq!(first_case_string(sw), "foobarbaz");
}

#[test]
fn single_string_pattern_unaffected() {
    // Regression guard: a lone string literal still parses as one pattern.
    let source = format!("void f() {{ switch (x) {{ {} }} }}", "case 'foo': break;");
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0, "switch: {sw:?}");
    assert_eq!(first_case_string(sw), "foo");
}

#[test]
fn switch_expression_member_access_constant_with_guard() {
    let source = format!(
        "void f() {{ {} }}",
        "final v = switch (s) { \"Lax\".length when ok(s) => 1, 'x'.length => 2, _ => 3 };"
    );
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "{stmts:?}");
    assert!(matches!(stmts.clone().next().unwrap(), Stmt::LocalVar(_)));
}

#[test]
fn switch_case_member_access_constant_with_guard() {
    let source = format!(
        "void f() {{ switch (x) {{ {} }} }}",
        "case 'Lax'.length when c: break;"
    );
    let parsed = parse(&source);
    let sw = parse_switch(parsed.tree.program(&source));
    let errs = parsed.errors.len();
    assert_eq!(errs, 0);
    let (_, guard) = first_case(sw);
    assert!(guard.is_some());
}

#[test]
fn object_pattern_declaration_binds_getter_shorthand() {
    let source = format!(
        "void f() {{ {} }}",
        "final AsyncError(:error) = result; use(error);"
    );
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    assert_eq!(stmts.len(), 2, "must preserve the following statement");
    let Stmt::PatternDecl(decl) = stmts.clone().next().unwrap() else {
        panic!(
            "expected pattern declaration, got {:?}",
            stmts.clone().next().unwrap()
        );
    };
    assert_eq!(decl.keyword(), VarKeyword::Final);
    let Pattern::Object(object) = decl.pattern() else {
        panic!("expected object pattern, got {:?}", decl.pattern());
    };
    let DartType::Named(ty) = object.type_() else {
        panic!("expected named type, got {:?}", object.type_());
    };
    assert_eq!(ty.segments().next().unwrap().name(), "AsyncError");
    assert_eq!(object.fields().len(), 1);
    assert_eq!(object.fields().next().unwrap().name().name(), "error");
    assert!(
        matches!(object.fields().next().unwrap().pattern(), Some(Pattern::Variable(variable)) if variable.name().name() == "error")
    );
    assert!(matches!(decl.init(), Expr::Ident(name) if name.name() == "result"));
    assert!(matches!(stmts.clone().nth(1).unwrap(), Stmt::Expr(_)));
}

#[test]
fn var_qualified_generic_object_pattern_declaration() {
    let source = format!(
        "void f() {{ {} }}",
        "var prefix.Box<int>(:value) = result; use(value);"
    );
    let parsed = parse(&source);
    let (stmts, errs) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errs, 0, "stmts: {stmts:?}");
    assert_eq!(stmts.len(), 2, "must preserve the following statement");
    let Stmt::PatternDecl(decl) = stmts.clone().next().unwrap() else {
        panic!(
            "expected pattern declaration, got {:?}",
            stmts.clone().next().unwrap()
        );
    };
    assert_eq!(decl.keyword(), VarKeyword::Var);
    let Pattern::Object(object) = decl.pattern() else {
        panic!("expected object pattern, got {:?}", decl.pattern());
    };
    let DartType::Named(ty) = object.type_() else {
        panic!("expected named type, got {:?}", object.type_());
    };
    assert_eq!(
        ty.segments()
            .map(falcon_syntax::cst::Identifier::name)
            .collect::<Vec<_>>(),
        ["prefix", "Box"]
    );
    assert_eq!(ty.type_args().len(), 1);
    assert!(
        matches!(ty.type_args().next().unwrap(), DartType::Named(arg) if arg.segments().next().unwrap().name() == "int")
    );
    assert_eq!(object.fields().len(), 1);
    assert_eq!(object.fields().next().unwrap().name().name(), "value");
    assert!(
        matches!(object.fields().next().unwrap().pattern(), Some(Pattern::Variable(variable)) if variable.name().name() == "value")
    );
    assert!(matches!(decl.init(), Expr::Ident(name) if name.name() == "result"));
    assert!(matches!(stmts.clone().nth(1).unwrap(), Stmt::Expr(_)));
}
