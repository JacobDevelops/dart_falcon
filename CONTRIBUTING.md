# Contributing to falcon

Thanks for your interest in falcon. This guide covers the dev environment, the
checks your change must pass, and how to add a lint rule end-to-end.

falcon is pre-1.0: rule ids, the rule set, and the config surface are still
moving. If you're planning a large change, open an issue first so we can agree on
the shape before you write it.

## AI assistance notice

AI-assisted contributions are welcome, as long as **you have verified the result**
— you understand the change, it passes every gate below, and the diagnostics are
actually correct (a plausible-looking rule that fires on the wrong nodes is worse
than no rule). Please disclose meaningful AI assistance in your pull request so
reviewers can calibrate. Don't open a PR you can't explain.

## Dev environment

Install [mise](https://mise.jdx.dev/getting-started.html) and [rustup](https://rustup.rs), then run from the repo root:

```sh
mise install
cargo build
```

`mise.toml` pins Bun, `cargo-nextest`, and `cargo-watch`. Activate mise in your shell to put them on `PATH`, or prefix commands with `mise exec --`. Rust stays under rustup: `rust-toolchain.toml` selects stable Rust, `rust-src`, `rust-analyzer`, Clippy, rustfmt, and the `wasm32-wasip2` target. Cargo installs the declared toolchain on first use.

The repo is a colocated jj + git repo. **jj (Jujutsu) users are welcome**, but
plain git works fine — use whichever you prefer.

## The gates

Your change must pass all of these before it's ready for review. CI
(`.github/workflows/ci.yml`) runs the same set:

```sh
cargo fmt --all                                     # formatting
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                              # unit + integration + contract tests
cargo xtask validate-rules                          # golden corpus: every rule vs. its fixtures
```

If you add, remove, or rename a rule, regenerate the config schema and commit it:

```sh
cargo xtask schema        # writes schema/falcon.schema.json
```

`cargo test --workspace` fails if the committed schema is stale (a contract test
regenerates it and compares), so this isn't optional.

Fix issues at the root cause — don't silence clippy, `#[allow]` around a real
problem, or loosen a test to go green.

The benchmark in `benches/jfit_mobile_bench.rs` optionally uses a private corpus
outside the repository. Set `JFIT_PATH` to opt in; there is no machine-specific
fallback path. The corpus is not required to contribute or run the test suite.

## The golden corpus

Every rule is validated against fixtures under
`crates/falcon_rules/tests/corpus/<rule-id>/`:

- **`bad.dart`** — code that triggers the rule, with a `/* expect: <rule-id> */`
  annotation on the same line as each violation (optionally
  `/* expect: <rule-id>, msg: "..." */` to assert the message).
- **`good.dart`** — code that must produce **zero** diagnostics for the rule (no
  annotations at all).
- Additional `bad_2.dart` / `good_2.dart` variants are allowed; aim for ≥5
  positive and ≥5 negative examples.
- **`config.json`** (optional) — a full-shaped `falcon.json` applied only when
  validating that directory; use it for threshold- or option-gated rules.
- **`cross-file/`** subdir — cross-file rules put a small multi-file Dart
  project here so the harness can exercise whole-module analysis.

The full spec, including the annotation format and upstream-fixture migration
notes, is in
[`crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md`](./crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md).

`cargo xtask validate-rules` runs the whole corpus; `cargo xtask validate-rules
--rule <rule-id>` runs one rule.

## Adding a rule end-to-end

1. **Scaffold** the implementation and fixture stubs:

   ```sh
   cargo xtask codegen rule --group <complexity|correctness|performance|style|suspicious> --name <rule-id>
   ```

   This writes the rule module and empty `bad.dart` / `good.dart` fixtures, then
   prints the exact follow-up steps.

2. **Implement** the rule. Rule structs derive `Default` and implement the `Rule`
   trait from `falcon_analyze` (or `CrossFileRule` for project-wide analysis).
   Use the `Visitor` trait / `visit_*` helpers in `falcon_syntax::visitor` to
   traverse the AST from `falcon_syntax` (`falcon_syntax::ast::*`).

   > **Construction gotcha:** `Expr::New` is emitted **only** for `new X(...)` and
   > `const X(...)`. A plain `X()` or a named constructor like `X.from()` parses as
   > a **`Call` chain**, not `Expr::New`. A rule that detects object construction
   > must handle both shapes, or it will silently miss the common `X()` case.

3. **Declare the module** in `crates/falcon_rules/src/lint/<group>/mod.rs`
   (or `src/cross_file.rs` for a cross-file rule).

4. **Register the rule** with one `RuleMeta` entry in
   `crates/falcon_rules/src/meta.rs`, using `RuleMeta::file::<T>` for syntax-only
   rules, `RuleMeta::resolved::<T>` for file rules requiring resolution, or
   `RuleMeta::cross_file::<T>` for project-wide rules. Pass the canonical name,
   typed `RuleGroup`, `Domain` slice, `recommended` flag, and source. Sources are
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

Rules read their options through `ctx.config.rule_options(group, rule_id)`, which
returns the parsed option object for the active config (or `None` for defaults).
See `max_parameters_for_function` or `use-design-system-item` for a worked
example, and document new options in [`docs/configuration.md`](./docs/configuration.md).

### Suppressions

Inline suppression is handled centrally by
`crates/falcon_analyze/src/suppressions.rs` — you don't wire it up per rule. The
directive shape is `// falcon-ignore lint/<group>/<rule>: <reason>` (or
`cross-file/<group>/<rule>` for cross-file rules — the legacy `project/…` spelling
still works — and the `falcon-ignore-all` whole-file variant). The reason is
mandatory; a malformed directive is itself reported.

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

Thanks for contributing!
