//! Iterative, single-file fixes using the same analysis pipeline as ordinary checks.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use falcon_analyze::AnalysisEnvironment;
use falcon_config::{FalconConfig, RuleFix};
use falcon_diagnostics::{ApplyFixError, Diagnostic, FixKind, RULES_THREAD_ERROR_DIAGNOSTIC_ID};

use crate::{ProjectAnalysis, analyze_sources, resolve_document_positions};

const MAX_FIX_ITERATIONS: usize = 100;

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
/// Diagnostics always describe the returned source, including on a stop.
pub fn fix_file(
    analysis: &ProjectAnalysis,
    config: &FalconConfig,
    environment: &mut AnalysisEnvironment,
    path: &Path,
    source: &str,
    unsafe_fixes: bool,
) -> FixOutcome {
    fix_with(source, unsafe_fixes, MAX_FIX_ITERATIONS, |source| {
        let files = [(path.to_path_buf(), source.to_owned())];
        let (mut diagnostics, _, _) = analyze_sources(analysis, &files, &[], config, environment);
        falcon_rules::apply_severities(&mut diagnostics, config);
        apply_fix_policies(config, &mut diagnostics);
        diagnostics
    })
}

fn fix_with(
    source: &str,
    unsafe_fixes: bool,
    limit: usize,
    mut analyze: impl FnMut(&str) -> Vec<Diagnostic>,
) -> FixOutcome {
    let mut source = source.to_owned();
    // String keys hash seen text and verify equality, avoiding hash-collision stops.
    let mut seen: HashMap<String, usize> = HashMap::with_capacity(limit);
    let mut applied: Vec<&'static str> = Vec::with_capacity(limit);
    loop {
        let mut diagnostics = analyze(&source);
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
        let mut next = None;
        let mut stop = None;
        for diagnostic in &diagnostics {
            let Some(fix) = &diagnostic.fix else { continue };
            if fix.kind == FixKind::Unsafe && !unsafe_fixes {
                continue;
            }
            let candidate = match fix.apply(&source) {
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
            if let Some(&start) = seen.get(&candidate) {
                let mut rules = applied[start..].to_vec();
                rules.push(diagnostic.rule);
                stop = Some(FixStop::Cycle {
                    rules: unique_rules(rules),
                });
            } else if applied.len() == limit {
                let mut rules = applied.clone();
                rules.push(diagnostic.rule);
                stop = Some(FixStop::IterationLimit {
                    rules: unique_rules(rules),
                });
            } else {
                next = Some((candidate, diagnostic.rule));
            }
            break;
        }
        if let Some((candidate, rule)) = next {
            seen.insert(std::mem::replace(&mut source, candidate), applied.len());
            applied.push(rule);
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
            r#"
linter:
  rules:
    style:
      unnecessary-new: { level: error, fix: none }
cross-file:
  rules:
    security:
      secure-pubspec-urls: { level: error, fix: unsafe }
"#,
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
            r#"
linter:
  rules:
    style:
      unnecessary-new: { level: error, fix: safe }
"#,
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
