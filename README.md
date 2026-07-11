# falcon

[![CI](https://github.com/JacobDevelops/dart_falcon/actions/workflows/ci.yml/badge.svg)](https://github.com/JacobDevelops/dart_falcon/actions/workflows/ci.yml)

**falcon** is a fast, standalone linter for Dart and Flutter, written in Rust. It
parses Dart itself — no Dart SDK, no `analyzer` package, no analysis server — so a
whole project lints in a single pass with no warm-up.

falcon ships **79 lint rules**: 76 across five groups (`complexity`,
`correctness`, `performance`, `style`, `suspicious`) plus 3 cross-file **project**
rules (`unused-files`, `unused-code`, `unnecessary-nullable`) that reason about the
whole module graph rather than one file at a time.

Configuration is a single biome-shaped `falcon.json`: grouped rules, per-rule
severities and options, per-path overrides, a `flutter` domain, and a separate
`project` section for cross-file analysis. Diagnostics are suppressed inline with
`// falcon-ignore lint/<group>/<rule>: <reason>` — the reason is mandatory.

> **Status: pre-1.0.** falcon is usable today but the rule set, rule ids, and
> config surface are still moving. Expect breaking changes before 1.0 (see the
> [roadmap](./ROADMAP.md)). Pin a revision if you depend on stable behaviour.

## Installation

falcon is distributed as a Nix flake whose default package is a **prebuilt static
binary** fetched from GitHub Releases — zero compilation, no Rust toolchain.

Add it as a flake input:

```nix
{
  inputs.falcon.url = "github:JacobDevelops/dart_falcon";
}
```

Then reference `falcon.packages.${system}.default` in a devShell or package list.
Or run it directly without installing anything:

```sh
nix run github:JacobDevelops/dart_falcon -- check .
```

See [docs/installation.md](./docs/installation.md) for supported systems, the
build-from-source package, and how releases are cut.

### Build from source

With a stable Rust toolchain and Cargo:

```sh
cargo build --release
./target/release/falcon check .
```

## Usage

```sh
falcon check .            # lint the current directory
falcon check lib/ test/   # lint specific paths
falcon check . --format json
falcon lsp                # start the language server (JSON-RPC over stdin)
falcon version            # print version information
```

`falcon check` discovers `falcon.json` automatically: first the current directory,
then the enclosing git root, then `~/.falcon.json`. With no config found, every
rule runs at its default severity. Pass `--config <path>` to point at a specific
file.

## Configuration

`falcon.json` is biome 2.x-shaped — rules are grouped, and each entry is either a
severity string or `{ "level": ..., "options": { ... } }`:

```json
{
  "linter": {
    "rules": {
      "recommended": true,
      "complexity": { "max_lines_for_file": "off" },
      "style": { "prefer-trailing-comma": { "level": "error", "options": {} } }
    },
    "domains": { "flutter": "recommended" }
  },
  "project": {
    "enabled": true,
    "rules": {
      "correctness": { "unused-files": "warn", "unused-code": "warn" }
    }
  }
}
```

Full reference — every rule, option, per-path `overrides`, and the `flutter`
domain — is in [docs/configuration.md](./docs/configuration.md).

## Suppressions

Suppress a diagnostic inline with a reason (modelled on Biome's `biome-ignore`):

```dart
dynamic payload = decode(bytes); // falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
```

The reason after the colon is required; a malformed or reasonless directive is
itself reported. Use `// falcon-ignore-all lint/<group>/<rule>: <reason>` to
suppress a rule across a whole file.

## Editor support

falcon speaks LSP (`falcon lsp`). First-party extensions live in
[`extensions/`](./extensions):

- **VS Code** — `extensions/falcon-vscode`
- **Zed** — `extensions/falcon-zed`

## Roadmap

Near-term plans and the road to 1.0 are tracked in [ROADMAP.md](./ROADMAP.md).

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md) for the dev
environment, the CI gates, and how to add a rule end-to-end.

## Credits

falcon's configuration design is inspired by [Biome](https://biomejs.dev). Many
rules are ports of [dart_code_linter](https://github.com/dart-code-checker/dart-code-linter)
and [pyramid_lint](https://github.com/Nialixus/pyramid_lint); provenance is
tracked per-rule in the rule metadata (`RuleSource`).

## License

[MIT](./LICENSE) © 2026-present Jacob Sanderson and falcon contributors.
