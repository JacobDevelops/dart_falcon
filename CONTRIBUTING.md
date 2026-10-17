# Contributing to falcon

This guide covers the dev environment, checks, and how to add a lint rule.

falcon is pre-1.0: rule ids, the rule set, and the config surface are still
moving. If you're planning a large change, open an issue first so we can agree on
the shape before you write it.

## AI assistance notice

AI-assisted contributions are welcome if you understand and verify the result. Run the checks below and confirm the diagnostics are correct. Disclose meaningful AI assistance in your pull request. Don't open a PR you can't explain.

## Dev environment

Install [mise](https://mise.jdx.dev/getting-started.html) and [rustup](https://rustup.rs), then run from the repo root:

```sh
mise install
makers build
```

`mise.toml` pins cargo-make (`makers`), `cargo-nextest`, `cargo-watch`, taplo, and Bun. Activate mise in your shell to put them on `PATH`, or prefix commands with `mise exec --`. Rust stays under rustup: `rust-toolchain.toml` tracks the latest nightly with `rust-src`, `rust-analyzer`, Clippy, rustfmt, and the `wasm32-wasip2` target. rustup installs the declared toolchain on first use. `makers --list-all-steps` lists every task.

The repo is a colocated jj + git repo. **jj (Jujutsu) users are welcome**, but
plain git works fine. Use whichever you prefer.

## The gates

Run these checks before review. CI runs lint, tests, corpus validation, and benchmark compilation; `makers A` also checks the workspace and builds release binaries locally.

```sh
makers lint            # rustfmt, taplo, clippy -D warnings
makers test            # nextest + doctests
makers validate_rules  # golden corpus: every rule vs. its fixtures
makers A               # everything above plus check, bench compile and a release build
```

If you add, remove, or rename a rule, regenerate the config schema and commit it:

```sh
cargo xtask schema        # writes schema/falcon.schema.json
```

`cargo test --workspace` fails if the committed schema is stale (a contract test
regenerates it and compares), so this isn't optional.

Fix issues at the root cause. Don't silence clippy, `#[allow]` around a real
problem, or loosen a test to go green.

The benchmark in `benches/jfit_mobile_bench.rs` optionally uses a private corpus
outside the repository. Set `JFIT_PATH` to opt in; there is no machine-specific
fallback path. The corpus is not required to contribute or run the test suite.

## The golden corpus

Every rule is validated against fixtures under
`crates/falcon_rules/tests/corpus/<rule-id>/`:

- **`bad.dart`**: code that triggers the rule, with a `/* expect: <rule-id> */`
  annotation on the same line as each violation (optionally
  `/* expect: <rule-id>, msg: "..." */` to assert the message).
- **`good.dart`**: code that must produce **zero** diagnostics for the rule (no
  annotations at all).
- Additional `bad_2.dart` / `good_2.dart` variants are allowed; aim for ≥5
  positive and ≥5 negative examples.
- **`config.json`** (optional): a full-shaped `falcon.json` applied only when
  validating that directory; use it for threshold- or option-gated rules.
- **`cross-file/`** subdir: cross-file rules put a small multi-file Dart
  project here so the harness can exercise whole-module analysis.

The full spec, including the annotation format and upstream-fixture migration
notes, is in
[`crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md`](./crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md).

`cargo xtask validate-rules` runs the whole corpus; `cargo xtask validate-rules
--rule <rule-id>` runs one rule.

## Adding a rule end-to-end

1. **Scaffold** the implementation and fixture stubs:

   ```sh
   cargo xtask codegen rule --group <a11y|complexity|correctness|performance|security|style|suspicious> --name <rule-id>
   ```

   This writes the rule module and empty `bad.dart` / `good.dart` fixtures, then
   prints the exact follow-up steps.

2. **Implement** the rule. Derive `Default` and implement `Rule` from `falcon_analyze`, or `CrossFileRule` for project-wide analysis. The required `prepare` hook selects exactly one analysis kind:

   - `RulePreparation::Structural`: syntax rules return an independent `StructuralRuleVisitor` collector. Wrap the complete trait implementation in `falcon_analyze::structural_visitor!` so callback subscriptions are derived from its methods, not maintained separately. All enabled collectors share the source-order structural walk. Inspect nodes in `visit_*` callbacks, restore scoped state in matching `exit_*` callbacks, and return diagnostics from consuming `finish`. Do not recurse from callbacks. `StructuralTraversal::SkipChildren` and `route_child` suppress callbacks only for that collector, never for other rules.
   - `RulePreparation::Semantic`: type- or flow-aware rules return a `SemanticRuleVisitor` collector for the shared evaluation-order semantic walk.
   - `RulePreparation::Diagnostics`: reserve immediate diagnostics for genuine whole-file, source/token, path, or directive checks, or for a rule gated off before collection. Do not run a rule-local primary AST traversal here.

   Use the parsed `SyntaxStore` through `ctx.syntax` for tokens, delimiters, and comments; don't re-lex the file. Pass the parser's store to `AnalyzeContext::new(path, source, config, &syntax)`. Token ids belong to their store, so use `syntax_for_span(span)` for nested interpolations. Preserve literal spelling when comparing expressions.

   Use `ctx.local_writes(program)` or `ctx.mutated_declarations(program)` for mutation checks. They resolve declarations and handle shadowing; name-only write facts are not a substitute.

   > **Construction gotcha:** `Expr::New` is emitted **only** for `new X(...)` and
   > `const X(...)`. A plain `X()` or a named constructor like `X.from()` parses as
   > a **`Call` chain**, not `Expr::New`. A rule that detects object construction
   > must handle both shapes, or it will silently miss the common `X()` case.

3. **Declare the module** in `crates/falcon_rules/src/lint/<group>.rs`
   (or `src/cross_file.rs` for a cross-file rule).

4. **Register the rule** with one `RuleMeta` entry in
   `crates/falcon_rules/src/meta.rs`, using `RuleMeta::file::<T>` for syntax-only
   rules, `RuleMeta::resolved::<T>` for file rules requiring resolution, or
   `RuleMeta::cross_file::<T>` for project-wide rules. Pass the canonical name,
   typed `RuleGroup`, optional `Domain`, `recommended` flag, and source. Sources are
   `RuleSource::DartCodeLinter("...")`, `RuleSource::PyramidLint("...")`,
   `RuleSource::Lints("...")`, or `RuleSource::Falcon` for original rules.
   Constructors use `T::default()`. Execution identities, registries, and schemas
   derive from this table; there is no separate registration list or rule name
   method.

5. **Add a showcase entry**: list the rule in the root `falcon.json` under its
   group (`linter.rules.<group>` for file rules, `"cross-file".rules.<group>` for
   cross-file rules).

   Compilation checks module declarations and constructors. `meta_tests.rs`
   checks metadata name uniqueness, source ids, and the committed schema;
   corpus tests check diagnostic identities against fixture expectations.
   `root_config_tests.rs` requires the root `falcon.json` to list every rule
   exactly once.

6. **Write the corpus** (`bad.dart` + `good.dart`, plus `config.json` if the rule
   takes options), then run `cargo xtask validate-rules --rule <rule-id>` until green.

### Rule options

Read options with `crate::lint::options::for_rule(ctx, "<rule-id>")` and the typed `options::usize` / `options::strings` helpers. They look up the group from metadata and honor per-path overrides through `ctx.rule_options`. See `max_parameters_for_function` or `use_design_system_item` for examples. Document new options in [`docs/configuration.md`](./docs/configuration.md).

### Suppressions

Inline suppression is handled centrally by `crates/falcon_analyze/src/suppressions.rs`; don't wire it up per rule. Use `// falcon-ignore lint/<group>/<rule>: <reason>` for file rules or `cross-file/<group>/<rule>` for cross-file rules. `falcon-ignore-all` suppresses a rule throughout a file. A reason is mandatory, and malformed directives are reported.

## Commits & pull requests

- Use **conventional commits** (`feat:`, `fix:`, `chore:`, `refactor:`, …). One
  logical change per commit.
- Run the four gates before pushing.
- Keep PR descriptions concrete: what changed, why, and how you verified it.

## Releasing (maintainers)

falcon versions its workspace crates together. To cut a release:

1. Bump `version` under `[workspace.package]` in `Cargo.toml` (all workspace crates derive from it).
2. Commit, then tag: `git tag vX.Y.Z && git push --tags`.

The tag triggers `.github/workflows/release.yml`, which builds the four platform binaries and publishes a GitHub Release with auto-generated notes (failing if the tag and `Cargo.toml` version disagree).

