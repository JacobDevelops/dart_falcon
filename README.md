<img src="assets/logo/falcon-icon.svg" width="96" alt="falcon logo" align="right">

# falcon

[![CI](https://github.com/JacobDevelops/dart_falcon/actions/workflows/ci.yml/badge.svg)](https://github.com/JacobDevelops/dart_falcon/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/JacobDevelops/dart_falcon)](https://github.com/JacobDevelops/dart_falcon/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)

Docs and rule reference: [dart-falcon.jacobdevelops.com](https://dart-falcon.jacobdevelops.com)

falcon is a standalone linter for Dart and Flutter, written in Rust. It parses Dart without the Dart analyzer or analysis server. Install the Dart SDK and resolve your dependencies for rules that need SDK or package types.

When separate apps load physical copies of the same package or SDK library, falcon uses the copy with the lexicographically last canonical defining-file path consistently, independent of traversal or editor-open order. This deterministic fallback does not provide version-specific member resolution for each app; check apps separately when dependency versions differ. Unrequested local libraries contribute type and deprecation metadata without becoming lint targets.

falcon includes **513 shipped rules**, with **215 recommended** by default, across seven groups: `a11y`, `complexity`, `correctness`, `performance`, `security`, `style`, and `suspicious`. The shipped rules include cross-file checks and 142 official lints from the 1.0 catalog. Falcon chooses its recommended set independently of upstream presets.

Enabled semantic rules share one scope-aware traversal per file, with independent per-rule collectors and diagnostics. Syntax-only rules and dependency-ordered library analyses retain their specialized traversals.

Configure rules, severities, options, and per-path overrides in `falcon.json`. Cross-file checks have a separate `cross-file` section. Inline suppressions require a reason: `// falcon-ignore lint/<group>/<rule>: <reason>`.

> **Status: pre-1.0.** falcon is usable today but the rule set, rule ids, and
> config surface are still moving. Expect breaking changes before 1.0 (see the
> [roadmap](./ROADMAP.md)). Pin a revision if you depend on stable behaviour.

## Performance

With cold analyzer state, falcon runs **3.9–6.0× faster than `dart analyze` across all four measured corpora**; with warm state, `dart analyze` is faster on three. Cold state is relevant to fresh CI runs, not CI systems retaining analyzer caches. Dart also performs full type checking, and diagnostic counts differ. See the [results, limitations, and reproduction instructions](https://dart-falcon.jacobdevelops.com/benchmarks).

## Installation

falcon ships as a single binary.

**Prebuilt binaries** (Linux x86_64/aarch64 static, macOS Intel/Apple Silicon)
are attached to every [GitHub Release](https://github.com/JacobDevelops/dart_falcon/releases):

```sh
curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/latest/download/falcon-x86_64-linux.tar.gz | tar -xz
sudo mv falcon /usr/local/bin/
```

**From source** with a nightly Rust toolchain:

```sh
cargo +nightly install --git https://github.com/JacobDevelops/dart_falcon dart_falcon
```

On x86_64 GNU/Linux, falcon uses jemalloc for Rust allocations with its default
configuration. Other targets, including the prebuilt binaries, retain the system
allocator. Building jemalloc requires a native C compiler,
a POSIX shell for the bundled configure script, and make. The Rust allocator
wrapper is MIT/Apache-2.0 licensed; bundled jemalloc is
[BSD-2-Clause licensed](https://docs.rs/crate/tikv-jemalloc-sys/0.7.1+5.3.1-0-g81034ce1f1373e37dc865038e1bc8eeecf559ce8/source/jemalloc/COPYING).

**mise**: add this to your project's `mise.toml`, then run `mise install`:

```toml
[tools]
"github:JacobDevelops/dart_falcon" = { version = "0.4.3", matching = "falcon-" }
```

Run `mise exec -- falcon check .`, or use `falcon` directly with mise activated.

See [docs/installation.md](./docs/installation.md) for all channels, supported
platforms, and how releases are cut.

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
recommended rule runs at its default severity. Pass `--config <path>` to point at
a specific file.

Checks analyze files in parallel and sort diagnostics deterministically. Set `RAYON_NUM_THREADS` to limit worker threads, for example `RAYON_NUM_THREADS=4 falcon check .`.

## Configuration

`falcon.json` groups rules by category. Each entry is a severity string or `{ "level": ..., "options": { ... } }`:

```json
{
  "linter": {
    "rules": {
      "recommended": true,
      "complexity": { "max-lines-for-file": "off" },
      "style": { "prefer-trailing-comma": { "level": "error", "options": {} } }
    },
    "domains": { "flutter": "recommended" }
  },
  "cross-file": {
    "enabled": true,
    "rules": {
      "correctness": { "unused-files": "warn", "unused-code": "warn" }
    }
  }
}
```

See [docs/configuration.md](./docs/configuration.md) for options, per-path `overrides`, and the `flutter` domain.

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

- **VS Code**: `extensions/falcon-vscode`
- **Zed**: `extensions/falcon-zed`
- **Neovim**: no plugin needed, see the [setup snippet](./docs/installation.md#neovim)

## Roadmap

Near-term plans and the road to 1.0 are tracked in [ROADMAP.md](./ROADMAP.md).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for setup, checks, and how to add a rule.

## Credits

falcon's configuration design is inspired by [Biome](https://biomejs.dev). Many rules are ports of [dart_code_linter](https://github.com/dart-code-checker/dart-code-linter), [pyramid_lint](https://pub.dev/packages/pyramid_lint), and Dart's official lints.

## License

[MIT](./LICENSE) © 2026-present Jacob Sanderson and falcon contributors.
