# Roadmap

falcon is pre-1.0. This tracks shipped milestones and what's left, not a release commitment. Items and order will shift.

## Toward 1.0

1.0 is a rule-catalog and readiness push. [`docs/v1-checklist.md`](docs/v1-checklist.md) is the checklist of engine work and planned rules that must be resolved before 1.0. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` defines the shipped registry and default enablement.

### Shipped

All 142 official lints from the 1.0 catalog are registered. Against `dart analyze` 3.13.5, they match on 34 corpora (421,953 agreements) and 9 additional projects: dio, bloc, riverpod, flame, drift, freezed, get_it, mobx, and jfit (206,914 agreements). Both sets have zero false positives, false negatives, or range mismatches, including UTF-16 columns and diagnostic lengths. `package_api_docs`, `package_prefixed_library_names`, and `unsafe_html` are no longer shipped because Dart 3.13.5 no longer implements them; existing config entries produce unknown-rule warnings. `implicit_reopen` deliberately differs from the analyzer's declaration-order bug.

Falcon's default set has been re-chosen independently of upstream presets, with 215 of 513 shipped rules recommended. Enable other rules explicitly or migrate rules listed directly under `linter.rules` in an analyzer config. Migration doesn't expand `include:` presets.

Falcon uses an arena concrete syntax tree that retains every token and trivia, so syntax rules read from one tree without parser changes.

- **Engine checklist complete.** Per-package-context library identity and the concrete syntax tree are shipped.
- **Must-have and DCM nice-to-have rules shipped.** All 139 DCM nice-to-have rules have shipped; the 137 new rules and two aliases bring the registry to 513 rules.

### Remaining

- **Rule-catalog expansion.** Every must-have and DCM nice-to-have rule has shipped, and every remaining checklist row is now 1.0 scope:
  - **DCM-inspired rules: 24**, including whole-program-race analyses like `require-atomic-async-updates`, config-schema-heavy rules, and single-type-per-file style rules.
  - **Additional rules: 3.**
- **Cross-file rules.** Eleven ship today: `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, `sort-pub-dependencies`, `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles`, and `banned-imports`. The 8 planned additions are `unused-assets`, `unused-l10n`, `architecture-layer-boundaries`, `unexported-public-api`, `missing-assets`, `unused-dev-dependencies`, `no-deep-package-imports`, and `duplicate-barrel-exports`.
- **Hardening.** The October 2026 audit's engine, tooling and docs fixes plus the 1.0 workflow features (safe autofixes, baselines, CI report formats, git-aware PR review, severity thresholds, install and update, monorepo runs), staged in the checklist.
- **e2e OSS corpus.** Add a `cargo xtask e2e` harness for pinned, shallow clones of large Dart/Flutter repos, including immich, AppFlowy, flutter/packages, flame, LocalSend, bloc, riverpod, Wonderous, flutter/samples, and dart-lang/sdk `tests/language`. It should catch panics and parser/internal errors, snapshot diagnostic counts, and smoke-test migration of common `analysis_options.yaml` presets. Run a fast subset in PR CI and the full set nightly.
- **Fix `prefer-iterable-every` porting bug.** Our port detects the *negated*
  `!where(pred).isEmpty` (which is `any(pred)` semantics, overlapping
  `prefer-iterable-any`) and suggests `.every()` with the wrong predicate semantics.
  Match the non-negated `where(pred).isEmpty` instead, which corresponds to
  `every(!pred)`, and update the corpus. Keep the valid
  `where(pred).length == length` case. Make this behavior change separately.
- **CHANGELOG.** A maintained changelog, starting at the 1.0 release.

## Post-1.0

- **Niche-package rule sets.** The flame / patrol / mocktail / get_it /
  fake_async rule sets.
