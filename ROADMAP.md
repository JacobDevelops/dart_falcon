# Roadmap

falcon is pre-1.0. This is a direction sketch, not a commitment — items and order
will shift. Grouped roughly from near-term to exploratory.

## Near-term

Most of the original near-term batch has shipped — `falcon migrate`, a published
`falcon.json` JSON schema, per-path rule options in `overrides`, project rules in
the LSP, and release-automation polish (tag/`Cargo.toml` version guard, flake
version derived from `Cargo.toml`). A maintained CHANGELOG is deferred to the 1.0
release. The Toward 1.0 work below has now shipped as well.

## Toward 1.0

All four Toward-1.0 items have shipped:

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
  `unnecessary-nullable` project rule. Local type inference and a cross-file
  return-type index removed the false positives that had kept them opt-in, so all
  three are recommended and on by default.
- **Dart language tracking.** The parser now tracks the language through Dart 3.9,
  including dot-shorthand expressions and digit separators.

## Exploratory / post-1.0

- **Fix `prefer-iterable-every` porting bug.** Our port detects the *negated*
  `!where(pred).isEmpty` (which is `any(pred)` semantics, overlapping
  `prefer-iterable-any`) and suggests `.every()` without inverting the predicate.
  Upstream pyramid_lint matches the *non-negated* `where(pred).isEmpty` → `every(!pred)`.
  Change detection to bare `where(pred).isEmpty`, invert the predicate in the fix,
  and rewrite the corpus accordingly (keep the valid `where(pred).length == length`
  case). Behavior change — do it as its own PR.
- **Remaining type-resolution-blocked rules.** Roughly 22 official
  `package:lints` / `package:flutter_lints` rules still require deeper type
  resolution than the current minimal layer provides (full type hierarchies,
  inference across the SDK, etc.). Expanding the resolver to unlock them is a
  post-1.0 effort.
- **DCM (Dart Code Metrics) rule integration.** DCM's current special rules are
  paywalled and numerous; reimplement the useful ones as open falcon rules.
- **Automatic documentation generation.** Generate rule docs from rule metadata and
  rustdoc, biome-style. Explicitly deferred from the current docs pass.
