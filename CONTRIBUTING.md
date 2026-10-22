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

`mise.toml` installs cargo-make (`makers`), `cargo-nextest` from the nextest GitHub release, `cargo-watch`, taplo, Bun, [Knope](https://knope.tech), and [mbx](https://github.com/jdx/mr-boxington) at their latest releases. Activate mise in your shell to put them on `PATH`, or prefix commands with `mise exec --`. mise routes `cargo` through mbx, a compiler cache shared across checkouts, and `makers` tasks call `mbx` directly (`CARGO_CMD=cargo makers ...` bypasses it). Rust stays under rustup: `rust-toolchain.toml` tracks the latest nightly with `rust-src`, `rust-analyzer`, Clippy, rustfmt, and the `wasm32-wasip2` target. rustup installs the declared toolchain on first use. `makers --list-all-steps` lists every task.

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

Public-project comparisons against Dart 3.13.5 run with `cargo xtask bench-compare`
(or `makers bench_compare`). Install Flutter 3.47.7-stable with mise first;
hyperfine is pinned in `mise.toml`. The command builds release falcon, resolves
dependencies outside timing, and writes the website's benchmark data. See
[the benchmark methodology](docs/benchmarks.md) for scenarios, pinned projects,
cache handling, limitations, and reproduction options.

`cargo xtask perf-lock` checks the median release-binary runtime against a budget
on the private mobile corpus; set `JFIT_PATH` or pass `--corpus <path>`.
Use `--runs <n>` and `--budget-ms <ms>` to select the sample count and budget,
or `--falcon-bin <path> --skip-build` to measure an already-built binary.
The default release build honors `CARGO_TARGET_DIR`. `--json` emits the timing
result for automation.

## The golden corpus

Every rule is validated against fixtures under
`crates/falcon_rules/tests/corpus/<rule-id>/`:

- **`bad.dart`**: code that triggers the rule, with a `/* expect: <rule-id> */`
  annotation on the same line as each violation (optionally
  `/* expect: <rule-id>, msg: "..." */` to assert the message).
- **`good.dart`**: code that must produce **zero** diagnostics for the rule (no
  annotations at all).
- Additional `bad_2.dart` / `good_2.dart` variants and nested fixture directories
  are allowed. Both harnesses retain all top-level Dart fixtures and recursively
  include nested files with `bad*`, `good*`, or `clean*` basenames, or an `expect:`
  annotation for their owning rule. Other nested files remain dependency/support
  inputs, not isolated clean fixtures. Aim for ≥5 positive and ≥5 negative examples.
- **`config.json`** (optional): a full-shaped `falcon.json` applied only when
  validating that directory; use it for threshold- or option-gated rules.
- **`cross-file/`** subdir: cross-file rules put independent multi-file Dart
  projects in `bad*` and `good*` directories here. Both harnesses analyze each
  project together, including recursively nested sources, rather than treating
  its files as isolated fixtures.
- **`project/`** subdir: resolver-dependent file rules put related sources here.
  Both harnesses analyze these recursively as one project, preserving imports,
  parts, and library context. `cross-file/` and resolver `project/` inputs are
  excluded from ordinary isolated-file discovery.

The full spec, including the annotation format and upstream-fixture migration
notes, is in
[`crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md`](./crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md).

`cargo test -p falcon_rules --test corpus_tests` runs the corpus in-process
without a pre-built CLI. It matches every annotation by rule, file, and line;
when `msg:` is present, it also requires the exact produced message, including
in cross-file and resolver projects. Mismatches show expected and actual text.
Produced corpus diagnostics must end in a full stop and use balanced backticks
for code; a style ratchet rejects quoted code and bare camelCase, multiword
PascalCase, or qualified API spellings without banning natural apostrophes in
contractions, possessives, or ordinary quoted prose.

`cargo xtask validate-rules` runs the same fixture layouts through the CLI;
`cargo xtask validate-rules --rule <rule-id>` runs one rule and fails if the
filter matches nothing. Its optional message comparison uses the configured
fuzzy threshold (`--threshold`, default `0.85`), including project fixtures.
Run both harnesses: CLI validation also covers command-line execution, while
in-process tests enforce exact messages and registration/coverage contracts.
Both CLI corpus validation and documentation generation honor `CARGO_TARGET_DIR`.
Run `cargo xtask docgen` to regenerate website data, or `cargo xtask docgen --check`
to reject stale committed data without writing files. Generation also rejects
empty rule descriptions and bad examples that produce no diagnostics.

## Adding a rule end-to-end

1. **Scaffold** the implementation and fixture stubs:

   ```sh
   cargo xtask codegen rule --group <a11y|complexity|correctness|performance|security|style|suspicious> --name <rule-id>
   ```

   Rule ids must be canonical ASCII kebab-case: begin with a lowercase letter,
   then use lowercase letters, digits, and single hyphens between nonempty
   segments. Add `--cross-file` for a project-wide rule:

   ```sh
   cargo xtask codegen rule --group correctness --name avoid-project-foo --cross-file
   ```

   This writes a `Rule` stub under `src/lint/<group>/`, with placeholder
   `bad.dart` / `good.dart` fixtures. With `--cross-file`, it writes a
   `CrossFileRule` stub under `src/cross_file/` and separate
   `cross-file/bad/` (`main.dart` and `support.dart`) and `cross-file/good/`
   (`main.dart`) fixture projects, without top-level Dart fixtures. Replace every
   placeholder before submitting; scaffolding is not corpus coverage. The command
   prints the declaration entry and remaining steps. Internal diagnostic ids,
   Rust keywords, and names that conflict with the selected template are rejected
   before files are written.

2. **Implement** the rule. Derive `Default` and implement `Rule` from
   `falcon_analyze`, or implement `CrossFileRule::prepare_project` for project-wide
   analysis.
   File rules' required `prepare` hook selects exactly one analysis kind:

   - `RulePreparation::Structural`: syntax rules return an independent `StructuralRuleVisitor` collector. Wrap the complete trait implementation in `falcon_analyze::structural_visitor!` so callback subscriptions are derived from its methods, not maintained separately. All enabled collectors share the source-order structural walk. Inspect nodes in `visit_*` callbacks, restore scoped state in matching `exit_*` callbacks, and return diagnostics from consuming `finish`. Do not recurse from callbacks. `StructuralTraversal::SkipChildren` and `route_child` suppress callbacks only for that collector, never for other rules.
   - `RulePreparation::Semantic`: type- or flow-aware rules return a `SemanticRuleVisitor` collector for the shared evaluation-order semantic walk.
   - `RulePreparation::Diagnostics`: reserve immediate diagnostics for genuine whole-file, source/token, path, or directive checks, or for a rule gated off before collection. Do not run a rule-local primary syntax-tree traversal here.

   Use `ctx.tree()` with `ctx.source` for tokens, delimiters, comments, and typed CST views; don't re-lex the file. Construct a context with `AnalyzeContext::new(file_path, config, &parsed)`, where `parsed` is an `Arc<ParsedFile>` owning the source and tree. Interpolation nodes and their tokens belong to that same tree and use absolute source spans; no separate syntax store is needed. Preserve literal spelling when comparing expressions.

   Use `ctx.local_writes(program)` or `ctx.mutated_declarations(program)` for mutation checks. They resolve declarations and handle shadowing; name-only write facts are not a substitute.

   > **Construction gotcha:** `Expr::New` is emitted **only** for `new X(...)` and
   > `const X(...)`. A plain `X()` or a named constructor like `X.from()` parses as
   > a **`Call` chain**, not `Expr::New`. A rule that detects object construction
   > must handle both shapes, or it will silently miss the common `X()` case.

3. **Declare the rule once** inside `crate::declare_rules!` in
   `crates/falcon_rules/src/lint/<group>.rs`. For example, within the existing
   suspicious group declaration:

   ```rust
   crate::declare_rules! {
       Suspicious;
       avoid_foo::AvoidFoo => ("avoid-foo", file, None, false, RuleSource::Falcon);
   }
   ```

   Add the entry to the existing invocation, not a second invocation. For a
   project-wide rule, use the invocation in `crates/falcon_rules/src/cross_file.rs`;
   cross-file entries carry their group:

   ```rust
   avoid_project_foo::AvoidProjectFoo => ("avoid-project-foo", cross_file, Correctness, false, RuleSource::Falcon);
   ```

   Choose `file` for syntax-only rules, `resolved` for file rules requiring
   resolution, `cross_file` for project-wide rules, or `resolved_cross_file` when
   project-wide analysis requires resolution. Reading `ctx.library()` to inspect
   part siblings also requires resolution, even when the rule uses a structural
   collector. File entries include an optional domain (`None` or, for example,
   `Some(Domain::Flutter)`); cross-file entries omit this field. Both include the
   recommended flag and source. Sources are
   `RuleSource::DartCodeLinter("...")`,
   `RuleSource::PyramidLint("...")`, `RuleSource::Lints("...")`, or
   `RuleSource::Falcon` for original rules. Rules with options append
   `, options = module::Options` inside the declaration tuple, after the source.

   The macro declares the module, supplies the rule type's inherent `NAME`, and
   creates its metadata entry and default constructor. Execution identities,
   registries, and schemas derive from these declarations. Do not add a separate
   `pub mod`, `RuleMeta` entry, registration list, or rule name method.

4. **Add a showcase entry**: list the rule in the root `falcon.json` under its
   group (`linter.rules.<group>` for file rules, `"cross-file".rules.<group>` for
   cross-file rules).

   Compilation checks declarations and constructors. `meta_tests.rs` checks
   metadata name uniqueness, source ids, and the committed schema. Diagnostic
   identities come from the declarations and are stamped by the registries.
   `root_config_tests.rs` requires the root `falcon.json` to list every rule
   exactly once.

5. **Write the corpus** (`bad.dart` + `good.dart` for file rules, plus
   `config.json` if the rule takes options). Cross-file rules need genuine
   multi-file positive and negative projects in `cross-file/bad/` and
   `cross-file/good/`, not only single-file lookalikes. Run
   `cargo xtask validate-rules --rule <rule-id>`, then both `makers test` and
   `makers validate_rules`. The in-process corpus tests enforce registration
   coverage and reject orphan directories and placeholder fixtures; the CLI
   harness alone does not establish these contracts.

6. **Refresh generated contracts** with `cargo xtask schema` and
   `cargo xtask docgen`, and commit their outputs. Write the rule's Rustdoc
   description and real positive/negative examples before docgen: it extracts
   implementation documentation and runs a freshly built Falcon on the corpus.

7. **Add a changeset** with `makers changeset` for the new user-facing rule,
   following **Commits & pull requests**, and run **The gates** before review.

### Rule options

Define an `Options` type beside the rule, deriving `Default`, `serde::Serialize`,
`serde::Deserialize`, and `schemars::JsonSchema`. Use
`#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]` so omitted
fields use defaults and unknown keys are rejected. Document each field with a
Rustdoc description and preserve the rule's existing defaults; custom defaults
can use a manual `Default` implementation.

Implement the options contract with `options::impl_options!(Options)` and read
the typed value with `options::get::<Options>(ctx, Self::NAME)`, using
`crate::lint::options`. Declare the options type beside the rule in its existing
per-group `crate::declare_rules!` invocation:

```rust
avoid_foo::AvoidFoo => ("avoid-foo", file, None, false, RuleSource::Falcon, options = avoid_foo::Options);
```

The declaration supplies options registration and schema metadata. Do not add a
second registry in `lint/options.rs`. The loader validates options and reports
invalid values at their configuration path; the typed lookup honors per-path
overrides. Nested option models also derive `Serialize`, `Deserialize`, and
`JsonSchema`, with kebab-case field names and unknown-field rejection where
applicable.

Run `cargo xtask schema` and `cargo xtask docgen` after changing options. Their
schemas and the generated options table in [`docs/configuration.md`](./docs/configuration.md)
share the registered types, including field descriptions and defaults. Keep
handwritten behavioral examples outside the generated table markers.

### Suppressions

Inline suppression is handled centrally by `crates/falcon_analyze/src/suppressions.rs`; don't wire it up per rule. Use `// falcon-ignore lint/<group>/<rule>: <reason>` for file rules or `cross-file/<group>/<rule>` for cross-file rules. `falcon-ignore-all` suppresses a rule throughout a file. A reason is mandatory, and malformed directives are reported.

## Commits & pull requests

- Keep each commit to one logical change. Conventional commit prefixes are welcome, but release versions and notes come only from changesets, not commit messages or PR titles.
- Include a changeset for user-facing changes, such as new rules, diagnostic fixes, CLI behavior, or installation changes. Internal refactors, tests, and documentation-only changes do not need one unless they affect users.
- Run the four gates before pushing.
- Keep PR descriptions concrete: what changed, why, and how you verified it.

Run `makers changeset` (it wraps `knope document-change`) and commit the generated `.changeset/*.md` file with your PR. Choose `patch` for fixes, `minor` for features, or `major` for breaking changes, and describe the change for users. You can also create a uniquely named file by hand, for example `.changeset/fix-install.md`:

```markdown
---
default: patch
---

Fix installation on Linux.
```

`default` is Knope's single-package key. All workspace crates share the resulting version.

## Releasing (maintainers)

falcon versions its workspace crates together and publishes binaries through GitHub Releases, not crates.io.

1. Configure the repository Actions secret `RELEASE_PAT` with a fine-grained personal access token scoped to this repository, granting **Contents: read and write** and **Pull requests: read and write**. Also grant **Workflows: read and write** so refreshing an existing release branch can carry workflow changes from `main`. The PAT lets bot pushes trigger PR checks.
2. Merge user-facing PRs with their changesets. Every push to `main` with `.changeset/*.md` files refreshes the bot's `release` branch and release PR; pushes without changesets skip preparation. The credential-free `knope prepare-release` job bumps `Cargo.toml`, writes `CHANGELOG.md`, consumes changesets, runs `cargo xtask version` to refresh lockfiles and pins, and commits. It passes the release branch as a Git bundle to a fresh submission job with no compiler or restored cache. Only the submission step receives `RELEASE_PAT`; it pushes with hooks disabled and transient credentials, then creates or updates the release PR.
3. Review and merge the release PR. `.github/workflows/release.yml` builds the four platform binaries from the merge commit and publishes a GitHub Release tagged `vX.Y.Z`, using that version's changelog section as release notes.

`makers version X.Y.Z` (it wraps `cargo xtask version X.Y.Z`) still exists for manual bumps. It updates `[workspace.package]` in `Cargo.toml`, refreshes `Cargo.lock`, and rewrites pinned release URLs, schema URLs, archive names and mise pins in root and `docs/` Markdown, plus `website/src/data/cli.json` and both editor extensions. `CHANGELOG.md` is excluded so historical versions stay unchanged. A test fails if the current installation references drift; new pin shapes belong in `with_version_references` in `xtask/src/version.rs`.

For a manual release, update the version and changelog, commit and merge, then push the matching `vX.Y.Z` tag. Tag pushes and manual workflow dispatches with an existing tag remain supported, and fail if the tag and `Cargo.toml` version disagree. If an old version has no changelog section, GitHub generates its release notes.

Each tarball ships twice: `falcon-X.Y.Z-<platform>.tar.gz` for pinned installs and `falcon-<platform>.tar.gz` for the unpinned `releases/latest/download/` quick-start links. Releases include SHA256 checksums and build provenance attestations.

