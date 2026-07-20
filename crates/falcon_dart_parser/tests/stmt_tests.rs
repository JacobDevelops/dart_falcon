//! Regression tests for statement-level parser gaps: each construct must parse
//! with zero errors and produce a faithful AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

/// Parse `src` as the body of an async function and return its statements plus
/// the parse-error count. The wrapper is `async` so `await`/`yield` statements
/// parse in their natural context.
fn body_stmts(src: &str) -> (Vec<Stmt>, usize) {
    let (prog, errors) = parse(&format!("void f() async {{ {src} }}"));
    let func = match prog.declarations.first() {
        Some(TopLevelDecl::Function(f)) => f,
        other => panic!("expected function, got {other:?}\nerrors: {errors:?}"),
    };
    let block = match func.body.as_ref() {
        Some(FunctionBody::Block(b)) => b,
        other => panic!("expected block body, got {other:?}"),
    };
    (block.stmts.clone(), errors.len())
}

/// Parse `src` as a function body and return its single statement, asserting
/// zero parse errors and exactly one statement.
fn only_stmt(src: &str) -> Stmt {
    let (stmts, errors) = body_stmts(src);
    assert_eq!(errors, 0, "expected zero errors for {src:?}");
    assert_eq!(
        stmts.len(),
        1,
        "expected one statement for {src:?}: {stmts:?}"
    );
    stmts.into_iter().next().unwrap()
}

// ── Item (a): labeled statements ──────────────────────────────────────────────

#[test]
fn test_labeled_for_loop() {
    let stmt = only_stmt("outer: for (;;) { break outer; }");
    let labeled = match stmt {
        Stmt::Labeled(l) => l,
        other => panic!("expected labeled statement, got {other:?}"),
    };
    assert_eq!(labeled.label.name, "outer");
    assert!(
        matches!(*labeled.stmt, Stmt::For(_)),
        "inner: {:?}",
        labeled.stmt
    );
}

#[test]
fn test_labeled_while_loop() {
    let stmt = only_stmt("loop: while (true) { continue loop; }");
    match stmt {
        Stmt::Labeled(l) => {
            assert_eq!(l.label.name, "loop");
            assert!(matches!(*l.stmt, Stmt::While(_)));
        }
        other => panic!("expected labeled statement, got {other:?}"),
    }
}

#[test]
fn test_nested_labels() {
    // Two labels stacking, each wrapping the next.
    let stmt = only_stmt("a: b: for (;;) {}");
    match stmt {
        Stmt::Labeled(outer) => {
            assert_eq!(outer.label.name, "a");
            match *outer.stmt {
                Stmt::Labeled(inner) => {
                    assert_eq!(inner.label.name, "b");
                    assert!(matches!(*inner.stmt, Stmt::For(_)));
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
    let stmt = only_stmt("x ? a : b;");
    match stmt {
        Stmt::Expr(e) => assert!(
            matches!(e.expr, Expr::Conditional { .. }),
            "got {:?}",
            e.expr
        ),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_label_does_not_break_map_literal_in_decl() {
    // A map literal in a var initializer keeps its `key: value` colons.
    let (stmts, errors) = body_stmts("var m = {'a': 1, 'b': 2};");
    assert_eq!(errors, 0);
    match &stmts[0] {
        Stmt::LocalVar(v) => {
            let init = v.declarators[0].initializer.as_ref().unwrap();
            assert!(matches!(init, Expr::Map { .. }), "got {init:?}");
        }
        other => panic!("expected local var, got {other:?}"),
    }
}

// ── Item (b): `await for` ─────────────────────────────────────────────────────

#[test]
fn test_await_for_in() {
    let stmt = only_stmt("await for (var e in s) {}");
    match stmt {
        Stmt::For(f) => {
            assert!(f.is_await, "expected await-for");
            match f.init {
                Some(ForInit::ForIn { name, .. }) => assert_eq!(name.name, "e"),
                other => panic!("expected for-in, got {other:?}"),
            }
        }
        other => panic!("expected for statement, got {other:?}"),
    }
}

#[test]
fn test_plain_for_is_not_await() {
    let stmt = only_stmt("for (var e in s) {}");
    match stmt {
        Stmt::For(f) => assert!(!f.is_await),
        other => panic!("expected for statement, got {other:?}"),
    }
}

#[test]
fn test_labeled_await_for() {
    // `await for` still works when labeled.
    let stmt = only_stmt("outer: await for (final e in s) { break outer; }");
    match stmt {
        Stmt::Labeled(l) => match *l.stmt {
            Stmt::For(f) => assert!(f.is_await),
            other => panic!("expected for, got {other:?}"),
        },
        other => panic!("expected labeled, got {other:?}"),
    }
}

// ── Item (c): untyped local functions ─────────────────────────────────────────

#[test]
fn test_untyped_local_func_block_and_arrow() {
    let (stmts, errors) = body_stmts("foo() {} bar() => 1;");
    assert_eq!(errors, 0);
    assert_eq!(stmts.len(), 2, "stmts: {stmts:?}");
    match &stmts[0] {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name.name, "foo");
            assert!(f.return_type.is_none());
            assert!(matches!(f.body, FunctionBody::Block(_)));
        }
        other => panic!("expected local func, got {other:?}"),
    }
    match &stmts[1] {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name.name, "bar");
            assert!(f.return_type.is_none());
            assert!(matches!(f.body, FunctionBody::Arrow(..)));
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_untyped_local_func_with_params() {
    let stmt = only_stmt("greet(String name) => print(name);");
    match stmt {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name.name, "greet");
            assert!(f.return_type.is_none());
            assert_eq!(f.params.positional.len(), 1);
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_untyped_generic_local_func() {
    let stmt = only_stmt("ident<T>(T x) => x;");
    match stmt {
        Stmt::LocalFunc(f) => {
            assert_eq!(f.name.name, "ident");
            assert_eq!(f.type_params.len(), 1);
            assert!(f.return_type.is_none());
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_untyped_async_local_func() {
    let stmt = only_stmt("load() async {}");
    match stmt {
        Stmt::LocalFunc(f) => {
            assert!(f.is_async);
            assert!(!f.is_generator);
            assert!(f.return_type.is_none());
        }
        other => panic!("expected local func, got {other:?}"),
    }
}

#[test]
fn test_plain_call_stays_call() {
    // A bare call statement must NOT become a local function.
    let stmt = only_stmt("foo();");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Call { .. }), "got {:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_generic_call_stays_call() {
    // `foo<int>();` is a generic invocation, not a local function.
    let stmt = only_stmt("foo<int>();");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Call { .. }), "got {:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_call_with_args_stays_call() {
    let stmt = only_stmt("foo(a, b);");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Call { .. }), "got {:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

// ── Item (d): pattern assignment ──────────────────────────────────────────────

#[test]
fn test_record_pattern_assignment() {
    let stmt = only_stmt("(a, b) = e;");
    match stmt {
        Stmt::PatternAssign(p) => {
            assert!(
                matches!(p.pattern, Pattern::Record(_)),
                "got {:?}",
                p.pattern
            );
            assert!(matches!(p.value, Expr::Ident(_)));
        }
        other => panic!("expected pattern assignment, got {other:?}"),
    }
}

#[test]
fn test_list_pattern_assignment() {
    let stmt = only_stmt("[a, b] = e;");
    match stmt {
        Stmt::PatternAssign(p) => {
            assert!(matches!(p.pattern, Pattern::List(_)), "got {:?}", p.pattern);
        }
        other => panic!("expected pattern assignment, got {other:?}"),
    }
}

#[test]
fn test_record_pattern_assign_targets_are_variables() {
    // Bare-identifier assignment targets must bind as `Pattern::Variable` (which
    // the visitor walks) rather than `Pattern::Const{expr:None}` (walked as
    // nothing), matching the declaration twin `var (a, b) = e;`.
    let stmt = only_stmt("(a, b) = e;");
    match stmt {
        Stmt::PatternAssign(p) => match p.pattern {
            Pattern::Record(rec) => {
                let names: Vec<&str> = rec
                    .fields
                    .iter()
                    .map(|f| match &f.pattern {
                        Pattern::Variable { name, .. } => name.name.as_str(),
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
    let stmt = only_stmt("[a, b] = e;");
    match stmt {
        Stmt::PatternAssign(p) => match p.pattern {
            Pattern::List(list) => {
                let names: Vec<&str> = list
                    .elements
                    .iter()
                    .map(|el| match el {
                        ListPatternElement::Pattern(Pattern::Variable { name, .. }) => {
                            name.name.as_str()
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
    let stmt = only_stmt("(a, b);");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Record { .. }), "got {:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_paren_field_assign_not_pattern_assign() {
    // `(x).f = 1;` is an ordinary assignment expression, not a pattern assign.
    let stmt = only_stmt("(x).f = 1;");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Assign { .. }), "got {:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

// ── Item (e): if-case guards ──────────────────────────────────────────────────

#[test]
fn test_if_case_guard_bare_pattern() {
    let stmt = only_stmt("if (x case p when g) {}");
    match stmt {
        Stmt::If(i) => match i.condition {
            IfCondition::Case(_, pattern, guard) => {
                assert!(
                    matches!(*pattern, Pattern::Const(_)),
                    "pattern: {pattern:?}"
                );
                let guard = guard.expect("expected a when-guard");
                assert!(matches!(*guard, Expr::Ident(_)), "guard: {guard:?}");
            }
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

#[test]
fn test_if_case_guard_dotted_pattern() {
    let stmt = only_stmt("if (x case State.a when c) {}");
    match stmt {
        Stmt::If(i) => match i.condition {
            IfCondition::Case(_, pattern, guard) => {
                match *pattern {
                    Pattern::Const(c) => {
                        let segs: Vec<_> = c.name.iter().map(|s| s.name.as_str()).collect();
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
    let stmt = only_stmt("if (x case final y) {}");
    match stmt {
        Stmt::If(i) => match i.condition {
            IfCondition::Case(_, _, guard) => assert!(guard.is_none()),
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

#[test]
fn test_if_case_typed_variable_guard() {
    // A typed variable pattern followed by a guard: `String s when s.isNotEmpty`.
    let stmt = only_stmt("if (x case String s when s.isNotEmpty) {}");
    match stmt {
        Stmt::If(i) => match i.condition {
            IfCondition::Case(_, pattern, guard) => {
                assert!(
                    matches!(*pattern, Pattern::Variable { .. }),
                    "pattern: {pattern:?}"
                );
                assert!(guard.is_some());
            }
            other => panic!("expected case condition, got {other:?}"),
        },
        other => panic!("expected if statement, got {other:?}"),
    }
}

// ── Corpus-found statement gaps ──────────────────────────────────────────

#[test]
fn label_between_switch_cases() {
    let (_stmts, e) = body_stmts("switch (x) { case 1: continue lbl; lbl: case 2: break; }");
    assert_eq!(e, 0);
}

// ── Item: `{ literal; ... }` at statement start is a block, not a set literal ──

#[test]
fn test_brace_with_literal_then_semicolon_is_block() {
    // `{ 1; }` — a leading literal makes it *look* like a set, but the top-level
    // `;` proves it is a block of statements.
    let stmt = only_stmt("{ 1; }");
    match stmt {
        Stmt::Block(b) => {
            assert_eq!(b.stmts.len(), 1);
            assert!(matches!(b.stmts[0], Stmt::Expr(_)), "{:?}", b.stmts[0]);
        }
        other => panic!("expected block, got {other:?}"),
    }
}

#[test]
fn test_brace_set_literal_statement_still_expr() {
    // Control: `{1};` (no inner `;`) stays a set-literal expression statement.
    let stmt = only_stmt("{1};");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Set { .. }), "{:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

// ── Item: assignment / complex expressions in statement-leading parens ─────────

#[test]
fn test_paren_assignment_statement() {
    // `(v = 0) ?? 0;` — the leading `(` must not be misparsed as a pattern
    // assignment; it is an ordinary parenthesised assignment expression.
    let stmt = only_stmt("(v = 0) ?? 0;");
    match stmt {
        Stmt::Expr(e) => assert!(
            matches!(
                e.expr,
                Expr::Binary {
                    op: BinaryOp::NullCoalesce,
                    ..
                }
            ),
            "{:?}",
            e.expr
        ),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_conditional_with_assignments_statement() {
    // `a ? b = c : d = e;` — a ternary expression statement whose branches assign.
    let stmt = only_stmt("a ? b = c : d = e;");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Conditional { .. }), "{:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

#[test]
fn test_parenthesised_collection_with_complex_element() {
    // `({if (a) b = c});` — a parenthesised set literal whose element is an
    // `if` comprehension with an assignment body. The leading `(` must not be
    // taken for a pattern assignment.
    let stmt = only_stmt("({if (a) b = c});");
    match stmt {
        Stmt::Expr(e) => assert!(matches!(e.expr, Expr::Set { .. }), "{:?}", e.expr),
        other => panic!("expected expression statement, got {other:?}"),
    }
}

// ── Contextual keywords: yield/await as identifiers in non-generator/non-async ─

/// Parse `src` as the body of a plain (non-async, non-generator) function and
/// return its statements plus the parse-error count. In this context `await`
/// and `yield` are ordinary identifiers, not reserved words.
fn sync_body_stmts(src: &str) -> (Vec<Stmt>, usize) {
    let (prog, errors) = parse(&format!("void f() {{ {src} }}"));
    let func = match prog.declarations.first() {
        Some(TopLevelDecl::Function(f)) => f,
        other => panic!("expected function, got {other:?}\nerrors: {errors:?}"),
    };
    let block = match func.body.as_ref() {
        Some(FunctionBody::Block(b)) => b,
        other => panic!("expected block body, got {other:?}"),
    };
    (block.stmts.clone(), errors.len())
}

#[test]
fn test_yield_as_identifier_in_plain_function() {
    let (stmts, errors) = sync_body_stmts("var yield = 0; yield = 1;");
    assert_eq!(errors, 0, "yield is a valid identifier here: {stmts:?}");
    // Second statement is an assignment to the local `yield`, not a yield stmt.
    match &stmts[1] {
        Stmt::Expr(e) => assert!(
            matches!(e.expr, Expr::Assign { .. }),
            "expected assignment, got {:?}",
            e.expr
        ),
        other => panic!("expected expr stmt, got {other:?}"),
    }
}

#[test]
fn test_yield_method_call_is_member_access_not_yield_stmt() {
    // `yield.toString()` on a local `yield` must parse as a member call, not a
    // YieldStmt of a dot-shorthand (the silent wrong-AST regression).
    let (stmts, errors) = sync_body_stmts("var yield = 0; yield.toString();");
    assert_eq!(errors, 0, "{stmts:?}");
    let expr = match &stmts[1] {
        Stmt::Expr(e) => &e.expr,
        other => panic!("expected expr stmt, got {other:?}"),
    };
    match expr {
        Expr::Call { callee, .. } => match &**callee {
            Expr::Field { object, field, .. } => {
                assert!(matches!(**object, Expr::Ident(ref id) if id.name == "yield"));
                assert_eq!(field.name, "toString");
            }
            other => panic!("expected field access callee, got {other:?}"),
        },
        other => panic!("expected call, got {other:?}"),
    }
    assert!(
        !stmts.iter().any(|s| matches!(s, Stmt::Yield(_))),
        "must not produce a yield statement"
    );
}

#[test]
fn test_yield_still_reserved_in_generator() {
    // Inside a `sync*` generator, `yield` remains the yield keyword.
    let (prog, errors) = parse("Iterable<int> f() sync* { yield 1; }");
    assert_eq!(errors.len(), 0, "{errors:?}");
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    let block = match func.body.as_ref().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("got {other:?}"),
    };
    assert!(
        matches!(block.stmts[0], Stmt::Yield(_)),
        "expected yield stmt, got {:?}",
        block.stmts[0]
    );
}

#[test]
fn test_await_as_identifier_in_plain_function() {
    let (stmts, errors) = sync_body_stmts("var await = 0; await = 1; print(await + 1);");
    assert_eq!(errors, 0, "await is a valid identifier here: {stmts:?}");
    match &stmts[1] {
        Stmt::Expr(e) => assert!(
            matches!(e.expr, Expr::Assign { .. }),
            "expected assignment, got {:?}",
            e.expr
        ),
        other => panic!("expected expr stmt, got {other:?}"),
    }
}

#[test]
fn test_await_still_reserved_in_async_function() {
    // Inside an `async` body, a leading `await` is still an await-expression.
    let (prog, errors) = parse("void f() async { await g(); }");
    assert_eq!(errors.len(), 0, "{errors:?}");
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    let block = match func.body.as_ref().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("got {other:?}"),
    };
    match &block.stmts[0] {
        Stmt::Expr(e) => assert!(
            matches!(e.expr, Expr::Await { .. }),
            "expected await expr, got {:?}",
            e.expr
        ),
        other => panic!("expected expr stmt, got {other:?}"),
    }
}

// ── Metadata on for-loop variables (all three forms) ──────────────────────────

#[test]
fn test_metadata_on_for_each_var() {
    let (_, errors) = sync_body_stmts("for (@meta var x in [1, 2]) { print(x); }");
    assert_eq!(errors, 0, "annotation on for-each var must parse");
}

#[test]
fn test_metadata_on_for_each_final() {
    let (_, errors) = sync_body_stmts("for (@meta final x in [1]) { print(x); }");
    assert_eq!(errors, 0, "annotation on for-each final must parse");
}

#[test]
fn test_metadata_on_c_style_for_var() {
    let (_, errors) = sync_body_stmts("for (@meta var i = 0; i < 3; i++) { print(i); }");
    assert_eq!(errors, 0, "annotation on C-style for var must parse");
}

#[test]
fn test_metadata_on_pattern_for_each() {
    let (_, errors) =
        sync_body_stmts("for (@meta var (a, b) in <(int, int)>[]) { print(a + b); }");
    assert_eq!(errors, 0, "annotation on pattern for-each must parse");
}

// ── Recursion-depth guard on statement nesting ────────────────────────────────

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
            let (_, errors) = parse(&src);
            errors.iter().any(|e| e.message.contains("nesting too deep"))
        })
        .unwrap();
    assert!(
        handle.join().expect("parser thread must not panic/abort"),
        "expected a nesting-too-deep error for deeply nested blocks"
    );
}
