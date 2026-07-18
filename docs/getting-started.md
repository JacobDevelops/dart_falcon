# Getting Started

falcon is a fast, standalone linter for Dart and Flutter, written in Rust. It
parses Dart itself — no Dart SDK, no `analyzer` package, no analysis server — so a
whole project lints in a single pass. This guide takes you from install to a
green CI run.

## Installation

falcon ships as a single self-contained binary. Pick whichever channel fits your
setup; [installation.md](./installation.md) covers every platform and channel in
full.

**Prebuilt binary** (Linux x86_64/aarch64 static, macOS Intel/Apple Silicon):

```sh
curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/latest/download/falcon-0.3.0-x86_64-linux.tar.gz | tar -xz
sudo mv falcon /usr/local/bin/
falcon version
```

**From source** with a stable Rust toolchain:

```sh
cargo install --git https://github.com/JacobDevelops/dart_falcon dart_falcon
```

**Nix** — run it directly or add the flake (`github:JacobDevelops/dart_falcon`)
to a devShell:

```sh
nix run github:JacobDevelops/dart_falcon -- check .
```

## Configuration

falcon runs with **zero configuration**: with no `falcon.json` present, every
recommended rule runs at its default severity (warning).

```sh
falcon check .
```

When you want to tune the rule set, drop a `falcon.json` at your project root.
There is no `init` command — create the file yourself:

```json
{
  "$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/main/schema/falcon.schema.json",
  "linter": {
    "rules": {
      "recommended": true,
      "complexity": { "max-lines-for-file": "off" }
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

> **Note:** Point `$schema` at the published schema for rule-name autocomplete and
> validation in your editor. All rule ids and config keys are **kebab-case**
> (`max-lines-for-file`, `cross-file`, `max-errors`).

Rules live under groups (`complexity`, `correctness`, `performance`, `style`,
`suspicious`); each entry is a level string (`off`/`on`/`info`/`warn`/`error`) or
a `{ "level", "options" }` object. Whole-project checks such as `unused-files`
live in the separate top-level [`cross-file`](./configuration.md#cross-file--cross-file-rules)
section — not under `linter`. See [configuration.md](./configuration.md) for the
full surface.

## Usage

Lint the current directory, specific paths, or the whole project:

```sh
falcon check .            # lint the current directory
falcon check lib/ test/   # lint specific paths
falcon check . --config ./falcon.json   # point at an explicit config
```

Emit machine-readable output for tooling:

```sh
falcon check . --format json
```

falcon discovers `falcon.json` automatically — the current directory, then the
enclosing git root, then `~/.falcon.json`.

**Exit codes:** `falcon check` exits `0` when clean and `1` when any diagnostic
(warning or error) is reported — so a failing check breaks CI by default. Override
the failure code with `--exit-code`, or cap output with `--max-errors`:

```sh
falcon check . --exit-code 2    # use exit code 2 on findings
falcon check . --max-errors 50  # stop after 50 diagnostics
```

## Editor setup

falcon speaks LSP. Start the server directly:

```sh
falcon lsp
```

First-party extensions launch that server for you and wire up `$schema`
autocomplete for `falcon.json`:

- **VS Code** — `extensions/falcon-vscode`
- **Zed** — `extensions/falcon-zed`

Both launch the `falcon` binary from your `PATH`, so install falcon with any
channel above first. See [installation.md](./installation.md#editor-extensions)
for details.

## CI usage

Download the release binary and run `falcon check` — a non-zero exit fails the
job automatically:

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
          curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/latest/download/falcon-0.3.0-x86_64-linux.tar.gz | tar -xz
          sudo mv falcon /usr/local/bin/
      - name: Lint
        run: falcon check .
```

> **Note:** Pin the version in the download URL (here `0.3.0`) so CI is
> reproducible. falcon is [pre-1.0](../ROADMAP.md) — behavior can change between
> releases.

## Next steps

- [Configuration](./configuration.md) — every rule, option, per-path `overrides`,
  and the `flutter` domain.
- [Rules index](/linter/rules) — the full rule catalog with examples.
- [Suppressions](./suppressions.md) — silence a single diagnostic inline with
  `// falcon-ignore`.
- [Installation](./installation.md) — all channels, supported platforms, and how
  releases are cut.
- [Migrating](./configuration.md#migrating-from-dart_code_linter--pyramid_lint) —
  generate a `falcon.json` from an existing `analysis_options.yaml`.
- [Roadmap](../ROADMAP.md) — near-term plans and the road to 1.0.
