# Getting Started

falcon is a standalone linter for Dart and Flutter, written in Rust. It doesn't run the Dart analyzer or analysis server. Install the Dart SDK and resolve your dependencies for rules that need SDK or package types.

## Installation

falcon ships as a single binary. [installation.md](./installation.md) covers all platforms and channels.

**Prebuilt binary** (Linux x86_64/aarch64 static, macOS Intel/Apple Silicon):

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
"github:JacobDevelops/dart_falcon" = { version = "0.4.2", matching = "falcon-" }
```

Run `mise exec -- falcon check .`, or use `falcon` directly with mise activated.

## Configuration

falcon runs with **zero configuration**: with no `falcon.json` present, its
**174 recommended rules out of 320 shipped rules** run at their default severity
(warning). Falcon chooses this set independently of upstream presets.

```sh
falcon check .
```

When you want to tune the rule set, drop a `falcon.json` at your project root.
There is no `init` command; create the file yourself:

```json
{
  "$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/main/schema/falcon.schema.json",
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
falcon check lib/ test/   # lint specific paths
falcon check . --config ./falcon.json   # point at an explicit config
```

Imported libraries and their parts provide type information even when they are outside the paths you pass. Diagnostics remain limited to the requested files.

Emit machine-readable output for tooling:

```sh
falcon check . --format json
```

Diagnostic `line` and `col` are one-based, with columns counted in UTF-16 code units. The half-open `span.start` and `span.end` are UTF-8 byte offsets into the source file, not a column and length. To compare a diagnostic range with `dart analyze --format=machine`, count the UTF-16 code units in the source slice covered by `span`.

The `eol-at-end-of-file` rule can report a virtual one-character range at EOF. In that case `span.end` is one byte beyond the file; preserve the extra unit when computing the UTF-16 range length.

falcon discovers `falcon.json` automatically: the current directory, then the
enclosing git root, then `~/.falcon.json`.

**Exit codes:** `falcon check` exits `0` when clean and `1` when diagnostics (including info or warnings) are reported. `--exit-code` changes the findings exit code. Configuration failures and source files larger than 4 GiB (4,294,967,295 bytes) exit `1` regardless of that flag; unreadable or non-UTF-8 files are skipped with a warning. Use `--max-errors` to cap reported diagnostics:

```sh
falcon check . --exit-code 2    # use exit code 2 on findings
falcon check . --max-errors 50  # report at most 50 diagnostics
```

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
          curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/download/v0.4.2/falcon-0.4.2-x86_64-linux.tar.gz | tar -xz
          sudo mv falcon /usr/local/bin/
      - name: Lint
        run: falcon check .
```

> Pin both the release tag and archive version in CI, for example `/releases/download/v0.4.2/falcon-0.4.2-x86_64-linux.tar.gz`. A `/releases/latest/download/` URL is not pinned. falcon is [pre-1.0](../ROADMAP.md), so behavior can change between releases.

## Next steps

- [Configuration](./configuration.md): options, per-path `overrides`, and the `flutter` domain.
- [Rules index](/linter/rules): rule descriptions and examples.
- [Suppressions](./suppressions.md): silence diagnostics with `// falcon-ignore`.
- [Installation](./installation.md): channels, platforms, and releases.
- [Migrating](./configuration.md#migrating-from-dart_code_linter--pyramid_lint): generate a `falcon.json` from `analysis_options.yaml`.
- [Roadmap](../ROADMAP.md): plans for 1.0.
