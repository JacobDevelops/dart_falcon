# Installation

falcon ships as a single self-contained binary — no Dart SDK, no analysis
server, no runtime dependencies. Pick whichever channel fits your setup.

## Supported platforms

| Platform         | Binary                       |
| ---------------- | ---------------------------- |
| Linux x86_64     | fully static (musl)          |
| Linux aarch64    | fully static (musl)          |
| macOS Intel      | native                       |
| macOS Apple Silicon | native                    |

Windows binaries are not published yet — Windows users can
[build from source](#build-from-source) or use WSL.

## Prebuilt binaries (recommended)

Every release attaches platform tarballs to the
[GitHub Release](https://github.com/JacobDevelops/dart_falcon/releases). Each
tarball contains a single `falcon` executable.

```sh
# Substitute the latest version and your platform:
#   x86_64-linux | aarch64-linux | x86_64-darwin | aarch64-darwin
curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/latest/download/falcon-0.3.0-x86_64-linux.tar.gz \
  | tar -xz
sudo mv falcon /usr/local/bin/   # or anywhere on your PATH

falcon version
```

## Build from source

With a stable Rust toolchain:

```sh
# straight from the repository
cargo install --git https://github.com/JacobDevelops/dart_falcon dart_falcon

# or from a checkout
cargo build --release
./target/release/falcon check .
```

The package is named `dart_falcon`; the installed binary is `falcon`.

## Nix

The repository is also a Nix flake whose default package is the prebuilt static
binary (a fixed-output fetch of the release tarball — no compilation):

```nix
{
  inputs.falcon.url = "github:JacobDevelops/dart_falcon";
}
```

Reference `falcon.packages.${system}.default` in a devShell or package list, or
run it directly:

```sh
nix run github:JacobDevelops/dart_falcon -- check .
```

`packages.<system>.falcon` is the build-from-source escape hatch (crane + the
pinned toolchain). When no release has been published yet, `default` falls back
to the source build. The flake reads `nix/binaries.json`, which CI updates with
each release's tarball hashes.

## Editor extensions

falcon speaks LSP (`falcon lsp`); first-party extensions live in
[`extensions/`](../extensions):

- **VS Code** — `extensions/falcon-vscode`
- **Zed** — `extensions/falcon-zed`

Both launch the `falcon` binary from your `PATH`, so install falcon with any
channel above first.

## How releases work

1. A maintainer pushes a tag `vX.Y.Z`.
2. CI (`.github/workflows/release.yml`) builds all four platform binaries
   (static musl for Linux via `cargo-zigbuild`, native builds for macOS).
3. The binaries are packaged as `falcon-X.Y.Z-<system>.tar.gz` and attached to a
   GitHub Release.
4. CI computes each tarball's SRI hash and commits `nix/binaries.json` back to
   `main` for the flake's prebuilt packages.

## Next steps

See [configuration.md](./configuration.md) for configuring rules and options,
or run `falcon check .` — with no `falcon.json` present, every recommended rule
runs at its default severity.
