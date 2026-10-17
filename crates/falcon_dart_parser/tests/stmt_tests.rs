//! Regression tests for statement-level parser gaps: each construct must parse
//! with zero errors and produce a faithful CST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;

mod common;
use common::body_stmts;

/// Return the single statement, asserting exactly one statement.
fn only_stmt(program: Program<'_>) -> Stmt<'_> {
    let stmts = body_stmts(program);
    assert_eq!(stmts.len(), 1, "expected one statement: {stmts:?}");
    stmts.into_iter().next().unwrap()
}

#[test]
fn test_empty_and_rethrow_have_no_expression_children() {
    use falcon_syntax::cst::visit::{Visitor, walk_stmt};

    let source = "; null; try {} catch (e) { rethrow; throw e; }";
    let source = format!("void f() async {{ {} }}", source);
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0);
    let Stmt::Empty(span) = stmts.clone().next().unwrap() else {
        panic!(
            "expected empty statement, got {:?}",
            stmts.clone().next().unwrap()
        );
    };
    assert_eq!(span.span().end - span.span().start, 1);
    assert!(matches!(
        stmts.clone().nth(1).unwrap(),
        Stmt::Expr(statement) if matches!(statement.expr(), Expr::NullLit(_))
    ));
    let Stmt::TryCatch(statement) = stmts.clone().nth(2).unwrap() else {
        panic!(
            "expected try statement, got {:?}",
            stmts.clone().nth(2).unwrap()
        );
    };
    let catch = statement.catches().next().unwrap().body();
    let Stmt::Rethrow(span) = catch.stmts().next().unwrap() else {
        panic!(
            "expected rethrow statement, got {:?}",
            catch.stmts().next().unwrap()
        );
    };
    assert_eq!(span.span().end - span.span().start, "rethrow;".len());
    assert!(matches!(
        catch.stmts().nth(1).unwrap(),
        Stmt::Throw(statement) if matches!(statement.value(), Expr::Ident(identifier) if identifier.name() == "e")
    ));

    #[derive(Default)]
    struct Expressions(usize);

    impl<'a> Visitor<'a> for Expressions {
        fn visit_expr(&mut self, _: Expr<'a>) {
            self.0 += 1;
        }
    }

    let mut expressions = Expressions::default();
    for statement in stmts.clone() {
        walk_stmt(&mut expressions, statement);
    }
    assert_eq!(expressions.0, 2);
}

#[test]
fn test_labeled_for_loop() {
    let source = format!(
        "void f() async {{ {} }}",
        "outer: for (;;) { break outer; }"
    );
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    let labeled = match stmt {
        Stmt::Labeled(l) => l,
        other => panic!("expected labeled statement, got {other:?}"),
    };
    assert_eq!(labeled.label().name(), "outer");
    assert!(
        matches!(labeled.stmt(), Stmt::For(_)),
        "inner: {:?}",
        labeled.stmt()
    );
}

#[test]
fn test_labeled_while_loop() {
    let source = format!(
        "void f() async {{ {} }}",
        "loop: while (true) { continue loop; }"
    );
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Labeled(l) => {
            assert_eq!(l.label().name(), "loop");
            assert!(matches!(l.stmt(), Stmt::While(_)));
        }
        other => panic!("expected labeled statement, got {other:?}"),
    }
}

#[test]
fn test_nested_labels() {
    // Two labels stacking, each wrapping the next.
    let source = format!("void f() async {{ {} }}", "a: b: for (;;) {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Labeled(outer) => {
            assert_eq!(outer.label().name(), "a");
            match outer.stmt() {
                Stmt::Labeled(inner) => {
                    assert_eq!(inner.label().name(), "b");
                    assert!(matches!(inner.stmt(), Stmt::For(_)));
                }
                other => panic!("expected inner label, got {other:?}"),
            }
        }
        other => panic!("expected labeled statement, got {other:?}"),
    }
}

#[test]
fn test_label_does_not_break_ternary() {
    // `x ? a : b;` must stay a conditional expression, never a label.
    let source = format!("void f() async {{ {} }}", "x ? a : b;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(
            matches!(e.expr(), Expr::Conditional(_)),
            "got {:?}",
            e.expr()
        ),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_label_does_not_break_map_literal_in_decl() {
    // A map literal in a var initializer keeps its `key: value` colons.
    let source = format!("void f() async {{ {} }}", "var m = {'a': 1, 'b': 2};");
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0);
    match stmts.clone().next().unwrap() {
        Stmt::LocalVar(v) => {
            let init = v.declarators().next().unwrap().initializer().unwrap();
            assert!(matches!(init, Expr::Map(_)), "got {init:?}");
        }
        other => panic!("expected local var, got {other:?}"),
    }
}

#[test]
fn test_await_for_in() {
    let source = format!("void f() async {{ {} }}", "await for (var e in s) {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::For(f) => {
            assert!(f.is_await(), "expected await-for");
            match f.header().init() {
                Some(ForInit::ForIn(init)) => assert_eq!(init.name().name(), "e"),
                other => panic!("expected for-in, got {other:?}"),
            }
        }
        other => panic!("expected for statement, got {other:?}"),
    }
}

#[test]
fn test_plain_for_is_not_await() {
    let source = format!("void f() async {{ {} }}", "for (var e in s) {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::For(f) => assert!(!f.is_await()),
        other => panic!("expected for statement, got {other:?}"),
    }
}

#[test]
fn test_labeled_await_for() {
    // `await for` still works when labeled.
    let source = format!(
        "void f() async {{ {} }}",
        "outer: await for (final e in s) { break outer; }"
    );
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Labeled(l) => match l.stmt() {
            Stmt::For(f) => assert!(f.is_await()),
            other => panic!("expected for, got {other:?}"),
        },
        other => panic!("expected labeled, got {other:?}"),
    }
}

#[test]
fn test_untyped_local_func_block_and_arrow() {
    let source = format!("void f() async {{ {} }}", "foo() {} bar() => 1;");
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0);
    assert_eq!(stmts.len(), 2, "stmts: {stmts:?}");
    match stmts.clone().next().unwrap() {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name().name(), "foo");
            assert!(f.return_type().is_none());
            assert!(matches!(f.body(), FunctionBody::Block(_)));
        }
        other => panic!("expected local func, got {other:?}"),
    }
    match stmts.clone().nth(1).unwrap() {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name().name(), "bar");
            assert!(f.return_type().is_none());
            assert!(matches!(f.body(), FunctionBody::Arrow(..)));
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_untyped_local_func_with_params() {
    let source = format!(
        "void f() async {{ {} }}",
        "greet(String name) => print(name);"
    );
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name().name(), "greet");
            assert!(f.return_type().is_none());
            assert_eq!(f.params().params().len(), 1);
            assert_eq!(
                f.params().params().next().unwrap().kind(),
                ParamKind::Positional
            );
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_untyped_generic_local_func() {
    let source = format!("void f() async {{ {} }}", "ident<T>(T x) => x;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name().name(), "ident");
            assert_eq!(f.type_params().len(), 1);
            assert!(f.return_type().is_none());
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_untyped_async_local_func() {
    let source = format!("void f() async {{ {} }}", "load() async {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.async_marker(), AsyncMarker::Async);
            assert!(f.return_type().is_none());
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_plain_call_stays_call() {
    // A bare call statement must NOT become a local function.
    let source = format!("void f() async {{ {} }}", "foo();");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Call(_)), "got {:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_generic_call_stays_call() {
    // `foo<int>();` is a generic invocation, not a local function.
    let source = format!("void f() async {{ {} }}", "foo<int>();");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Call(_)), "got {:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_call_with_args_stays_call() {
    let source = format!("void f() async {{ {} }}", "foo(a, b);");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Call(_)), "got {:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_record_pattern_assignment() {
    let source = format!("void f() async {{ {} }}", "(a, b) = e;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::PatternAssign(p) => {
            assert!(
                matches!(p.pattern(), Pattern::Record(_)),
                "got {:?}",
                p.pattern()
            );
            assert!(matches!(p.value(), Expr::Ident(_)));
        }
        other => panic!("expected pattern assignment, got {other:?}"),
    }
}

#[test]
fn test_list_pattern_assignment() {
    let source = format!("void f() async {{ {} }}", "[a, b] = e;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::PatternAssign(p) => {
            assert!(
                matches!(p.pattern(), Pattern::List(_)),
                "got {:?}",
                p.pattern()
            );
        }
        other => panic!("expected pattern assignment, got {other:?}"),
    }
}

#[test]
fn test_record_pattern_assign_targets_are_variables() {
    // Assignment targets bind variables, matching `var (a, b) = e;`.
    let source = format!("void f() async {{ {} }}", "(a, b) = e;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::PatternAssign(p) => match p.pattern() {
            Pattern::Record(rec) => {
                let names: Vec<&str> = rec
                    .fields()
                    .map(|f| match f.pattern() {
                        Pattern::Variable(variable) => variable.name().name(),
                        other => panic!("expected variable target, got {other:?}"),
                    })
                    .collect();
                assert_eq!(names, ["a", "b"]);
            }
            other => panic!("expected record pattern, got {other:?}"),
        },
        other => panic!("expected pattern assignment, got {other:?}"),
    }
}

#[test]
fn test_list_pattern_assign_targets_are_variables() {
    let source = format!("void f() async {{ {} }}", "[a, b] = e;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::PatternAssign(p) => match p.pattern() {
            Pattern::List(list) => {
                let names: Vec<&str> = list
                    .elements()
                    .map(|el| match el {
                        ListPatternElement::Pattern(Pattern::Variable(variable)) => {
                            variable.name().name()
                        }
                        other => panic!("expected variable target, got {other:?}"),
                    })
                    .collect();
                assert_eq!(names, ["a", "b"]);
            }
            other => panic!("expected list pattern, got {other:?}"),
        },
        other => panic!("expected pattern assignment, got {other:?}"),
    }
}

#[test]
fn test_record_literal_expr_not_pattern_assign() {
    // `(a, b);` with no `=` is a record expression statement, not an assignment.
    let source = format!("void f() async {{ {} }}", "(a, b);");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Record(_)), "got {:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_paren_field_assign_not_pattern_assign() {
    // `(x).f = 1;` is an ordinary assignment expression, not a pattern assign.
    let source = format!("void f() async {{ {} }}", "(x).f = 1;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Assign(_)), "got {:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_if_case_guard_bare_pattern() {
    let source = format!("void f() async {{ {} }}", "if (x case p when g) {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::If(i) => match i.condition() {
            IfCondition::Case(condition) => {
                let pattern = condition.pattern();
                let guard = condition.guard();
                assert!(matches!(pattern, Pattern::Const(_)), "pattern: {pattern:?}");
                let guard = guard.expect("expected a when-guard");
                assert!(matches!(guard, Expr::Ident(_)), "guard: {guard:?}");
            }
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

#[test]
fn test_if_case_guard_dotted_pattern() {
    let source = format!("void f() async {{ {} }}", "if (x case State.a when c) {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::If(i) => match i.condition() {
            IfCondition::Case(condition) => {
                let pattern = condition.pattern();
                let guard = condition.guard();
                match pattern {
                    Pattern::Const(constant) => {
                        let ConstPatternValue::Name(name) = constant.value() else {
                            panic!("expected Name constant value")
                        };
                        let segs: Vec<_> = name.map(|s| s.name()).collect();
                        assert_eq!(segs, ["State", "a"]);
                    }
                    other => panic!("expected const pattern, got {other:?}"),
                }
                assert!(guard.is_some(), "expected a when-guard");
            }
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

#[test]
fn test_if_case_without_guard() {
    // if-case without `when` leaves the guard field empty.
    let source = format!("void f() async {{ {} }}", "if (x case final y) {}");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::If(i) => match i.condition() {
            IfCondition::Case(condition) => assert!(condition.guard().is_none()),
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

#[test]
fn test_if_case_typed_variable_guard() {
    // A typed variable pattern followed by a guard: `String s when s.isNotEmpty`.
    let source = format!(
        "void f() async {{ {} }}",
        "if (x case String s when s.isNotEmpty) {}"
    );
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::If(i) => match i.condition() {
            IfCondition::Case(condition) => {
                let pattern = condition.pattern();
                let guard = condition.guard();
                assert!(
                    matches!(pattern, Pattern::Variable(_)),
                    "pattern: {pattern:?}"
                );
                assert!(guard.is_some());
            }
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

#[test]
fn label_between_switch_cases() {
    let source = format!(
        "void f() async {{ {} }}",
        "switch (x) { case 1: continue lbl; lbl: case 2: break; }"
    );
    let parsed = parse(&source);
    let (_stmts, e) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(e, 0);
}

#[test]
fn test_brace_with_literal_then_semicolon_is_block() {
    // `{ 1; }` — a leading literal makes it *look* like a set, but the top-level
    // `;` proves it is a block of statements.
    let source = format!("void f() async {{ {} }}", "{ 1; }");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Block(b) => {
            assert_eq!(b.stmts().len(), 1);
            assert!(
                matches!(b.stmts().next().unwrap(), Stmt::Expr(_)),
                "{:?}",
                b.stmts().next().unwrap()
            );
        }
        other => panic!("expected block, got {other:?}"),
    }
}

#[test]
fn test_brace_set_literal_statement_still_expr() {
    // Control: `{1};` (no inner `;`) stays a set-literal expression statement.
    let source = format!("void f() async {{ {} }}", "{1};");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Set(_)), "{:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_paren_assignment_statement() {
    // `(v = 0) ?? 0;` — the leading `(` must not be misparsed as a pattern
    // assignment; it is an ordinary parenthesised assignment expression.
    let source = format!("void f() async {{ {} }}", "(v = 0) ?? 0;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(
            matches!(
                e.expr(),
                Expr::Binary(binary) if binary.op() == BinaryOp::NullCoalesce
            ),
            "{:?}",
            e.expr()
        ),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_conditional_with_assignments_statement() {
    // `a ? b = c : d = e;` — a ternary expression statement whose branches assign.
    let source = format!("void f() async {{ {} }}", "a ? b = c : d = e;");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr(), Expr::Conditional(_)), "{:?}", e.expr()),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_parenthesised_collection_with_complex_element() {
    // `({if (a) b = c});` — a parenthesised set literal whose element is an
    // `if` comprehension with an assignment body. The leading `(` must not be
    // taken for a pattern assignment.
    let source = format!("void f() async {{ {} }}", "({if (a) b = c});");
    let parsed = parse(&source);
    assert_eq!(
        parsed.errors.len(),
        0,
        "expected zero errors for {source:?}"
    );
    let stmt = only_stmt(parsed.tree.program(&source));
    match stmt {
        Stmt::Expr(e) => assert!(
            matches!(e.expr().unparenthesized(), Expr::Set(_)),
            "{:?}",
            e.expr()
        ),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_yield_as_identifier_in_plain_function() {
    let source = format!("void f() {{ {} }}", "var yield = 0; yield = 1;");
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "yield is a valid identifier here: {stmts:?}");
    // Second statement is an assignment to the local `yield`, not a yield stmt.
    match stmts.clone().nth(1).unwrap() {
        Stmt::Expr(e) => assert!(
            matches!(e.expr(), Expr::Assign(_)),
            "expected assignment, got {:?}",
            e.expr()
        ),
        other => panic!("expected expr stmt, got {other:?}"),
    }
}

#[test]
fn test_yield_method_call_is_member_access_not_yield_stmt() {
    // `yield.toString()` on a local `yield` must parse as a member call, not a
    // YieldStmt of a dot-shorthand (the silent wrong-AST regression).
    let source = format!("void f() {{ {} }}", "var yield = 0; yield.toString();");
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "{stmts:?}");
    let expr = match stmts.clone().nth(1).unwrap() {
        Stmt::Expr(e) => e.expr(),
        other => panic!("expected expr stmt, got {other:?}"),
    };
    match expr {
        Expr::Call(call) => match call.callee() {
            Expr::Field(access) => {
                let object = access.object();
                let field = access.field();
                assert!(matches!(object, Expr::Ident(id) if id.name() == "yield"));
                assert_eq!(field.name(), "toString");
            }
            other => panic!("expected field access callee, got {other:?}"),
        },
        other => panic!("expected call, got {other:?}"),
    }
    assert!(
        !stmts.clone().any(|s| matches!(s, Stmt::Yield(_))),
        "must not produce a yield statement"
    );
}

#[test]
fn test_yield_still_reserved_in_generator() {
    // Inside a `sync*` generator, `yield` remains the yield keyword.
    let source = "Iterable<int> f() sync* { yield 1; }";
    let parsed = parse(source);
    let errors = &parsed.errors;
    let prog = parsed.tree.program(source);
    assert_eq!(errors.len(), 0, "{errors:?}");
    let func = match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    let block = match func.body().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("got {other:?}"),
    };
    assert!(
        matches!(block.stmts().next().unwrap(), Stmt::Yield(_)),
        "expected yield stmt, got {:?}",
        block.stmts().next().unwrap()
    );
}

#[test]
fn test_await_as_identifier_in_plain_function() {
    let source = format!(
        "void f() {{ {} }}",
        "var await = 0; await = 1; print(await + 1);"
    );
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "await is a valid identifier here: {stmts:?}");
    match stmts.clone().nth(1).unwrap() {
        Stmt::Expr(e) => assert!(
            matches!(e.expr(), Expr::Assign(_)),
            "expected assignment, got {:?}",
            e.expr()
        ),
        other => panic!("expected expr stmt, got {other:?}"),
    }
}

#[test]
fn test_await_still_reserved_in_async_function() {
    // Inside an `async` body, a leading `await` is still an await-expression.
    let source = "void f() async { await g(); }";
    let parsed = parse(source);
    let errors = &parsed.errors;
    let prog = parsed.tree.program(source);
    assert_eq!(errors.len(), 0, "{errors:?}");
    let func = match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    let block = match func.body().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("got {other:?}"),
    };
    match block.stmts().next().unwrap() {
        Stmt::Expr(e) => assert!(
            matches!(e.expr(), Expr::Await(_)),
            "expected await expr, got {:?}",
            e.expr()
        ),
        other => panic!("expected expr stmt, got {other:?}"),
    }
}

#[test]
fn test_metadata_on_for_each_var() {
    let source = format!(
        "void f() {{ {} }}",
        "for (@meta var x in [1, 2]) { print(x); }"
    );
    let parsed = parse(&source);
    let (_, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "annotation on for-each var must parse");
}

#[test]
fn test_metadata_on_for_each_final() {
    let source = format!(
        "void f() {{ {} }}",
        "for (@meta final x in [1]) { print(x); }"
    );
    let parsed = parse(&source);
    let (_, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "annotation on for-each final must parse");
}

#[test]
fn test_metadata_on_c_style_for_var() {
    let source = format!(
        "void f() {{ {} }}",
        "for (@meta var i = 0; i < 3; i++) { print(i); }"
    );
    let parsed = parse(&source);
    let (_, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "annotation on C-style for var must parse");
}

#[test]
fn test_metadata_on_pattern_for_each() {
    let source = format!(
        "void f() {{ {} }}",
        "for (@meta var (a, b) in <(int, int)>[]) { print(a + b); }"
    );
    let parsed = parse(&source);
    let (_, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "annotation on pattern for-each must parse");
}

#[test]
fn test_deep_block_nesting_does_not_overflow() {
    // Deeply nested blocks recurse through parse_stmt/parse_block. Run on a
    // large-stack worker so the guard is reached before the default test stack
    // is exhausted; parsing must terminate with an error, never abort.
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let n = 3000;
            let src = format!("void f() {{ {}{} }}", "{".repeat(n), "}".repeat(n));
            let falcon_dart_parser::ParseResult { errors, .. } = parse(&src);
            errors
                .iter()
                .any(|e| e.message.contains("nesting too deep"))
        })
        .unwrap();
    assert!(
        handle.join().expect("parser thread must not panic/abort"),
        "expected a nesting-too-deep error for deeply nested blocks"
    );
}

#[test]
fn test_await_for_rejected_in_plain_function() {
    // `await for` is only legal in an async body; in a sync body `await` is an
    // identifier and this must NOT silently parse as an await-for loop.
    let source = format!("void f() {{ {} }}", "await for (var x in xs) { print(x); }");
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert!(errors > 0, "sync-body await-for must error: {stmts:?}");
    assert!(
        !stmts
            .clone()
            .any(|s| matches!(s, Stmt::For(f) if f.is_await())),
        "must not build an await-for in a sync body: {stmts:?}"
    );
}

#[test]
fn test_await_for_accepted_in_async_function() {
    let source = "void f(Stream<int> xs) async { await for (var x in xs) { print(x); } }";
    let parsed = parse(source);
    let errors = &parsed.errors;
    let prog = parsed.tree.program(source);
    assert_eq!(errors.len(), 0, "{errors:?}");
    let func = match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    let block = match func.body().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("got {other:?}"),
    };
    assert!(
        matches!(block.stmts().next().unwrap(), Stmt::For(f) if f.is_await()),
        "expected await-for, got {:?}",
        block.stmts().next().unwrap()
    );
}

#[test]
fn annotated_local_function_keeps_metadata_and_body() {
    let source = format!(
        "void f() {{ {} }}",
        "@pragma('vm:prefer-inline') void round(int i) { i; } round(1);"
    );
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0);
    assert_eq!(stmts.len(), 2);
    let Stmt::LocalFunc(function) = stmts.clone().next().unwrap() else {
        panic!(
            "expected local function: {:?}",
            stmts.clone().next().unwrap()
        );
    };
    assert_eq!(function.name().name(), "round");
    assert_eq!(function.annotations().len(), 1);
    assert_eq!(function.params().params().len(), 1);
}

#[test]
fn annotated_local_variable_keeps_metadata() {
    let source = format!(
        "void f() {{ {} }}",
        "@a @b final int x = 1; @c var y = 2; @d int z = 3;"
    );
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0);
    let counts: Vec<usize> = stmts
        .clone()
        .map(|stmt| match stmt {
            Stmt::LocalVar(decl) => decl.annotations().len(),
            other => panic!("expected local variable: {other:?}"),
        })
        .collect();
    assert_eq!(counts, [2, 1, 1]);
}

#[test]
fn object_pattern_assignment_statement() {
    let source = format!(
        "void f() {{ {} }}",
        "AsyncError(:error, :stackTrace) = intercept(); p.Box<int>(:value) = box; f(x) == y;"
    );
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0);
    assert!(
        matches!(stmts.clone().next().unwrap(), Stmt::PatternAssign(assign) if matches!(assign.pattern(), Pattern::Object(_)))
    );
    assert!(matches!(
        stmts.clone().nth(1).unwrap(),
        Stmt::PatternAssign(_)
    ));
    assert!(matches!(stmts.clone().nth(2).unwrap(), Stmt::Expr(_)));
}

#[test]
fn test_misplaced_await_in_sync_body_is_await_expression() {
    // The front end parses `await operand` in a sync body as a (misplaced)
    // await expression rather than a dangling identifier.
    let source = format!(
        "void f() {{ {} }}",
        "await SystemChrome.reset(<int>[]); await f() + 1; await = 1;"
    );
    let parsed = parse(&source);
    let (stmts, errors) = (
        body_stmts(parsed.tree.program(&source)),
        parsed.errors.len(),
    );
    assert_eq!(errors, 0, "{stmts:?}");
    assert!(
        matches!(stmts.clone().next().unwrap(), Stmt::Expr(e) if matches!(e.expr(), Expr::Await(_)))
    );
    assert!(
        matches!(stmts.clone().nth(1).unwrap(), Stmt::Expr(e) if matches!(e.expr(), Expr::Binary(_)))
    );
    assert!(
        matches!(stmts.clone().nth(2).unwrap(), Stmt::Expr(e) if matches!(e.expr(), Expr::Assign(_)))
    );
}
