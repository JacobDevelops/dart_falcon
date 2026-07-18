# Roadmap

falcon is pre-1.0. This is a direction sketch, not a commitment — items and order
will shift. Grouped roughly from near-term to exploratory.

## Near-term

Most of the original near-term batch has shipped — `falcon migrate`, a published
`falcon.json` JSON schema, per-path rule options in `overrides`, project rules in
the LSP, and release-automation polish (tag/`Cargo.toml` version guard, flake
version derived from `Cargo.toml`). A maintained CHANGELOG is deferred to the 1.0
release. The first Toward-1.0 batch (below) has now shipped as well.

## Toward 1.0 — shipped

All four items in the first Toward-1.0 batch have shipped:

- **Rule id normalization + twin-rule unification.** Every rule id is now
  canonical kebab-case, and the duplicated twin rules (`no_empty_block` /
  `avoid_empty_blocks`, `no_magic_number`, `avoid_unused_parameters`) collapsed
  into one canonical rule each. Legacy `snake_case` and twin ids still resolve as
  deprecated aliases, and `falcon migrate` rewrites existing configs (and
  `falcon.json` files) to the canonical ids.
- **Flutter domain buildout.** The official `package:lints` / `package:flutter_lints`
  recommended rules (the class-A set) are implemented, bringing the rule count to
  148. The `flutter` domain is complete for every Flutter-relevant rule those
  presets can express without full type resolution.
- **Type-resolution layer.** A minimal type-resolution layer now backs
  `no-boolean-literal-compare`, `avoid-ignoring-return-values`, and the
  `unnecessary-nullable` cross-file rule. Local type inference and a cross-file
  return-type index removed the false positives that had kept them opt-in, so all
  three are recommended and on by default.
- **Dart language tracking.** The parser now tracks the language through Dart 3.9,
  including dot-shorthand expressions and digit separators.

## Toward 1.0 — the plan

1.0 is a rule-catalog and readiness push. The full planned catalog — every rule,
with falcon group, analysis type, and priority — lives in
[`docs/rule-catalog-1.0.md`](docs/rule-catalog-1.0.md). Baseline today is **148
rules shipped**; the plan adds **387 candidate rules** (104 must-have that block
1.0, 255 nice-to-have stretch, 28 explicitly post-1.0).

- **Rule-catalog expansion.** Three sources feed 1.0:
  - **Remaining official lints — 145.** Every live `dart-lang/linter` rule falcon
    doesn't yet implement: 25 are preset members (`core`/`recommended`/`flutter`)
    and are must-have; 120 are non-preset/Effective-Dart and are nice-to-have.
  - **DCM-inspired rules — 231** (75 must-have, 132 nice-to-have, 24 post-1.0).
    DCM's modern catalog is paywalled; these reimplement the high-value ones as
    open falcon rules.
  - **Cross-file rules — 11** (see below).
  - By analysis type the work skews toward types: of the 25 preset must-haves,
    all but `file_names` need type resolution (~18 full, 3 local-inference, 3
    pubspec-driven). This drives the **resolver-expansion workstream** — growing
    the minimal type-resolution layer (full type hierarchies, SDK-wide inference)
    to unlock the ~18+ full-resolution rules; the 3 pubspec rules
    (`depend_on_referenced_packages`, `secure_pubspec_urls`, `package_names`) can
    ship independently of it.
- **New domains.** Alongside `flutter`, add `test`, `bloc`, `riverpod`,
  `provider`, `flutter_hooks`, and `equatable` domains (from the DCM survey),
  activated per file/framework so framework users get targeted checks.
- **Cross-file rules.** The top-level `project` config section has been renamed to
  **`cross-file`** (breaking, pre-1.0) — shipped. `falcon migrate` rewrites old
  configs; `project` remains a deprecated alias. Three cross-file rules ship today
  (`unused-code`, `unused-files`, `unnecessary-nullable`); 11 more are planned —
  Tier 1 must-haves `unused-dependencies`, `undeclared-dependencies`,
  `no-import-cycles`, `banned-imports`; Tier 2 `unused-assets`, `unused-l10n`,
  `unexported-public-api`; Tier 3 post-1.0.
- **e2e OSS corpus.** A `cargo xtask e2e` harness runs falcon against pinned,
  shallow clones of large real-world Dart/Flutter repos (immich, AppFlowy,
  flutter/packages, flame, LocalSend, bloc, riverpod, Wonderous, flutter/samples,
  dart-lang/sdk `tests/language`). Gates: zero panics, zero parser/internal-error
  diagnostics, `insta` snapshots of diagnostic counts, and `migrate` smoke tests
  over vendored `analysis_options.yaml` fixtures (flutter_lints,
  very_good_analysis, DCM, custom_lint, strict modes). PR CI runs a fast subset;
  the full set runs nightly.
- **Docs website + generated rule docs.** A TanStack Start site under `website/`,
  with biome-style rule documentation generated from rule metadata and rustdoc.
- **Logo / branding.** A falcon logo and brand, plus extension icons for the
  editor integrations.
- **Fix `prefer-iterable-every` porting bug.** Our port detects the *negated*
  `!where(pred).isEmpty` (which is `any(pred)` semantics, overlapping
  `prefer-iterable-any`) and suggests `.every()` without inverting the predicate.
  Upstream pyramid_lint matches the *non-negated* `where(pred).isEmpty` →
  `every(!pred)`. Change detection to bare `where(pred).isEmpty`, invert the
  predicate in the fix, and rewrite the corpus accordingly (keep the valid
  `where(pred).length == length` case). Behavior change — do it as its own PR.

## Post-1.0

- **Experimental official lints.** A handful of upstream rules still marked
  experimental (`implicit_reopen`, `invalid_case_patterns`, `no_default_cases`,
  `unnecessary_null_checks`, `use_late_for_private_fields_and_variables`) wait for
  upstream to stabilize.
- **Niche-package DCM rules + deep analyses.** The flame / patrol / mocktail /
  get_it / fake_async rule sets, plus the DCM rules the catalog marks post-1.0
  (config-schema-heavy meta-rules, whole-program-race analyses like
  `require-atomic-async-updates`, and single-type-per-file style rules).
- **Cross-file Tier 3.** `missing-assets`, `unused-dev-dependencies`,
  `no-deep-package-imports`, `duplicate-barrel-exports`.
