# Roadmap

falcon is pre-1.0. This lists what's *not* done yet — a direction sketch, not a
commitment. Items and order will shift.

## Toward 1.0

1.0 is a rule-catalog and readiness push. The full planned catalog — every rule,
with falcon group, analysis type, and priority — lives in
[`docs/rule-catalog-1.0.md`](docs/rule-catalog-1.0.md). The shipped registry is
defined by `RULE_METADATA` in `crates/falcon_rules/src/meta.rs`; the catalog
records candidate rules and their priorities, some of which have since shipped.

- **Rule-catalog expansion.** Two sources remain for 1.0 (all 145 remaining
  official lints have shipped):
  - **DCM-inspired rules — 231** (75 must-have, 132 nice-to-have, 24 post-1.0).
    DCM's modern catalog is paywalled; these reimplement the high-value ones as
    open falcon rules.
  - **Cross-file rules — 11** (see below).
- **New domains.** Alongside `flutter`, add `test`, `bloc`, `riverpod`,
  `provider`, `flutter_hooks`, and `equatable` domains (from the DCM survey),
  activated per file/framework so framework users get targeted checks.
- **Cross-file rules.** Three ship today (`unused-code`, `unused-files`,
  `unnecessary-nullable`); 11 more are planned — Tier 1 must-haves
  `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles`,
  `banned-imports`; Tier 2 `unused-assets`, `unused-l10n`,
  `unexported-public-api`; Tier 3 post-1.0.
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
