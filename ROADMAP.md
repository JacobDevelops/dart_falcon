# Roadmap

falcon is pre-1.0. This tracks shipped milestones and what's left, not a release commitment. Items and order will shift.

## Toward 1.0

1.0 is a rule-catalog and readiness push. [`docs/v1-checklist.md`](docs/v1-checklist.md) is the checklist of engine work and planned rules that must be resolved before 1.0. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` defines the shipped registry and default enablement.

### Shipped

All 142 official lints from the 1.0 catalog are registered. Against `dart analyze` 3.13.5, they match on 34 corpora (421,953 agreements) and 9 additional projects: dio, bloc, riverpod, flame, drift, freezed, get_it, mobx, and jfit (206,914 agreements). Both sets have zero false positives, false negatives, or range mismatches, including UTF-16 columns and diagnostic lengths. `package_api_docs`, `package_prefixed_library_names`, and `unsafe_html` are no longer shipped because Dart 3.13.5 no longer implements them; existing config entries produce unknown-rule warnings. `implicit_reopen` deliberately differs from the analyzer's declaration-order bug.

All 142 have `recommended: false` in falcon, including the 25 upstream preset members. Enable them explicitly or migrate rules listed directly under `linter.rules` in an analyzer config. Migration doesn't expand `include:` presets.

### Remaining

- **Engine checklist.** Per-package-context library identity, thread-spawn failure handling, and a concrete syntax tree; see [`docs/v1-checklist.md`](docs/v1-checklist.md#engine).
- **Rule-catalog expansion.** Two sources remain in the checklist:
  - **DCM-inspired rules: 231** (75 must-have, 132 nice-to-have, 24 post-1.0).
    These are candidates from the DCM survey, not current pricing claims.
  - **Cross-file rules: 11** (see below).
- **New domains.** Alongside `flutter`, add `test`, `bloc`, `riverpod`,
  `provider`, `flutter_hooks`, and `equatable` domains (from the DCM survey),
  activated per file/framework so framework users get targeted checks.
- **Cross-file rules.** Seven ship today: `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, and `sort-pub-dependencies`. The 11 planned additions are Tier 1 must-haves `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles`, `banned-imports`; Tier 2 `unused-assets`, `unused-l10n`, `unexported-public-api`; and four Tier 3 post-1.0 rules.
- **e2e OSS corpus.** Add a `cargo xtask e2e` harness for pinned, shallow clones of large Dart/Flutter repos, including immich, AppFlowy, flutter/packages, flame, LocalSend, bloc, riverpod, Wonderous, flutter/samples, and dart-lang/sdk `tests/language`. It should catch panics and parser/internal errors, snapshot diagnostic counts, and smoke-test migration of common `analysis_options.yaml` presets. Run a fast subset in PR CI and the full set nightly.
- **Fix `prefer-iterable-every` porting bug.** Our port detects the *negated*
  `!where(pred).isEmpty` (which is `any(pred)` semantics, overlapping
  `prefer-iterable-any`) and suggests `.every()` with the wrong predicate semantics.
  Match the non-negated `where(pred).isEmpty` instead, which corresponds to
  `every(!pred)`, and update the corpus. Keep the valid
  `where(pred).length == length` case. Make this behavior change separately.
- **CHANGELOG.** A maintained changelog, starting at the 1.0 release.

## Post-1.0

- **Niche-package DCM rules + deep analyses.** The flame / patrol / mocktail /
  get_it / fake_async rule sets, plus the DCM rules the catalog marks post-1.0
  (config-schema-heavy meta-rules, whole-program-race analyses like
  `require-atomic-async-updates`, and single-type-per-file style rules).
- **Cross-file Tier 3.** `missing-assets`, `unused-dev-dependencies`,
  `no-deep-package-imports`, `duplicate-barrel-exports`.
