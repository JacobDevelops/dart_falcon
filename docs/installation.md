# Installation

falcon ships as a single binary. It doesn't run the Dart analyzer or analysis server. Install the Dart SDK and resolve your dependencies for rules that need SDK or package types.

## Supported platforms

| Platform            | Binary                        |
| ------------------- | ----------------------------- |
| Linux x86_64        | fully static (musl)           |
| Linux aarch64       | fully static (musl)           |
| macOS Intel         | native, macOS 11.7.1 or later |
| macOS Apple Silicon | native, macOS 11.7.1 or later |

Windows binaries are not published yet. Windows users can
[build from source](#build-from-source) or use WSL.

## Prebuilt binaries (recommended)

Every release attaches platform tarballs to the
[GitHub Release](https://github.com/JacobDevelops/dart_falcon/releases). Each
tarball contains a single `falcon` executable.

```sh
# Substitute your platform:
#   x86_64-linux | aarch64-linux | x86_64-darwin | aarch64-darwin
curl -fsSL https://github.com/JacobDevelops/dart_falcon/releases/latest/download/falcon-x86_64-linux.tar.gz \
  | tar -xz
sudo mv falcon /usr/local/bin/   # or anywhere on your PATH

falcon version
```

Each release also publishes `SHA256SUMS` and a GitHub build provenance
attestation for every tarball. To verify a download before extracting it:

```sh
base=https://github.com/JacobDevelops/dart_falcon/releases/latest/download
curl -fsSLO "$base/falcon-x86_64-linux.tar.gz" -O "$base/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify falcon-x86_64-linux.tar.gz --repo JacobDevelops/dart_falcon
```

The mise GitHub backend runs these checks automatically.

## Build from source

With a nightly Rust toolchain. On x86_64 GNU/Linux, building the bundled jemalloc
also requires a native C compiler, a POSIX shell, and make:

```sh
# straight from the repository
cargo +nightly install --git https://github.com/JacobDevelops/dart_falcon dart_falcon

# or from a checkout
cargo build --release
./target/release/falcon check .
```

The package is named `dart_falcon`; the installed binary is `falcon`.

## mise

Add the GitHub release backend to your project's `mise.toml`:

```toml
[tools]
"github:JacobDevelops/dart_falcon" = { version = "0.4.2", matching = "falcon-" }
```

Install the pinned release and run it:

```sh
mise install
mise exec -- falcon check .
```

With [mise activated](https://mise.jdx.dev/getting-started.html), `falcon` is available directly on your `PATH`. The GitHub backend downloads the release archive for your platform; it doesn't compile Rust.

## Editor extensions

falcon speaks LSP (`falcon lsp`); first-party extensions live in
[`extensions/`](../extensions):

- **VS Code**: `extensions/falcon-vscode`
- **Zed**: `extensions/falcon-zed`

Both launch the `falcon` binary from your `PATH`, so install falcon with any
channel above first.

### Neovim

Neovim 0.11+ needs no plugin. Add this to your config:

```lua
vim.lsp.config('falcon', {
  cmd = { 'falcon', 'lsp' },
  filetypes = { 'dart' },
  -- only start in projects that have a falcon.json
  root_dir = function(bufnr, on_dir)
    local root = vim.fs.root(bufnr, 'falcon.json')
    if root then
      on_dir(root)
    end
  end,
  -- apply falcon.json edits without restarting the server
  capabilities = { workspace = { didChangeWatchedFiles = { dynamicRegistration = true } } },
})
vim.lsp.enable('falcon')
```

On Linux and BSD, install `inotify-tools` as well. Without `inotifywait`, Neovim
falls back to watching every directory under the project separately.

## How releases work

1. A maintainer pushes a tag `vX.Y.Z`.
2. CI (`.github/workflows/release.yml`) cross-builds all four platform binaries
   on Linux with `cargo-zigbuild` (static musl for Linux, zig-linked macOS).
3. The binaries are packaged as `falcon-X.Y.Z-<system>.tar.gz` plus a
   versionless `falcon-<system>.tar.gz` copy for `releases/latest/download/`
   links, checksummed into `SHA256SUMS`, given a build provenance attestation,
   and attached to a GitHub Release.

## Next steps

See [configuration.md](./configuration.md) for configuring rules and options,
or run `falcon check .`. With no `falcon.json` present, falcon's 170 recommended
rules out of 310 shipped rules run at their default severity.
