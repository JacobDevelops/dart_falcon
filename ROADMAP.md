# Roadmap

falcon is pre-1.0. This is a direction sketch, not a commitment — items and order
will shift. Grouped roughly from near-term to exploratory.

## Near-term

Most of the original near-term batch has shipped — `falcon migrate`, a published
`falcon.json` JSON schema, per-path rule options in `overrides`, project rules in
the LSP, and release-automation polish (tag/`Cargo.toml` version guard, flake
version derived from `Cargo.toml`). A maintained CHANGELOG is deferred to the 1.0
release. Next up is the Toward 1.0 work below.

## Toward 1.0

- **Rule id normalization + twin-rule unification.** Ids are a kebab/snake mix
  inherited from the upstream linters, with some duplicated rules (e.g.
  `no-empty-block` / `no_empty_block`). Normalize and unify — breaking, so targeted
  at 1.0, shipped alongside `falcon migrate` so existing configs can be rewritten.
- **Flutter domain buildout.** Treat domains as rule presets: implement the missing
  core Dart rules currently covered only by `flutter analyze`, and have the
  `flutter` domain include every Flutter-relevant rule.
- **Type-resolution-dependent rules.** `no-boolean-literal-compare`,
  `avoid-ignoring-return-values`, and `unnecessary-nullable` are conservative and
  off-by-default without a type resolver. A resolver would let them run reliably.
- **Dart language tracking.** Keep pace with new syntax beyond Dart 3.4 (null-aware
  elements etc. already done); track the language as it evolves.
- **Fix `prefer-iterable-every` porting bug.** Our port detects the *negated*
  `!where(pred).isEmpty` (which is `any(pred)` semantics, overlapping
  `prefer-iterable-any`) and suggests `.every()` without inverting the predicate.
  Upstream pyramid_lint matches the *non-negated* `where(pred).isEmpty` → `every(!pred)`.
  Change detection to bare `where(pred).isEmpty`, invert the predicate in the fix,
  and rewrite the corpus accordingly (keep the valid `where(pred).length == length`
  case). Behavior change — do it as its own PR.

## Exploratory

- **DCM (Dart Code Metrics) rule integration.** DCM's current special rules are
  paywalled and numerous; reimplement the useful ones as open falcon rules.
- **Automatic documentation generation.** Generate rule docs from rule metadata and
  rustdoc, biome-style. Explicitly deferred from the current docs pass.
