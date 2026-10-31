# Getting Started

falcon is a standalone linter for Dart and Flutter, written in Rust. It doesn't run the Dart analyzer or analysis server. Install the Dart SDK and resolve your dependencies for rules that need SDK or package types.

## Installation

falcon ships as a single binary. [installation.md](./installation.md) covers all platforms and channels.

**Prebuilt binary** (Linux x86_64/aarch64 static, macOS Apple Silicon):

```sh
curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/latest/download/falcon-x86_64-linux.tar.gz | tar -xz
sudo mv falcon /usr/local/bin/
falcon version
```

**From source** with a nightly Rust toolchain:

```sh
cargo +nightly install --git https://github.com/JacobDevelops/dart_falcon dart_falcon
```

**mise**: add this to your project's `mise.toml`, then run `mise install`:

```toml
[tools]
"github:JacobDevelops/dart_falcon" = { version = "0.4.7", matching = "falcon-" }
```

Run `mise exec -- falcon check .`, or use `falcon` directly with mise activated.

## Configuration

falcon runs with **zero configuration**: with no `falcon.json` present, its
**215 recommended rules out of 546 shipped rules** run at their default severity
(warning). Falcon chooses this set independently of upstream presets.

```sh
falcon check .
```

When you want to tune the rule set, drop a `falcon.json` at your project root.
There is no `init` command; create the file yourself:

```json
{
  "$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/v0.4.7/schema/falcon.schema.json",
  "linter": {
    "rules": {
      "recommended": true,
      "complexity": { "max-lines-for-file": "warn" }
    },
    "domains": { "flutter": "recommended" }
  },
  "cross-file": {
    "enabled": true,
    "rules": {
      "correctness": { "unused-files": "warn" }
    }
  }
}
```

> Point `$schema` at the published schema for rule-name autocomplete and validation. Rule ids and structural config keys use kebab-case (`max-lines-for-file`, `cross-file`, `max-errors`); names inside rule `options` use snake_case.

Rules live under groups (`a11y`, `complexity`, `correctness`, `performance`,
`security`, `style`, `suspicious`); each entry is a level string (`off`/`on`/`info`/`warn`/`error`) or
a `{ "level", "options" }` object. Whole-project checks such as `unused-files`
live in the separate top-level [`cross-file`](./configuration.md#cross-file-cross-file-rules)
section, not under `linter`. See [configuration.md](./configuration.md) for the
full surface.

## Usage

Lint the current directory, specific paths, or the whole project:

```sh
falcon check .            # lint the current directory
falcon check              # equivalent to falcon check .
falcon check lib/ test/   # lint specific paths
falcon check . --config ./falcon.json   # point at an explicit config
```

Imported libraries and their parts provide type information even when they are outside the paths you pass. Diagnostics remain limited to the requested files.

Apply safe fixes, explicitly include unsafe fixes, or preview without writing:

```sh
falcon check . --write
falcon check . --write --unsafe
falcon check . --write --dry-run
```

Fixes run one at a time with re-analysis between edits. Conflicting fixes that
cycle, invalid edits, and the iteration backstop are reported as errors rather
than silently stopping. Cross-file findings are checked after fixing, but their
fixes are not automatically applied. See [autofix policy](./configuration.md#autofix-policy)
for per-rule classification and overrides.


Emit machine-readable output for tooling:

```sh
falcon check . --format json
```

CI reports are available as `--format sarif` (SARIF 2.1.0),
`--format gitlab` (GitLab Code Quality / CodeClimate JSON),
`--format checkstyle` (XML), `--format sonar` (Sonar generic issue import JSON),
and `--format html` (a self-contained page). Redirect stdout to a report file.
GitLab and Sonar paths are relative to the current directory, so invoke falcon
from the repository or Sonar project base. Other report paths are relative to
the configuration root, or the current directory when no configuration exists.
Status messages stay on stderr. Output limits also apply to report files;
omit `max-errors` when uploading a complete CI report.

Diagnostic `line` and `col` are one-based, with columns counted in UTF-16 code units; unresolved positions are omitted from JSON rather than reported as zero. JSON severities are lowercase (`error`, `warning`, `info`). The half-open `span.start` and `span.end` are UTF-8 byte offsets into the source file, not a column and length. To compare a diagnostic range with `dart analyze --format=machine`, count the UTF-16 code units in the source slice covered by `span`.

The `eol-at-end-of-file` rule can report a virtual one-character range at EOF. In that case `span.end` is one byte beyond the file; preserve the extra unit when computing the UTF-16 range length.

Diagnostics may include a `fix` object with a `title` (the quick-fix label) and a `kind` of `safe` (preserves program behaviour) or `unsafe` (may change behaviour). Its non-empty `edits` contain a `span` and `replacement` string. Edit spans are half-open UTF-8 byte offsets into the original source, sorted, non-overlapping, within bounds, and on character boundaries. Apply all edits of one fix atomically against that original source, then re-analyze before choosing another fix. Diagnostics without fixes omit `fix`. LSP diagnostics do not carry fix payloads; editors request quick fixes and fix-all through code actions.

falcon discovers `falcon.json` automatically: the current directory, then the
nearest Git or Jujutsu repository root (`.git` or `.jj`), then `~/.falcon.json`.

**Exit codes:** `falcon check` exits `0` when there are no errors, including warning-only and info-only runs. Errors exit `1`; `--error-on-warnings` (or config `error-on-warnings: true`) also fails warning-only runs. Info never fails a run. `--exit-code` (also accepted as `--error-exit-code`) changes the findings exit code. Configuration and analysis pipeline failures exit `1` regardless of that flag. A missing or explicitly filtered path, or a run that selects neither Dart files nor package manifests, reports an error; use `--no-errors-on-unmatched` for tools such as lint-staged that can pass unmatched paths. A manifest counts when named explicitly or found within an input directory, not merely discovered in an ancestor directory. Omitting paths checks the current directory. Unreadable or non-UTF-8 files are skipped with a warning. `--max-errors` limits printing only, not the exit code:

```sh
falcon check . --exit-code 2    # use exit code 2 on findings
falcon check . --max-errors 50  # report at most 50 diagnostics
falcon check missing.dart --no-errors-on-unmatched  # tolerate unmatched paths
```

### Adopting falcon with existing findings

Record current lint debt, then report only new findings:

```sh
falcon check . --write-baseline             # writes falcon-baseline.json
falcon check . --write-baseline debt.json   # choose a path
falcon check . --baseline debt.json --error-on-warnings
```

The baseline stores counted content fingerprints, not line numbers or messages.
Inserting lines above a finding does not make it new, while adding another
identical occurrence still reports that extra finding. Changes to the flagged
source text can make a finding new. Syntax and operational errors cannot be
baselined. Suppressed and fixed (stale) counts are printed on stderr. Counts
describe the selected input paths: use the same full-project inputs for capture
and CI rather than treating findings outside a partial run as fixed. Regenerating
replaces the whole baseline with current findings; syntax and operational errors
prevent writing it. Baseline matching happens before output limits and exit-code
selection. CLI baseline paths resolve from the current directory; a configured
`baseline` path resolves from the configuration root.

## Editor setup

falcon speaks LSP. Start the server directly:

```sh
falcon lsp
```

First-party extensions launch that server for you. The VS Code extension also enables schema autocomplete for `falcon.json`:

- **VS Code**: `extensions/falcon-vscode`
- **Zed**: `extensions/falcon-zed`
- **Neovim**: a short `vim.lsp.config` snippet, no plugin needed

All of them launch the `falcon` binary from your `PATH`, so install falcon with any
channel above first. See [installation.md](./installation.md#editor-extensions)
for details.

## CI usage

Download the release binary and run `falcon check`; a non-zero exit fails the job:

```yaml
# .github/workflows/lint.yml
name: lint
on: [push, pull_request]
jobs:
  falcon:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install falcon
        run: |
          curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/download/v0.4.7/falcon-0.4.7-x86_64-linux.tar.gz | tar -xz
          sudo mv falcon /usr/local/bin/
      - name: Lint
        run: falcon check .
```

> Pin both the release tag and archive version in CI, for example `/releases/download/v0.4.7/falcon-0.4.7-x86_64-linux.tar.gz`. A `/releases/latest/download/` URL is not pinned. falcon is [pre-1.0](./v1-checklist.md), so behavior can change between releases.

## Next steps

- [Configuration](./configuration.md): options, per-path `overrides`, and the `flutter` domain.
- [Rules index](/linter/rules): rule descriptions and examples.
- [Suppressions](./suppressions.md): silence diagnostics with `// falcon-ignore`.
- [Installation](./installation.md): channels, platforms, and releases.
- [Migrating](./configuration.md#migrating-from-dart_code_linter--pyramid_lint): generate a `falcon.json` from `analysis_options.yaml`.
- [1.0 checklist](./v1-checklist.md): what's left before 1.0.
