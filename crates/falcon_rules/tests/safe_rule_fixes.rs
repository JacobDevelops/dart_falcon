use std::path::Path;
use std::sync::Arc;

use falcon_analyze::{AnalyzeContext, ParsedFile, Rule, RuleAnalysis};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_diagnostics::{Diagnostic, FixKind};
use falcon_rules::lint::style::prefer_double_quotes::PreferDoubleQuotes;
use falcon_rules::lint::style::prefer_single_quotes::PreferSingleQuotes;
use falcon_rules::lint::style::unnecessary_new::UnnecessaryNew;

fn diagnostics(name: &'static str, rule: &dyn Rule, source: &str) -> Vec<Diagnostic> {
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree: result.tree,
    });
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(Path::new("fix.dart"), &config, &parsed);
    rule.analyze(name, parsed.program(), &context)
}

fn fixed(name: &'static str, rule: &dyn Rule, source: &str) -> String {
    let reports = diagnostics(name, rule, source);
    assert_eq!(reports.len(), 1, "{reports:?}");
    let fix = reports[0].fix.as_ref().expect("safe rule fix");
    assert_eq!(fix.kind, FixKind::Safe);
    fix.apply(source).expect("valid UTF-8 edits")
}

#[test]
fn unnecessary_new_removes_only_keyword_and_following_whitespace() {
    for (source, expected) in [
        ("var value = new Foo();", "var value = Foo();"),
        ("var value = new \t\r\n Foo();", "var value = Foo();"),
        (
            "var value = new /* keep */ Foo();",
            "var value = /* keep */ Foo();",
        ),
        (
            "var value = new// keep\nFoo();",
            "var value = // keep\nFoo();",
        ),
        (
            "var text = 'é🙂'; var value = new Foo();",
            "var text = 'é🙂'; var value = Foo();",
        ),
    ] {
        let reports = diagnostics("unnecessary-new", &UnnecessaryNew, source);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].message, "Remove the optional `new` keyword.");
        assert!(source[reports[0].span.start..reports[0].span.end].starts_with("new"));
        assert_eq!(fixed("unnecessary-new", &UnnecessaryNew, source), expected);
    }
    for source in ["var value = const Foo();", "var value = Foo();"] {
        assert!(diagnostics("unnecessary-new", &UnnecessaryNew, source).is_empty());
    }
}

#[test]
fn quote_fixes_preserve_raw_triple_interpolation_and_escape_spelling() {
    for (source, expected) in [
        (r#"var text = "é🙂";"#, "var text = 'é🙂';"),
        (r#"var text = r"é\n";"#, r"var text = r'é\n';"),
        (
            r#"var text = """é
🙂""";"#,
            "var text = '''é\n🙂''';",
        ),
        (r#"var text = r"""é\n""";"#, r"var text = r'''é\n''';"),
        (
            r#"var text = "é $name ${1 + 2}";"#,
            "var text = 'é $name ${1 + 2}';",
        ),
        (
            r#"var text = "escaped \" quote";"#,
            r#"var text = 'escaped \" quote';"#,
        ),
    ] {
        let reports = diagnostics("prefer-single-quotes", &PreferSingleQuotes, source);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].message, "Unnecessary use of double quotes.");
        assert_eq!(reports[0].fix.as_ref().unwrap().edits().len(), 2);
        assert_eq!(
            fixed("prefer-single-quotes", &PreferSingleQuotes, source),
            expected
        );
    }
}

#[test]
fn double_quote_fixes_preserve_literal_forms() {
    for (source, expected) in [
        ("var text = 'é🙂';", r#"var text = "é🙂";"#),
        (r"var text = r'é\n';", r#"var text = r"é\n";"#),
        ("var text = '''é\n🙂''';", "var text = \"\"\"é\n🙂\"\"\";"),
        (r"var text = r'''é\n''';", r#"var text = r"""é\n""";"#),
        (
            "var text = 'é $name ${1 + 2}';",
            r#"var text = "é $name ${1 + 2}";"#,
        ),
        (
            r"var text = 'escaped \' quote';",
            r#"var text = "escaped \' quote";"#,
        ),
    ] {
        let reports = diagnostics("prefer-double-quotes", &PreferDoubleQuotes, source);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].message, "Unnecessary use of single quotes.");
        assert_eq!(reports[0].fix.as_ref().unwrap().edits().len(), 2);
        assert_eq!(
            fixed("prefer-double-quotes", &PreferDoubleQuotes, source),
            expected,
        );
    }
}

#[test]
fn quote_fixes_preserve_existing_exclusions() {
    for source in [
        r#"var text = "it's fine";"#,
        r#"var text = "${value ? 'yes' : 'no'}";"#,
        r#"var text = '${"nested"}';"#,
        r#"var text = "\u0027";"#,
    ] {
        assert!(diagnostics("prefer-single-quotes", &PreferSingleQuotes, source).is_empty());
    }
    for source in [
        r#"var text = 'contains " quote';"#,
        r#"var text = '${value ? "yes" : "no"}';"#,
        r#"var text = "${'nested'}";"#,
        r"var text = '\u0022';",
    ] {
        assert!(diagnostics("prefer-double-quotes", &PreferDoubleQuotes, source).is_empty());
    }
}

#[test]
fn quote_preferences_form_a_real_two_rule_cycle() {
    let source = r#"var text = "é🙂";"#;
    let single = fixed("prefer-single-quotes", &PreferSingleQuotes, source);
    let double = fixed("prefer-double-quotes", &PreferDoubleQuotes, &single);
    assert_ne!(single, source);
    assert_eq!(double, source);
}

#[test]
fn adjacent_literals_have_independent_delimiter_fixes() {
    let source = r#"var text = "é" r"🙂";"#;
    let reports = diagnostics("prefer-single-quotes", &PreferSingleQuotes, source);
    assert_eq!(reports.len(), 2);
    for report in &reports {
        assert_eq!(report.message, "Unnecessary use of double quotes.");
        let fix = report.fix.as_ref().unwrap();
        assert_eq!(fix.kind, FixKind::Safe);
        let changed = fix.apply(source).unwrap();
        assert!(changed == r#"var text = 'é' r"🙂";"# || changed == r#"var text = "é" r'🙂';"#);
    }
}

#[test]
fn quote_fixes_separate_touching_literal_delimiters() {
    let cases = [
        (r#"var s = ""'x';"#, "var s = '' 'x';"),
        (r#"var s = r""'x';"#, "var s = r'' 'x';"),
        (r#"var s = ""'''x''';"#, "var s = '' '''x''';"),
        (r#"var s = ''"x";"#, "var s = '' 'x';"),
        (r#"var s = ''"";"#, "var s = '' '';"),
        (r#"var s = ''r"";"#, "var s = ''r'';"),
    ];
    for (source, expected) in cases {
        let changed = fixed("prefer-single-quotes", &PreferSingleQuotes, source);
        assert_eq!(changed, expected);
        let parsed = parse(&changed);
        assert!(parsed.errors.is_empty(), "{:?}: {changed}", parsed.errors);
    }
    let cases = [
        (r#"var s = ''"x";"#, r#"var s = "" "x";"#),
        (r#"var s = r''"x";"#, r#"var s = r"" "x";"#),
        (r#"var s = ''"""x""";"#, r#"var s = "" """x""";"#),
        (r#"var s = ""'x';"#, r#"var s = "" "x";"#),
        (r#"var s = ""'';"#, r#"var s = "" "";"#),
        (r#"var s = ""r'';"#, r#"var s = ""r"";"#),
    ];
    for (source, expected) in cases {
        let changed = fixed("prefer-double-quotes", &PreferDoubleQuotes, source);
        assert_eq!(changed, expected);
        let parsed = parse(&changed);
        assert!(parsed.errors.is_empty(), "{:?}: {changed}", parsed.errors);
    }
}
