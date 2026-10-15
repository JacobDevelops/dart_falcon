# Roadmap

falcon is pre-1.0. This tracks shipped milestones and what's left, not a release commitment. Items and order will shift.

## Toward 1.0

1.0 is a rule-catalog and readiness push. [`docs/rule-catalog-1.0.md`](docs/rule-catalog-1.0.md) records the shipped official lints and planned additions. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` defines the shipped registry and default enablement.

### Shipped

All 145 section-1 official lints are registered. Against `dart analyze` 3.13.5 on 34 corpora, they recorded 421,953 agreements, 0 false negatives, and 36 false positives. All 36 are `package_prefixed_library_names`, which upstream no longer enforces. `implicit_reopen` deliberately differs from the analyzer's declaration-order bug.

All 145 have `recommended: false` in falcon, including the 25 upstream preset members. Enable them explicitly or migrate an analyzer config; upstream preset membership isn't falcon default enablement.

### Remaining

- **Rule-catalog expansion.** Two sources remain in the catalog:
  - **DCM-inspired rules — 231** (75 must-have, 132 nice-to-have, 24 post-1.0).
    DCM's modern catalog is paywalled; these reimplement the high-value ones as
    open falcon rules.
  - **Cross-file rules — 11** (see below).
- **New domains.** Alongside `flutter`, add `test`, `bloc`, `riverpod`,
  `provider`, `flutter_hooks`, and `equatable` domains (from the DCM survey),
  activated per file/framework so framework users get targeted checks.
- **Cross-file rules.** Eight ship today: `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, `package-api-docs`, and `sort-pub-dependencies`. The 11 planned additions are Tier 1 must-haves `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles`, `banned-imports`; Tier 2 `unused-assets`, `unused-l10n`, `unexported-public-api`; and four Tier 3 post-1.0 rules.
- **e2e OSS corpus.** A `cargo xtask e2e` harness running falcon against pinned,
  shallow clones of large real-world Dart/Flutter repos (immich, AppFlowy,
  flutter/packages, flame, LocalSend, bloc, riverpod, Wonderous, flutter/samples,
  dart-lang/sdk `tests/language`). Gates: zero panics, zero parser/internal-error
  diagnostics, `insta` snapshots of diagnostic counts, and `migrate` smoke tests
  over vendored `analysis_options.yaml` fixtures (flutter_lints,
  very_good_analysis, DCM, custom_lint, strict modes). PR CI runs a fast subset;
  the full set runs nightly.
- **Generated rule docs.** The docs site is live; rule documentation still needs
  to be generated from rule metadata and rustdoc rather than hand-written.
- **Fix `prefer-iterable-every` porting bug.** Our port detects the *negated*
  `!where(pred).isEmpty` (which is `any(pred)` semantics, overlapping
  `prefer-iterable-any`) and suggests `.every()` without inverting the predicate.
  Upstream pyramid_lint matches the *non-negated* `where(pred).isEmpty` →
  `every(!pred)`. Change detection to bare `where(pred).isEmpty`, invert the
  predicate in the fix, and rewrite the corpus accordingly (keep the valid
  `where(pred).length == length` case). Behavior change — do it as its own PR.
- **CHANGELOG.** A maintained changelog, starting at the 1.0 release.

## Post-1.0

- **Niche-package DCM rules + deep analyses.** The flame / patrol / mocktail /
  get_it / fake_async rule sets, plus the DCM rules the catalog marks post-1.0
  (config-schema-heavy meta-rules, whole-program-race analyses like
  `require-atomic-async-updates`, and single-type-per-file style rules).
- **Cross-file Tier 3.** `missing-assets`, `unused-dev-dependencies`,
  `no-deep-package-imports`, `duplicate-barrel-exports`.
