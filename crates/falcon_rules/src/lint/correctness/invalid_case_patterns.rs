//! Flags pre-3.0 switch case expressions that change meaning or break under Dart 3 patterns.
//!
//! Dart 3 reinterprets `case` operands as patterns. Code still on a pre-3.0
//! language version can prepare for the migration by avoiding case expressions
//! that are invalid or mean something different as patterns: non-`const` list,
//! map, or set literals; implicitly-const constructor calls; `identical(...)`
//! calls; `&&`/`||` binary expressions; and the identifier `_` (a wildcard in
//! Dart 3). The lint applies only to libraries whose language version predates
//! 3.0. falcon only knows the version from an explicit `// @dart = 2.x` marker at
//! the top of the file, so files without one are not checked. Case expressions
//! that falcon's Dart 3 parser cannot represent (prefix operators on
//! non-literals, `is`, conditional, arithmetic, and `.length` operands) are
//! parse errors and cannot be inspected.
//!
//! Adopted from the official `invalid_case_patterns` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct InvalidCasePatterns;

impl Rule for InvalidCasePatterns {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if !predates_patterns(ctx.source) {
            return Vec::new();
        }
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// Whether the file opts into a language version below 3.0 via `// @dart = X.Y`.
///
/// The marker must precede the first token of the file; only comment lines and
/// blank lines may come before it.
fn predates_patterns(source: &str) -> bool {
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(comment) = line.strip_prefix("//") else {
            return false;
        };
        let Some(version) = comment
            .trim_start()
            .strip_prefix("@dart")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
        else {
            continue;
        };
        let major = version
            .trim()
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok());
        return major.is_some_and(|major| major < 3);
    }
    false
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check_case(&mut self, pattern: &Pattern) {
        let mut pattern = pattern;
        while let Pattern::ParenPattern { inner, .. } = pattern {
            pattern = inner;
        }
        let invalid = match pattern {
            // A non-const list or map literal in a Dart 2 case.
            Pattern::List(_) | Pattern::Map(_) => true,
            // `Foo(1)` / `identical(a, b)`: an implicitly-const constructor call
            // or an `identical` invocation in Dart 2.
            Pattern::Object(_) => true,
            // `a && b` / `a || b`: binary expressions in Dart 2.
            Pattern::LogicalAnd { .. } | Pattern::LogicalOr { .. } => true,
            // The identifier `_`, which Dart 3 reads as a wildcard.
            Pattern::Wildcard { type_: None, .. } => true,
            Pattern::Wildcard { .. }
            | Pattern::Variable { .. }
            | Pattern::Literal(_)
            | Pattern::Const(_)
            | Pattern::Record(_)
            | Pattern::Relational { .. }
            | Pattern::Cast { .. }
            | Pattern::NullCheck { .. }
            | Pattern::NullAssert { .. }
            | Pattern::ParenPattern { .. }
            | Pattern::Error { .. } => false,
        };
        if invalid {
            self.diags.push(self.ctx.diagnostic(
                "invalid-case-patterns",
                "This expression is not valid in a 'case' clause in Dart 3.0.",
                *pattern.span(),
            ));
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Switch(switch) = node {
            for case in &switch.cases {
                for kind in &case.cases {
                    if let SwitchCaseKind::Pattern(pattern, _) = kind {
                        self.check_case(pattern);
                    }
                }
            }
        }
        walk_stmt(self, node);
    }
}
