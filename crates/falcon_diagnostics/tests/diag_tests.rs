use falcon_diagnostics::{
    ContextLine, Diagnostic, Severity, Span, byte_to_line_col, byte_to_lsp_position,
};

#[test]
fn test_diagnostic_new() {
    let span = Span { start: 5, end: 10 };
    let diag = Diagnostic::new("DCL001", Severity::Error, "test message", "test.dart", span);

    assert_eq!(diag.rule, "DCL001");
    assert_eq!(diag.code, "DCL001");
    assert_eq!(diag.severity, Severity::Error);
    assert_eq!(diag.message, "test message");
    assert_eq!(diag.file_path, "test.dart");
    assert_eq!(diag.span.start, 5);
    assert_eq!(diag.span.end, 10);
    assert!(diag.suggestions.is_empty());
    assert!(diag.context_lines.is_empty());
}

#[test]
fn test_diagnostic_with_code() {
    let span = Span { start: 0, end: 5 };
    let diag = Diagnostic::new("DCL001", Severity::Warning, "test", "file.dart", span)
        .with_code("CUSTOM001");

    assert_eq!(diag.code, "CUSTOM001");
    assert_eq!(diag.rule, "DCL001");
}

#[test]
fn test_diagnostic_with_message() {
    let span = Span { start: 0, end: 5 };
    let diag = Diagnostic::new("DCL001", Severity::Error, "original", "file.dart", span)
        .with_message("updated message");

    assert_eq!(diag.message, "updated message");
    assert_eq!(diag.rule, "DCL001");
}

#[test]
fn test_diagnostic_with_suggestion() {
    let span = Span { start: 0, end: 5 };
    let diag = Diagnostic::new("DCL001", Severity::Info, "test", "file.dart", span)
        .with_suggestion("fix this", Some("replacement".to_string()))
        .with_suggestion("another fix", None);

    assert_eq!(diag.suggestions.len(), 2);
    assert_eq!(diag.suggestions[0].message, "fix this");
    assert_eq!(
        diag.suggestions[0].replacement,
        Some("replacement".to_string())
    );
    assert_eq!(diag.suggestions[1].message, "another fix");
    assert_eq!(diag.suggestions[1].replacement, None);
}

#[test]
fn test_diagnostic_with_context_lines() {
    let span = Span { start: 0, end: 5 };
    let lines = vec![
        ContextLine {
            line_number: 1,
            content: "void main() {".to_string(),
            is_primary: false,
        },
        ContextLine {
            line_number: 2,
            content: "  doSomething();".to_string(),
            is_primary: true,
        },
    ];
    let diag = Diagnostic::new("DCL001", Severity::Error, "test", "file.dart", span)
        .with_context_lines(lines);

    assert_eq!(diag.context_lines.len(), 2);
    assert!(!diag.context_lines[0].is_primary);
    assert!(diag.context_lines[1].is_primary);
    assert_eq!(diag.context_lines[1].content, "  doSomething();");
}

#[test]
fn test_format_text_error() {
    let source = "line1\nsecond";
    let span = Span { start: 6, end: 12 }; // 's' of 'second' -> line 2, col 1
    let mut diag = Diagnostic::new("DCL001", Severity::Error, "test message", "file.dart", span);
    diag.resolve_position(source);
    let text = diag.format_text();

    assert!(text.contains("file.dart"));
    assert!(text.contains("DCL001"));
    assert!(text.contains("error"));
    assert!(text.contains("test message"));
    // 1-based line:col of the span start, not the raw byte range.
    assert!(text.contains("file.dart:2:1:"), "got: {text}");
    assert!(!text.contains("+"), "must not print raw byte offsets: {text}");
}

#[test]
fn test_byte_to_line_col_is_one_based() {
    let source = "void main() {\n  print(\"hi\"\n}\n";
    // offset 0 -> line 1, col 1
    assert_eq!(byte_to_line_col(source, 0), (1, 1));
    // offset 5 ('m' of main) -> line 1, col 6
    assert_eq!(byte_to_line_col(source, 5), (1, 6));
    // offset of the '}' on line 3 -> line 3, col 1
    let brace = source.rfind('}').unwrap();
    assert_eq!(byte_to_line_col(source, brace), (3, 1));
}

#[test]
fn test_resolve_position_sets_line_col() {
    let source = "a\nbc\ndef";
    let span = Span { start: 5, end: 6 }; // 'd' on line 3, col 1
    let mut diag = Diagnostic::new("DCL001", Severity::Warning, "m", "f.dart", span);
    assert_eq!((diag.line, diag.col), (0, 0), "unresolved before call");
    diag.resolve_position(source);
    assert_eq!((diag.line, diag.col), (3, 1));
}

#[test]
fn test_format_text_warning() {
    let span = Span { start: 0, end: 5 };
    let diag = Diagnostic::new(
        "WRN001",
        Severity::Warning,
        "warning msg",
        "other.dart",
        span,
    );
    let text = diag.format_text();

    assert!(text.contains("warning"));
}

#[test]
fn test_format_json_has_rule() {
    let span = Span { start: 0, end: 5 };
    let diag = Diagnostic::new("DCL001", Severity::Error, "test", "file.dart", span);
    let json = diag.format_json();

    assert!(json.get("rule").is_some());
    assert_eq!(json.get("rule").unwrap().as_str(), Some("DCL001"));
}

#[test]
fn test_format_json_has_severity() {
    let span = Span { start: 0, end: 5 };
    let diag = Diagnostic::new("DCL001", Severity::Warning, "test", "file.dart", span);
    let json = diag.format_json();

    assert!(json.get("severity").is_some());
}

#[test]
fn test_format_json_carries_resolved_line_col() {
    let source = "a\nbc\ndef";
    let span = Span { start: 5, end: 6 }; // 'd' on line 3, col 1
    let mut diag = Diagnostic::new("DCL001", Severity::Error, "m", "f.dart", span);
    diag.resolve_position(source);
    let json = diag.format_json();

    // Byte span stays alongside the navigable line/col.
    assert_eq!(json.get("span").unwrap().get("start").unwrap().as_u64(), Some(5));
    assert_eq!(json.get("line").unwrap().as_u64(), Some(3));
    assert_eq!(json.get("col").unwrap().as_u64(), Some(1));
}

#[test]
fn test_format_lsp_severity_error() {
    let source = "void main() {}";
    let span = Span { start: 0, end: 4 };
    let diag = Diagnostic::new("DCL001", Severity::Error, "test", "file.dart", span);
    let lsp_diag = diag.format_lsp(source);

    assert_eq!(
        lsp_diag.severity,
        Some(lsp_types::DiagnosticSeverity::ERROR)
    );
}

#[test]
fn test_format_lsp_severity_warning() {
    let source = "void main() {}";
    let span = Span { start: 0, end: 4 };
    let diag = Diagnostic::new("WRN001", Severity::Warning, "test", "file.dart", span);
    let lsp_diag = diag.format_lsp(source);

    assert_eq!(
        lsp_diag.severity,
        Some(lsp_types::DiagnosticSeverity::WARNING)
    );
}

#[test]
fn test_format_lsp_code() {
    let source = "void main() {}";
    let span = Span { start: 0, end: 4 };
    let diag = Diagnostic::new("DCL001", Severity::Error, "test", "file.dart", span)
        .with_code("CUSTOM002");
    let lsp_diag = diag.format_lsp(source);

    match &lsp_diag.code {
        Some(lsp_types::NumberOrString::String(code)) => {
            assert_eq!(code, "CUSTOM002");
        }
        _ => panic!("Expected String code"),
    }
}

#[test]
fn test_format_lsp_line_col_first_line() {
    let source = "void main() {}";
    let span = Span { start: 5, end: 9 }; // 'main'
    let diag = Diagnostic::new("DCL001", Severity::Warning, "test", "file.dart", span);
    let lsp_diag = diag.format_lsp(source);

    assert_eq!(lsp_diag.range.start.line, 0);
    assert_eq!(lsp_diag.range.start.character, 5);
}

#[test]
fn test_format_lsp_line_col_second_line() {
    let source = "line1\nline2\nline3";
    let span = Span { start: 6, end: 11 }; // 'line2'
    let diag = Diagnostic::new("DCL001", Severity::Warning, "test", "file.dart", span);
    let lsp_diag = diag.format_lsp(source);

    assert_eq!(lsp_diag.range.start.line, 1);
    assert_eq!(lsp_diag.range.start.character, 0);
    assert_eq!(lsp_diag.range.end.line, 1);
}

#[test]
fn test_format_lsp_note_as_information() {
    let source = "void main() {}";
    let span = Span { start: 0, end: 4 };
    let diag = Diagnostic::new("DCL001", Severity::Note, "note msg", "file.dart", span);
    let lsp_diag = diag.format_lsp(source);

    assert_eq!(
        lsp_diag.severity,
        Some(lsp_types::DiagnosticSeverity::INFORMATION)
    );
}

#[test]
fn test_severity_display() {
    assert_eq!(format!("{}", Severity::Error), "error");
    assert_eq!(format!("{}", Severity::Warning), "warning");
    assert_eq!(format!("{}", Severity::Info), "info");
    assert_eq!(format!("{}", Severity::Note), "note");
}

#[test]
fn test_byte_to_lsp_position_start() {
    let source = "line1\nline2\nline3";
    let pos = byte_to_lsp_position(source, 0);
    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_byte_to_lsp_position_second_line() {
    let source = "line1\nline2\nline3";
    let pos = byte_to_lsp_position(source, 6); // 'l' of 'line2'
    assert_eq!(pos.line, 1);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_byte_to_lsp_position_mid_line() {
    let source = "void main() {}";
    let pos = byte_to_lsp_position(source, 5); // 'm' of 'main'
    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, 5);
}

#[test]
fn test_byte_to_lsp_position_clamped_to_end() {
    let source = "abc";
    let pos = byte_to_lsp_position(source, 999);
    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, 3);
}
