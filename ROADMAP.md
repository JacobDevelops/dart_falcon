# Roadmap

falcon is pre-1.0. This is a direction sketch, not a commitment — items and order
will shift. Grouped roughly from near-term to exploratory.

## Near-term

- **`falcon migrate` command.** Biome-style migration (like its eslint/prettier
  migrate): read a project's `analysis_options.yaml` blocks for the linters falcon
  progressively replaces — `dart_code_linter` and `pyramid_lint` — and generate the
  equivalent `falcon.json`.
- **Published JSON schema for `falcon.json`.** Ship a real schema so editors give
  autocomplete and validation instead of pointing `$schema` at a placeholder.
- **Per-path rule options in overrides.** `overrides` currently accept only
  severity / on-off; per-path option blocks are load-rejected today. Allow full
  per-path option configuration.
- **Project rules in the LSP.** Cross-file rules (`unused-files`, `unused-code`,
  `unnecessary-nullable`) run from the CLI only; surface them in `falcon lsp` too.
- **CHANGELOG + release automation polish.** A maintained changelog and tighter
  tag → build → publish → manifest-commit release flow.

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

## Exploratory

- **DCM (Dart Code Metrics) rule integration.** DCM's current special rules are
  paywalled and numerous; reimplement the useful ones as open falcon rules.
- **Automatic documentation generation.** Generate rule docs from rule metadata and
  rustdoc, biome-style. Explicitly deferred from the current docs pass.
