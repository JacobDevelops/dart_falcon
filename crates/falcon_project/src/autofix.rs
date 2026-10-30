//! Iterative, single-file fixes using the same analysis pipeline as ordinary checks.

use std::collections::HashMap;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use falcon_analyze::{AnalysisEnvironment, ExternalIndexParts};
use falcon_config::{FalconConfig, RuleFix};
use falcon_diagnostics::{
    ApplyFixError, Diagnostic, Fix, FixKind, RULES_THREAD_ERROR_DIAGNOSTIC_ID,
    SYNTAX_ERROR_DIAGNOSTIC_ID,
};

use crate::{ProjectAnalysis, autofix_analysis::AutofixAnalyzer, resolve_document_positions};

const MAX_NONCONVERGING_ITERATIONS: usize = 100;

/// The final analyzed source and any reason fixing could not finish.
#[derive(Debug)]
pub struct FixOutcome {
    pub source: String,
    pub diagnostics: Vec<Diagnostic>,
    pub stop: Option<FixStop>,
}

/// A rejected fix or a non-converging sequence. Frontends decide whether to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixStop {
    SyntaxErrors,
    Cancelled,
    InvalidFix {
        rule: &'static str,
        error: ApplyFixError,
    },
    Cycle {
        rules: Vec<&'static str>,
    },
    IterationLimit {
        rules: Vec<&'static str>,
    },
    AnalysisFailure {
        message: String,
        rules: Vec<&'static str>,
    },
}

impl fmt::Display for FixStop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SyntaxErrors => {
                formatter.write_str("autofix skipped because the file has syntax errors")
            }
            Self::Cancelled => formatter.write_str("autofix cancelled"),
            Self::InvalidFix { rule, error } => {
                write!(formatter, "invalid fix from `{rule}`: {error}")
            }
            Self::Cycle { rules } => {
                write!(formatter, "autofix cycle involving {}", rules.join(", "))
            }
            Self::IterationLimit { rules } => write!(
                formatter,
                "autofix iteration limit reached involving {}",
                rules.join(", ")
            ),
            Self::AnalysisFailure { message, rules } => {
                write!(formatter, "autofix analysis failed: {message}")?;
                if !rules.is_empty() {
                    write!(formatter, " after fixes from {}", rules.join(", "))?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FixStop {}

/// Resolve the path-specific fix override using the rule's registered group.
/// Unknown rule IDs retain their declared kind; `none` suppresses the fix.
/// This helper also supports cross-file diagnostic quick fixes, although the
/// iterative file engine never runs or applies cross-file rules.
#[must_use]
pub fn effective_fix_kind(
    config: &FalconConfig,
    path: &Path,
    rule: &str,
    declared: FixKind,
) -> Option<FixKind> {
    let Some(metadata) = falcon_rules::meta::meta_for(rule) else {
        return Some(declared);
    };
    let path = path.to_string_lossy();
    let policy = if metadata.is_cross_file() {
        config.cross_file_fix_for(&path, metadata.group.as_str(), metadata.name)
    } else {
        config.rule_fix_for(&path, metadata.group.as_str(), metadata.name)
    };
    match policy {
        Some(RuleFix::None) => None,
        Some(RuleFix::Safe) => Some(FixKind::Safe),
        Some(RuleFix::Unsafe) => Some(FixKind::Unsafe),
        None => Some(declared),
    }
}

/// Suppress or reclassify fix payloads for normal diagnostics and quick fixes.
pub fn apply_fix_policies(config: &FalconConfig, diagnostics: &mut [Diagnostic]) {
    for diagnostic in diagnostics {
        if let Some(fix) = &mut diagnostic.fix {
            match effective_fix_kind(
                config,
                Path::new(&diagnostic.file_path),
                diagnostic.rule,
                fix.kind,
            ) {
                Some(kind) => fix.kind = kind,
                None => diagnostic.fix = None,
            }
        }
    }
}

/// Apply one source-order fix at a time, reanalyzing after each change.
///
/// The caller's environment supplies package, SDK and import context to the
/// shared parser and resolving analysis pipeline on every iteration. Cross-file
/// findings must be collected by the caller's final project-wide analysis.
/// Diagnostics describe the returned source, except when cancellation avoids analysis.
pub fn fix_file(
    analysis: &ProjectAnalysis,
    config: &FalconConfig,
    environment: &mut AnalysisEnvironment,
    path: &Path,
    source: &str,
    unsafe_fixes: bool,
) -> FixOutcome {
    fix_file_with_context(
        analysis,
        config,
        environment,
        path,
        source,
        unsafe_fixes,
        FixContext {
            external: None,
            external_libraries: None,
            project_files: &[],
            cancelled: &|| false,
        },
    )
}

/// Cached semantic dependencies and cancellation supplied by an editor.
#[derive(Clone, Copy)]
pub struct FixContext<'a> {
    pub external: Option<ExternalIndexParts<'a>>,
    pub external_libraries: Option<&'a falcon_analyze::ExternalLibraries>,
    pub project_files: &'a [falcon_analyze::ProjectFile],
    pub cancelled: &'a dyn Fn() -> bool,
}

/// Fix a document using cached external indexes and cooperative cancellation.
pub fn fix_file_with_context(
    analysis: &ProjectAnalysis,
    config: &FalconConfig,
    environment: &mut AnalysisEnvironment,
    path: &Path,
    source: &str,
    unsafe_fixes: bool,
    context: FixContext<'_>,
) -> FixOutcome {
    let mut analyzer = AutofixAnalyzer::new(analysis, config, environment, path, context);
    fix_with_cancellation(
        source,
        unsafe_fixes,
        MAX_NONCONVERGING_ITERATIONS,
        context.cancelled,
        |source| analyzer.analyze(source),
    )
}

#[cfg(test)]
fn fix_with(
    source: &str,
    unsafe_fixes: bool,
    limit: usize,
    mut analyze: impl FnMut(&str) -> Vec<Diagnostic>,
) -> FixOutcome {
    fix_with_cancellation(source, unsafe_fixes, limit, &|| false, &mut analyze)
}

fn fix_with_cancellation(
    original: &str,
    unsafe_fixes: bool,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    mut analyze: impl FnMut(&str) -> Vec<Diagnostic>,
) -> FixOutcome {
    let mut source = original.to_owned();
    let mut seen: HashMap<(u64, usize), Vec<usize>> = HashMap::with_capacity(limit);
    let mut applied: Vec<&'static str> = Vec::with_capacity(limit);
    let mut edits: Vec<Fix> = Vec::with_capacity(limit);
    let mut best_fixable = None;
    let mut nonconverging = 0;
    let mut source_fingerprint = fingerprint(&source);
    loop {
        if cancelled() {
            return FixOutcome {
                source,
                diagnostics: Vec::new(),
                stop: Some(FixStop::Cancelled),
            };
        }
        let mut diagnostics = analyze(&source);
        if let Some(stop) = syntax_analysis_stop(&diagnostics, cancelled, &applied) {
            resolve_document_positions(&mut diagnostics, &source);
            return FixOutcome {
                source,
                diagnostics,
                stop: Some(stop),
            };
        }
        let fixable = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .fix
                    .as_ref()
                    .is_some_and(|fix| fix.kind == FixKind::Safe || unsafe_fixes)
            })
            .count();
        if best_fixable.is_some_and(|best| fixable >= best) {
            nonconverging += 1;
        } else {
            nonconverging = 0;
            best_fixable = Some(fixable);
        }
        if let Some(failure) = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == RULES_THREAD_ERROR_DIAGNOSTIC_ID)
        {
            let stop = Some(FixStop::AnalysisFailure {
                message: failure.message.clone(),
                rules: unique_rules(applied),
            });
            resolve_document_positions(&mut diagnostics, &source);
            return FixOutcome {
                source,
                diagnostics,
                stop,
            };
        }
        diagnostics.sort_by_key(|diagnostic| {
            (diagnostic.span.start, diagnostic.rule, diagnostic.span.end)
        });
        let (next, stop) = select_next_fix(
            &diagnostics,
            &source,
            unsafe_fixes,
            &FixHistory {
                original,
                seen: &seen,
                applied: &applied,
                edits: &edits,
            },
            nonconverging,
            limit,
        );
        if let Some(PendingFix {
            candidate,
            candidate_fingerprint,
            rule,
            fix,
        }) = next
        {
            seen.entry(source_fingerprint)
                .or_default()
                .push(applied.len());
            source = candidate;
            source_fingerprint = candidate_fingerprint;
            applied.push(rule);
            edits.push(fix);
        } else {
            resolve_document_positions(&mut diagnostics, &source);
            return FixOutcome {
                source,
                diagnostics,
                stop,
            };
        }
    }
}

fn syntax_analysis_stop(
    diagnostics: &[Diagnostic],
    cancelled: &dyn Fn() -> bool,
    applied: &[&'static str],
) -> Option<FixStop> {
    if cancelled()
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == SYNTAX_ERROR_DIAGNOSTIC_ID)
    {
        Some(if cancelled() {
            FixStop::Cancelled
        } else if applied.is_empty() {
            FixStop::SyntaxErrors
        } else {
            FixStop::AnalysisFailure {
                message: "a fix produced invalid source".to_owned(),
                rules: vec![*applied.last().expect("a fix was applied")],
            }
        })
    } else {
        None
    }
}

struct FixHistory<'a> {
    original: &'a str,
    seen: &'a HashMap<(u64, usize), Vec<usize>>,
    applied: &'a [&'static str],
    edits: &'a [Fix],
}

struct PendingFix {
    candidate: String,
    candidate_fingerprint: (u64, usize),
    rule: &'static str,
    fix: Fix,
}

fn select_next_fix(
    diagnostics: &[Diagnostic],
    source: &str,
    unsafe_fixes: bool,
    history: &FixHistory<'_>,
    nonconverging: usize,
    limit: usize,
) -> (Option<PendingFix>, Option<FixStop>) {
    let mut next = None;
    let mut stop = None;
    for diagnostic in diagnostics {
        let Some(fix) = &diagnostic.fix else { continue };
        if fix.kind == FixKind::Unsafe && !unsafe_fixes {
            continue;
        }
        let candidate = match fix.apply(source) {
            Ok(candidate) => candidate,
            Err(error) => {
                stop = Some(FixStop::InvalidFix {
                    rule: diagnostic.rule,
                    error,
                });
                break;
            }
        };
        if candidate == source {
            continue;
        }
        let candidate_fingerprint = fingerprint(&candidate);
        let repeated = history.seen.get(&candidate_fingerprint).and_then(|states| {
            states.iter().copied().find(|&state| {
                let mut previous = history.original.to_owned();
                for edit in &history.edits[..state] {
                    previous = edit.apply(&previous).expect("previously applied fix");
                }
                previous == candidate
            })
        });
        if let Some(start) = repeated {
            let mut rules = history.applied[start..].to_vec();
            rules.push(diagnostic.rule);
            stop = Some(FixStop::Cycle {
                rules: unique_rules(rules),
            });
        } else if nonconverging >= limit {
            let mut rules = history.applied.to_vec();
            rules.push(diagnostic.rule);
            stop = Some(FixStop::IterationLimit {
                rules: unique_rules(rules),
            });
        } else {
            next = Some(PendingFix {
                candidate,
                candidate_fingerprint,
                rule: diagnostic.rule,
                fix: fix.clone(),
            });
        }
        break;
    }
    (next, stop)
}

fn fingerprint(source: &str) -> (u64, usize) {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    (hasher.finish(), source.len())
}

fn unique_rules(rules: Vec<&'static str>) -> Vec<&'static str> {
    let mut unique = Vec::with_capacity(rules.len());
    for rule in rules {
        if !unique.contains(&rule) {
            unique.push(rule);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_diagnostics::{Fix, Severity, Span, TextEdit};

    #[test]
    fn many_shrinking_fixes_converge_beyond_the_iteration_limit() {
        let outcome = fix_with(&"x".repeat(101), false, 3, |source| {
            (0..source.len())
                .map(|start| diagnostic("shrinking", start, start + 1, ""))
                .collect()
        });
        assert_eq!(outcome.source, "");
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn syntax_errors_prevent_any_fixes() {
        let outcome = fix_with("broken", false, 3, |_| {
            vec![
                diagnostic("must-not-apply", 0, 6, "changed"),
                Diagnostic::new(
                    SYNTAX_ERROR_DIAGNOSTIC_ID,
                    Severity::Error,
                    "Invalid syntax.",
                    "test.dart",
                    Span { start: 0, end: 0 },
                ),
            ]
        });
        assert_eq!(outcome.source, "broken");
        assert_eq!(outcome.stop, Some(FixStop::SyntaxErrors));
    }

    #[test]
    fn cancellation_stops_before_reanalysis() {
        let calls = std::cell::Cell::new(0);
        let cancelled = std::cell::Cell::new(false);
        let outcome = fix_with_cancellation("a", false, 3, &|| cancelled.get(), |_| {
            calls.set(calls.get() + 1);
            cancelled.set(true);
            vec![diagnostic("must-not-apply", 0, 1, "b")]
        });
        assert_eq!(calls.get(), 1);
        assert_eq!(outcome.source, "a");
        assert_eq!(outcome.stop, Some(FixStop::Cancelled));
    }

    #[test]
    fn oscillating_diagnostic_counts_do_not_reset_the_growth_limit() {
        let outcome = fix_with("", false, 3, |source| {
            let count = if source.len().is_multiple_of(2) { 2 } else { 1 };
            (0..count)
                .map(|_| diagnostic("growing", source.len(), source.len(), "x"))
                .collect()
        });
        assert_eq!(outcome.source, "xxxx");
        assert!(matches!(outcome.stop, Some(FixStop::IterationLimit { .. })));
    }

    #[test]
    fn growing_fix_stops_without_spending_the_initial_diagnostic_count() {
        let outcome = fix_with("", false, 3, |source| {
            (0..200)
                .map(|_| diagnostic("growing", source.len(), source.len(), "x"))
                .collect()
        });
        assert_eq!(outcome.source, "xxx");
        assert!(matches!(outcome.stop, Some(FixStop::IterationLimit { .. })));
    }

    #[test]
    fn fix_producing_syntax_errors_names_the_responsible_rule() {
        let outcome = fix_with("a", false, 3, |source| {
            if source == "a" {
                vec![diagnostic("prefix", 0, 1, "b")]
            } else if source == "b" {
                vec![diagnostic("broken-fix", 0, 1, "c")]
            } else {
                vec![Diagnostic::new(
                    SYNTAX_ERROR_DIAGNOSTIC_ID,
                    Severity::Error,
                    "Invalid syntax.",
                    "test.dart",
                    Span { start: 0, end: 0 },
                )]
            }
        });
        assert!(
            matches!(outcome.stop, Some(FixStop::AnalysisFailure { rules, .. }) if rules == ["broken-fix"])
        );
    }

    #[test]
    fn cancellation_after_an_edit_avoids_the_next_analysis() {
        let checks = std::cell::Cell::new(0);
        let analyses = std::cell::Cell::new(0);
        let outcome = fix_with_cancellation(
            "a",
            false,
            3,
            &|| {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            },
            |_| {
                analyses.set(analyses.get() + 1);
                vec![diagnostic("first", 0, 1, "b")]
            },
        );
        assert_eq!(analyses.get(), 1);
        assert_eq!(outcome.source, "b");
        assert_eq!(outcome.stop, Some(FixStop::Cancelled));
    }

    fn diagnostic(rule: &'static str, start: usize, end: usize, text: &str) -> Diagnostic {
        Diagnostic::new(
            rule,
            Severity::Warning,
            "Fix this.",
            "test.dart",
            Span { start, end },
        )
        .with_fix(
            Fix::new(
                "Replace",
                FixKind::Safe,
                vec![TextEdit::new(Span { start, end }, text)],
            )
            .unwrap(),
        )
    }

    #[test]
    fn analysis_failure_after_a_fix_stops_before_any_more_edits() {
        let outcome = fix_with("a", false, 100, |source| {
            if source == "a" {
                vec![diagnostic("first", 0, 1, "b")]
            } else {
                vec![
                    diagnostic("must-not-apply", 0, 1, "c"),
                    Diagnostic::new(
                        RULES_THREAD_ERROR_DIAGNOSTIC_ID,
                        Severity::Error,
                        "rule worker failed",
                        "test.dart",
                        Span { start: 0, end: 0 },
                    ),
                ]
            }
        });
        assert_eq!(outcome.source, "b");
        assert!(matches!(
            outcome.stop,
            Some(FixStop::AnalysisFailure { message, rules })
                if message == "rule worker failed" && rules == ["first"]
        ));
    }

    #[test]
    fn cycle_rules_exclude_the_noncyclic_prefix() {
        let outcome = fix_with("prefix", false, 100, |source| {
            let (rule, replacement) = match source {
                "prefix" => ("prefix-rule", "a"),
                "a" => ("forward", "b"),
                _ => ("backward", "a"),
            };
            vec![diagnostic(rule, 0, source.len(), replacement)]
        });
        assert!(matches!(
            outcome.stop,
            Some(FixStop::Cycle { rules }) if rules == ["forward", "backward"]
        ));
    }

    #[test]
    fn convergence_exactly_at_the_limit_is_success() {
        let outcome = fix_with("", false, 3, |source| {
            if source.len() < 3 {
                vec![diagnostic("growing", source.len(), source.len(), "x")]
            } else {
                Vec::new()
            }
        });
        assert_eq!(outcome.source, "xxx");
        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn iteration_limit_reports_multiple_rules_without_duplicates() {
        let outcome = fix_with("", false, 3, |source| {
            let rule = if source.len() % 2 == 0 { "even" } else { "odd" };
            vec![diagnostic(rule, source.len(), source.len(), "x")]
        });
        assert!(matches!(
            outcome.stop,
            Some(FixStop::IterationLimit { rules }) if rules == ["even", "odd"]
        ));
    }

    #[test]
    fn nonoverlapping_fixes_are_reanalyzed_one_at_a_time() {
        let mut inputs = Vec::new();
        let outcome = fix_with("abc", false, 100, |source| {
            inputs.push(source.to_owned());
            match source {
                "abc" => vec![
                    diagnostic("first", 0, 1, "x"),
                    diagnostic("later", 2, 3, "z"),
                ],
                "xbc" => vec![diagnostic("later", 2, 3, "y")],
                _ => Vec::new(),
            }
        });
        assert_eq!(outcome.source, "xby");
        assert_eq!(inputs, ["abc", "xbc", "xby"]);
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn overlapping_diagnostics_are_reanalyzed_after_one_atomic_fix() {
        let mut inputs = Vec::new();
        let outcome = fix_with("abc", false, 100, |source| {
            inputs.push(source.to_owned());
            if source == "abc" {
                vec![
                    diagnostic("later", 1, 3, "Y"),
                    diagnostic("first", 0, 2, "X"),
                ]
            } else {
                Vec::new()
            }
        });
        assert_eq!(outcome.source, "Xc");
        assert_eq!(inputs, ["abc", "Xc"]);
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn overlapping_rule_fixes_both_apply_with_rederived_ranges() {
        let mut inputs = Vec::with_capacity(3);
        let outcome = fix_with("abc", false, 100, |source| {
            inputs.push(source.to_owned());
            match source {
                "abc" => vec![
                    diagnostic("later", 1, 3, "Y"),
                    diagnostic("first", 0, 2, "X"),
                ],
                "Xc" => vec![diagnostic("later", 0, 2, "Z")],
                _ => Vec::new(),
            }
        });
        assert_eq!(outcome.source, "Z");
        assert_eq!(inputs, ["abc", "Xc", "Z"]);
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn cycles_name_every_participating_rule_without_applying_repeated_text() {
        let outcome = fix_with("a", false, 100, |source| {
            vec![if source == "a" {
                diagnostic("forward", 0, 1, "b")
            } else {
                diagnostic("backward", 0, 1, "a")
            }]
        });
        assert_eq!(outcome.source, "b");
        assert!(
            matches!(outcome.stop, Some(FixStop::Cycle { rules }) if rules == ["forward", "backward"])
        );
        assert_eq!(outcome.diagnostics[0].rule, "backward");
    }

    #[test]
    fn iteration_cap_reports_rules_and_returns_current_diagnostics() {
        let outcome = fix_with("", false, 3, |source| {
            vec![diagnostic("growing", source.len(), source.len(), "x")]
        });
        assert_eq!(outcome.source, "xxx");
        assert_eq!(outcome.diagnostics[0].span.start, 3);
        assert!(
            matches!(outcome.stop, Some(FixStop::IterationLimit { rules }) if rules == ["growing"])
        );
    }

    #[test]
    fn no_op_does_not_prevent_a_later_fix() {
        let outcome = fix_with("ab", false, 100, |source| {
            let mut diagnostics = vec![diagnostic("no-op", 0, 1, "a")];
            if source == "ab" {
                diagnostics.push(diagnostic("change", 1, 2, "c"));
            }
            diagnostics
        });
        assert_eq!(outcome.source, "ac");
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn invalid_utf8_edit_is_reported_without_partial_application() {
        let outcome = fix_with("é", false, 100, |_| vec![diagnostic("invalid", 1, 2, "a")]);
        assert_eq!(outcome.source, "é");
        assert!(matches!(
            outcome.stop,
            Some(FixStop::InvalidFix {
                rule: "invalid",
                ..
            })
        ));
    }

    #[test]
    fn unsafe_fixes_require_opt_in() {
        let analyze = |source: &str| {
            if source == "a" {
                let mut diagnostic = diagnostic("unsafe", 0, 1, "b");
                diagnostic.fix.as_mut().unwrap().kind = FixKind::Unsafe;
                vec![diagnostic]
            } else {
                Vec::new()
            }
        };
        assert_eq!(fix_with("a", false, 100, analyze).source, "a");
        assert_eq!(fix_with("a", true, 100, analyze).source, "b");
    }

    #[test]
    fn fix_payload_policy_uses_registered_group_and_cross_file_section() {
        let config: FalconConfig = serde_yaml::from_str(
            r"
linter:
  rules:
    style:
      unnecessary-new: { level: error, fix: none }
cross-file:
  rules:
    security:
      secure-pubspec-urls: { level: error, fix: unsafe }
",
        )
        .unwrap();
        let mut diagnostics = vec![
            diagnostic("unnecessary-new", 0, 1, ""),
            diagnostic("secure-pubspec-urls", 0, 1, ""),
        ];
        apply_fix_policies(&config, &mut diagnostics);
        assert!(diagnostics[0].fix.is_none());
        assert_eq!(diagnostics[1].fix.as_ref().unwrap().kind, FixKind::Unsafe);
    }

    #[test]
    fn safe_override_reclassifies_an_unsafe_fix() {
        let config: FalconConfig = serde_yaml::from_str(
            r"
linter:
  rules:
    style:
      unnecessary-new: { level: error, fix: safe }
",
        )
        .unwrap();
        assert_eq!(
            effective_fix_kind(
                &config,
                Path::new("test.dart"),
                "unnecessary-new",
                FixKind::Unsafe
            ),
            Some(FixKind::Safe),
        );
    }
}
