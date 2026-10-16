//! AST-consumer compatibility tests, not analyzer diagnostic fixtures.
//! The language override changes case operands from patterns to expressions;
//! rules must retain their existing behavior on the same operand syntax.

use std::path::Path;

use falcon_analyze::{
    AnalyzeContext, IdentityIndex, IdentitySource, Rule, RuleAnalysis, TypeIndex,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_rules::lint::complexity::max_switch_cases::MaxSwitchCases;
use falcon_rules::lint::correctness::exhaustive_cases::ExhaustiveCases;
use falcon_rules::lint::correctness::no_adjacent_strings_in_list::NoAdjacentStringsInList;
use falcon_syntax::ast::{Stmt, SwitchCaseKind};
use falcon_syntax::visitor::{Visitor, walk_stmt};

fn case_program(source: &str, legacy: bool) -> falcon_dart_parser::ParseResult {
    let source = format!(
        "// @dart = {}\n{source}",
        if legacy { "2.19" } else { "3.13" }
    );
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    struct CaseKinds(bool, usize);
    impl Visitor for CaseKinds {
        fn visit_stmt(&mut self, node: &Stmt) {
            if let Stmt::Switch(switch) = node {
                for case in switch.cases.iter().flat_map(|group| &group.cases) {
                    if matches!(case, SwitchCaseKind::Default) {
                        continue;
                    }
                    assert_eq!(matches!(case, SwitchCaseKind::Expr(_)), self.0);
                    self.1 += 1;
                }
            }
            walk_stmt(self, node);
        }
    }
    let mut kinds = CaseKinds(legacy, 0);
    kinds.visit_program(&parsed.program);
    assert!(kinds.1 > 0);
    parsed
}

fn diagnostics(rule: &dyn Rule, source: &str, legacy: bool) -> Vec<String> {
    let falcon_dart_parser::ParseResult {
        program, syntax, ..
    } = case_program(source, legacy);
    let source = format!(
        "// @dart = {}\n{source}",
        if legacy { "2.19" } else { "3.13" }
    );
    let path = Path::new("legacy-switch.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }],
        &[],
    );
    let types = TypeIndex::from_resolved_files(
        [(
            path,
            falcon_analyze::LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(path, &source, &config, &syntax)
        .with_identities(&identities)
        .with_types(&types);
    let mut messages: Vec<_> = rule
        .analyze(&program, &ctx)
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect();
    messages.sort();
    messages
}

fn assert_compatible(rule: &dyn Rule, source: &str) {
    let modern = diagnostics(rule, source, false);
    assert!(
        !modern.is_empty(),
        "modern source must exercise the consumer"
    );
    assert_eq!(diagnostics(rule, source, true), modern);
}

#[test]
fn legacy_cases_count_toward_switch_limit() {
    assert_compatible(
        &MaxSwitchCases,
        "void f(int value) {
        switch (value) {
            case 0: case 1: case 2: case 3: case 4: case 5:
            case 6: case 7: case 8: case 9: case 10: break;
            default: break;
        }
    }",
    );
}

#[test]
fn legacy_case_expressions_visit_adjacent_strings_in_lists() {
    assert_compatible(
        &NoAdjacentStringsInList,
        "void f(Object value) {
        switch (value) { case const ['a' 'b']: break; }
    }",
    );
}

#[test]
fn legacy_case_expressions_preserve_this_access_shape() {
    // Invalid constant operands constrain AST shape, not lint diagnostics.
    let source = "class C {
        int value = 0;
        void f(Object input) { switch (input) { case const [this.value]: break; } }
    }";
    case_program(source, false);
    case_program(source, true);
}

#[test]
fn legacy_case_expressions_preserve_closure_shape() {
    // A closure is not a legal constant; only check the two case AST forms.
    let source = "var top = 0;
        void f(Object input) {
            switch (input) { case const [(top) => top]: break; }
        }";
    case_program(source, false);
    case_program(source, true);
}

#[test]
fn legacy_pseudoenum_cases_preserve_coverage() {
    let declaration = "class E {
        final int value;
        const E._(this.value);
        static const E a = E._(0);
        static const E b = E._(1);
    }";
    let complete = format!(
        "{declaration}
        void f(E value) {{ switch (value) {{ case E.a: break; case E.b: break; }} }}"
    );
    assert!(diagnostics(&ExhaustiveCases, &complete, false).is_empty());
    assert!(diagnostics(&ExhaustiveCases, &complete, true).is_empty());
    let incomplete = format!(
        "{declaration}
        void f(E value) {{ switch (value) {{ case E.a: break; }} }}"
    );
    assert_compatible(&ExhaustiveCases, &incomplete);
}
