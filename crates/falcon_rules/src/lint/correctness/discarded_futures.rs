//! Don't discard `Future`s returned by calls in synchronous functions.
//!
//! A `Future` created inside a non-`async` function and then dropped can't be
//! awaited, so its errors and completion are lost:
//!
//! ```dart
//! void save() {
//!   writeFile(); // BAD
//! }
//!
//! Future<void> save() async {
//!   await writeFile(); // GOOD
//! }
//! ```
//!
//! Expression statements, cascade sections, and string interpolations whose
//! resolved type is `Future` (or a subtype) or `FutureOr` are reported inside
//! synchronous bodies. Assignments, `Future.delayed(duration, computation)`,
//! `Map.putIfAbsent`, and members annotated `@awaitNotRequired`, including
//! inherited, extension, and local functions, are exempt; local annotations
//! follow the visible declaration rather than a reusable name. Function typedef
//! annotations follow the invoked value's immediate alias through inferred
//! variables, fields, records, and collections, but not explicit `.call` or an
//! unannotated alias of that typedef. Member calls, including null-aware
//! accesses, use the reached receiver branch for annotation lookup, as do
//! bounded type parameters and flow-promoted `FutureOr` values. Unresolved
//! types stay silent. Calls through getters underline the complete callee expression,
//! while method invocations underline only the method name.
//!
//! Adopted from the official `discarded_futures` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;
use crate::lint::support::unused_futures::{UnusedFuturesRule, unused_futures};

#[derive(Default)]
pub struct DiscardedFutures;

impl Rule for DiscardedFutures {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(unused_futures(
            program,
            ctx,
            &UnusedFuturesRule {
                in_async_bodies: false,
                future_message: "Avoid invoking an asynchronous function in a non-`async` function.",
                future_or_message: Some(
                    "Avoid invoking a function returning `FutureOr` in a non-`async` function.",
                ),
            },
        ))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn callable_getters_report_the_callee_but_methods_report_the_name() {
        let source = "import 'dart:async'; class C { Future<void> Function() get callback => () async {}; Future<void> method() async {} } void f(C c) { c.callback(); c.method(); (c.method)(); }";
        let diagnostics = inference::analyze(DiscardedFutures::NAME, &DiscardedFutures, Path::new("future_spans.dart"),
        source,
        &FalconConfig::default(),);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["c.callback", "method", "(c.method)"]);
    }

    #[test]
    fn constructor_calls_report_the_constructor_name_without_arguments_or_new() {
        let source = "import 'dart:async'; void f() { Future.value(); Future<void>.value(); new Future.value(); new Future<void>.value(); }";
        let diagnostics = inference::analyze(DiscardedFutures::NAME, &DiscardedFutures, Path::new("constructor_spans.dart"),
        source,
        &FalconConfig::default(),);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            actual,
            [
                "Future.value",
                "Future<void>.value",
                "Future.value",
                "Future<void>.value"
            ]
        );
    }
}

#[cfg(test)]
mod reviewed_range_tests {
    use crate::{all_rules, inference};
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn both_future_rules_preserve_generic_constructor_and_callable_property_ranges() {
        for (rule_name, marker) in [("discarded-futures", ""), ("unawaited-futures", "async")] {
            let source = format!(
                "import 'dart:async' as p;
                import 'reviewed_future_ranges.dart' as self;
                class C<T> implements p.Future<T> {{ C(); }}
                class Callbacks {{ p.Future<void> Function() get callback => () async {{}}; }}
                extension E on Callbacks {{ p.Future<void> Function() get ext => () async {{}}; }}
                void f(Callbacks c, Callbacks? nullable) {marker} {{
                  C<int>();
                  self.C<int>();
                  c.ext();
                  c..callback();
                  nullable?..callback();
                }}"
            );
            let rules = all_rules();
            let rule = &rules.iter().find(|(name, _)| *name == rule_name).unwrap().1;
            let diagnostics = inference::analyze(rule_name, rule.as_ref(), Path::new("reviewed_future_ranges.dart"),
            &source,
            &FalconConfig::default(),);
            let actual: Vec<_> = diagnostics
                .iter()
                .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
                .collect();
            assert_eq!(
                actual,
                [
                    "C<int>",
                    "self.C<int>",
                    "c.ext",
                    "..callback",
                    "?..callback"
                ],
                "{rule_name}"
            );
        }
    }
}
