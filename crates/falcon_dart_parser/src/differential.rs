use std::fmt::Write;

use falcon_syntax::{ast::Program, cst::SyntaxTree};

/// Asserts the CST's typed views render the same Debug as the AST.
///
/// Equality is checked on compact `{:?}` output: every view's Debug goes through
/// the std builders with the caller's formatter, as derived AST Debug does, so
/// compact equality is equivalent to `{:#?}` equality. Pretty output is only
/// rendered for the mismatch diff because std's `{:#?}` nests one `PadAdapter`
/// per level, costing O(depth²) per line; depth-guard inputs (thousands of
/// left-nested recovery nodes) would otherwise take minutes per parse.
pub(crate) fn assert_equivalent(source: &str, program: &Program, tree: &SyntaxTree) {
    assert!(
        tree.is_lossless(source),
        "CST does not retain every source byte"
    );
    let view = tree.program(source);
    if format!("{program:?}") == format!("{view:?}") {
        return;
    }
    let diff = debug_diff(&format!("{program:#?}"), &format!("{view:#?}"))
        .unwrap_or_else(|| "compact Debug differs but pretty Debug is identical".into());
    panic!("AST/CST Debug mismatch\n{diff}");
}

fn debug_diff(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut line = 1;
    let mut context = [None; 3];
    loop {
        let expected_line = expected_lines.next();
        let actual_line = actual_lines.next();
        if expected_line != actual_line {
            let mut diff = format!("--- AST\n+++ CST\n@@ line {line} @@\n");
            for text in context.iter().cycle().skip(line % 3).take(3).flatten() {
                writeln!(diff, "  {text}").unwrap();
            }
            for text in expected_line.into_iter().chain(expected_lines).take(3) {
                writeln!(diff, "- {text}").unwrap();
            }
            for text in actual_line.into_iter().chain(actual_lines).take(3) {
                writeln!(diff, "+ {text}").unwrap();
            }
            return Some(diff);
        }
        if expected_line.is_none() {
            return Some(format!(
                "--- AST\n+++ CST\n@@ line {line}: trailing newline differs @@\n"
            ));
        }
        context[line % 3] = expected_line;
        line += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::debug_diff;

    #[test]
    fn equal_debug_has_no_diff() {
        assert_eq!(debug_diff("Program {\n}\n", "Program {\n}\n"), None);
    }

    #[test]
    fn diff_reports_first_changed_line_with_bounded_context() {
        assert_eq!(
            debug_diff("same\nold\na\nb\nhidden", "same\nnew\na\nb\nhidden"),
            Some(
                "--- AST\n+++ CST\n@@ line 2 @@\n  same\n- old\n- a\n- b\n+ new\n+ a\n+ b\n".into()
            )
        );
    }

    #[test]
    fn diff_keeps_only_recent_preceding_lines_in_order() {
        assert_eq!(
            debug_diff("a\nb\nc\nd\nold", "a\nb\nc\nd\nnew"),
            Some("--- AST\n+++ CST\n@@ line 5 @@\n  b\n  c\n  d\n- old\n+ new\n".into())
        );
    }

    #[test]
    fn diff_reports_missing_lines() {
        assert_eq!(
            debug_diff("same\nold", "same"),
            Some("--- AST\n+++ CST\n@@ line 2 @@\n  same\n- old\n".into())
        );
    }

    #[test]
    fn diff_reports_trailing_newline_difference() {
        assert!(
            debug_diff("same\n", "same")
                .unwrap()
                .contains("trailing newline differs")
        );
    }
}
