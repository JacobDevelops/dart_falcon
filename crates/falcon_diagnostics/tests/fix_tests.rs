use falcon_diagnostics::{ApplyFixError, Diagnostic, Fix, FixKind, Severity, Span, TextEdit};
use serde_json::json;

fn edit(start: usize, end: usize, replacement: &str) -> TextEdit {
    TextEdit::new(Span { start, end }, replacement)
}

#[test]
fn applies_original_source_offsets_preserving_trivia_and_unicode() {
    let source = "// café\r\nvar value = 1; // kept\r\n";
    let start = source.find("var").unwrap();
    let end = source.find('1').unwrap();
    let fix = Fix::new(
        FixKind::Safe,
        vec![edit(start, start + 3, "final"), edit(end, end + 1, "123")],
    );
    assert_eq!(
        fix.apply(source).unwrap(),
        "// café\r\nfinal value = 123; // kept\r\n"
    );
}

#[test]
fn applies_insertions_deletions_and_adjacent_edits_in_order() {
    let fix = Fix::new(
        FixKind::Unsafe,
        vec![
            edit(0, 0, "first"),
            edit(0, 0, " second"),
            edit(0, 1, ""),
            edit(1, 2, "!"),
            edit(2, 2, " end"),
        ],
    );
    assert_eq!(fix.apply("ab").unwrap(), "first second! end");
    assert_eq!(Fix::new(FixKind::Safe, vec![]).apply("é").unwrap(), "é");
    assert_eq!(
        Fix::new(FixKind::Safe, vec![edit(0, 0, "x")])
            .apply("")
            .unwrap(),
        "x"
    );
}

#[test]
fn rejects_invalid_byte_spans_without_panicking() {
    for (start, end) in [(2, 1), (0, 4), (4, 4), (1, 2), (0, 1)] {
        let fix = Fix::new(FixKind::Safe, vec![edit(start, end, "x")]);
        assert_eq!(
            fix.apply("éa"),
            Err(ApplyFixError::InvalidSpan { index: 0 })
        );
    }
    let fix = Fix::new(FixKind::Safe, vec![edit(0, 0, "x"), edit(1, 2, "y")]);
    assert_eq!(
        fix.apply("éa"),
        Err(ApplyFixError::InvalidSpan { index: 1 })
    );
}

#[test]
fn rejects_unsorted_and_overlapping_edits() {
    for edits in [
        vec![edit(2, 3, "x"), edit(0, 1, "y")],
        vec![edit(0, 2, "x"), edit(1, 3, "y")],
        vec![edit(0, 2, "x"), edit(1, 1, "y")],
        vec![edit(2, 2, "x"), edit(1, 1, "y")],
    ] {
        assert_eq!(
            Fix::new(FixKind::Safe, edits).apply("abc"),
            Err(ApplyFixError::UnsortedOrOverlapping { index: 1 })
        );
    }
}

#[test]
fn json_and_lsp_preserve_fix_kind_and_original_byte_edits() {
    for (kind, name) in [(FixKind::Safe, "safe"), (FixKind::Unsafe, "unsafe")] {
        let mut diagnostic = Diagnostic::new(
            "test-rule",
            Severity::Warning,
            "message",
            "test.dart",
            Span { start: 0, end: 2 },
        )
        .with_fix(Fix::new(kind, vec![edit(0, 2, "a")]));
        diagnostic.resolve_position("é");
        let expected = json!({
            "kind": name,
            "edits": [{ "span": { "start": 0, "end": 2 }, "replacement": "a" }]
        });
        assert_eq!(serde_json::to_value(&diagnostic).unwrap()["fix"], expected);
        assert_eq!(
            diagnostic.clone().format_lsp("é").data,
            Some(json!({ "fix": expected }))
        );
        assert_eq!(diagnostic.fix.unwrap().apply("é").unwrap(), "a");
    }
}

#[test]
fn diagnostics_without_fixes_keep_existing_json_and_lsp_shape() {
    let diagnostic = Diagnostic::new(
        "test-rule",
        Severity::Warning,
        "message",
        "test.dart",
        Span { start: 0, end: 1 },
    );
    assert!(
        serde_json::to_value(&diagnostic)
            .unwrap()
            .get("fix")
            .is_none()
    );
    assert!(diagnostic.format_lsp("a").data.is_none());
}
