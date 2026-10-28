# Installation

falcon ships as a single binary. It doesn't run the Dart analyzer or analysis server. Install the Dart SDK and resolve your dependencies for rules that need SDK or package types.

## Supported platforms

| Platform            | Binary                        |
| ------------------- | ----------------------------- |
| Linux x86_64        | fully static (musl)           |
| Linux aarch64       | fully static (musl)           |
| macOS Apple Silicon | native, macOS 11.7.1 or later |

Windows binaries are not published yet. Windows users can
[build from source](#build-from-source) or use WSL.

## Prebuilt binaries (recommended)

Every release attaches platform tarballs to the
[GitHub Release](https://github.com/JacobDevelops/dart_falcon/releases). Each
tarball contains a single `falcon` executable.

```sh
# Substitute your platform:
#   x86_64-linux | aarch64-linux | aarch64-darwin
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

The checksum manifest comes from the same GitHub release as the archive. Matching
it checks integrity, not publisher authenticity if both assets are replaced.
Provenance verification is a separate check, as shown above.

Every release publishes Minisign signatures made with the publisher key
`RWRlvxXpBM715oZAT+91h6njpynlhlrZcueq2mV2yr1gfuc2B0mnmRwm` (key ID
`E6F5CE04E915BF65`). Download the archive's matching `.minisig` and verify it
before extracting:

```sh
minisign -Vm falcon-x86_64-linux.tar.gz -P RWRlvxXpBM715oZAT+91h6njpynlhlrZcueq2mV2yr1gfuc2B0mnmRwm
```

Substitute your platform's archive name. The same command can verify `SHA256SUMS`
against `SHA256SUMS.minisig`. The launcher does not verify signatures.

## Dart pub

On Linux and macOS, install falcon from pub.dev:

```sh
dart pub global activate falcon
```

The first invocation downloads the matching native binary for Linux on x86_64 or
arm64, or macOS on arm64. Make sure Dart's pub cache `bin` directory is on your
`PATH` to run `falcon`.

The pub launcher checks the archive's SHA256 checksum against `SHA256SUMS`
downloaded from the same GitHub release before installing into a versioned user
cache. This is an integrity check, not publisher authentication. It does not
verify Minisign signatures or GitHub provenance attestations. It supports
updating the native binary without reactivating the Dart package:

```sh
falcon self-update          # latest GitHub Release
falcon self-update X.Y.Z    # select a published version
FALCON_VERSION=X.Y.Z falcon check .
```

`self-update` is a pub-launcher command, not a command in the standalone native
binary. A successful update selects that version for future invocations of the
same pub launcher version. Upgrading or downgrading the Dart package ignores
selections saved by a different launcher version, including legacy selections
without a launcher version, and defaults to the newly installed package version.
Run `self-update` again to select a different native release.
`FALCON_VERSION` always overrides the selected version; an explicit `self-update`
version overrides the environment. Without either selection, the launcher uses
its pub package version. An update with `FALCON_VERSION` set and no explicit
version updates that pinned version instead of fetching latest. Downloads are
checked before atomic installation; a checksum failure leaves the installed
binary and version selection unchanged. Update the Dart wrapper itself with
`dart pub global activate falcon`.

## Homebrew

After the maintainer has enabled [tap publishing](#release-publishing-setup):

```sh
brew install JacobDevelops/tap/falcon
brew upgrade falcon
```

The tap ships the same checksummed Linux and macOS binaries as GitHub Releases.

## cargo-binstall

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) installed:

```sh
cargo binstall --git https://github.com/JacobDevelops/dart_falcon dart_falcon
```

The package is not published to crates.io, so `--git` supplies its manifest.
The manifest maps Linux GNU and musl targets to the static Linux release assets
and macOS arm64 to its native asset. It disables compilation and third-party
QuickInstall fallbacks. To install from a particular release, use a checkout of
that release with `cargo binstall --manifest-path Cargo.toml dart_falcon`.

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
"github:JacobDevelops/dart_falcon" = { version = "0.4.6", matching = "falcon-" }
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

The language server analyzes the folders supplied by the editor, falling back to
`rootUri` and then its working directory when no workspace folders are supplied.
Configuration is session-wide: an explicit config takes precedence, otherwise
discovery starts in the first workspace folder. Configuration errors appear as
editor messages. Diagnostics use UTF-16 positions and combine file and project
findings in a single publication.

Workspace parses are cached between edits. Clients should send
`workspace/didChangeWatchedFiles` events for disk changes so newly created,
changed, or removed files are included without restarting the server.

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

1. A maintainer merges the release PR, pushes a tag `vX.Y.Z`, or dispatches the
   **Release** workflow. Dispatch without `tag` releases the workspace version
   at the selected ref; supplying `tag` reruns an existing tagged version.
2. CI (`.github/workflows/release.yml`) cross-builds all three platform binaries
   on Linux with `cargo-zigbuild` (static musl for Linux, zig-linked macOS).
3. A macOS runner signs and verifies the Apple Silicon binary, saves it on a
   draft release, and submits it to Apple without waiting. A pending submission
   fails the job with its ID in the summary, releasing the runner. Check its
   status and rerun once Apple accepts it. Reruns reuse the saved signed binary
   and submission, and refuse a different source commit for the same draft.
4. Only accepted binaries are packaged as `falcon-X.Y.Z-<system>.tar.gz` plus a
   versionless `falcon-<system>.tar.gz` copy for `releases/latest/download/`
   links, checksummed into `SHA256SUMS`, signed with Minisign, given a build
   provenance attestation, and published on GitHub. CI removes the temporary
   signed-binary asset before publishing, then dispatches pub.dev publishing
   at the newly created tag and updates Homebrew. Existing pub.dev versions
   are skipped.

### Release publishing setup

Releases require all of these secrets. The release fails before building and
lists any that are missing.

- **Archive signing:** create a Minisign key pair (`minisign -G`). Store the
  complete secret key file as the Actions secret `MINISIGN_SECRET_KEY`. For a
  password-protected key, store its password as `MINISIGN_PASSWORD`; leave it
  unset for an unprotected key generated with `minisign -G -W`. Keep the secret
  key offline too, and distribute the public key through a trusted channel.
  Releases include `.minisig` signatures for every archive and `SHA256SUMS`.
  Verify them with `minisign -Vm <asset> -P <trusted-public-key>`.
- **macOS signing and notarisation:** export a **Developer ID Application**
  certificate with its private key as a password-protected `.p12` file. Store
  its base64-encoded contents as `APPLE_CERTIFICATE_P12` and the export password
  as `APPLE_CERTIFICATE_PASSWORD`. Set `APPLE_ID` to the Apple Developer account
  email, `APPLE_TEAM_ID` to its team ID, and `APPLE_APP_PASSWORD` to an
  app-specific password for that account.
  CI imports the certificate into a temporary keychain and signs the Darwin
  binary with the hardened runtime and a timestamp. Submission names include
  the version, platform and CDHash. Matching accepted submissions are reused;
  in-progress ones fail as pending without resubmitting, and invalid or
  rejected ones fail the release. Check status with `xcrun notarytool history`
  or `xcrun notarytool info <id>`, passing `--apple-id "$APPLE_ID" --team-id
  "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD"`. Rerun the failed jobs or
  redispatch at the original ref; supply `tag` only if it already exists.
  Tagless drafts do not create a tag until publishing. The release tarballs
  contain the signed binaries; published reruns recover them from the versioned
  tarball. Bare CLI binaries cannot have a ticket stapled to them, so the zip
  is only a submission container; users may need network access for macOS to
  check the notarisation ticket.
- **Homebrew:** create the public repository `JacobDevelops/homebrew-tap` with
  an initial commit. Set `HOMEBREW_TAP_TOKEN` to a fine-grained PAT with
  **Contents: read and write** on that tap repository. The single Actions variable
  `HOMEBREW_TAP` controls the destination (default `JacobDevelops/homebrew-tap`);
  if changed, substitute that owner/tap in the install command above. Release CI
  generates `Formula/falcon.rb` from the published `SHA256SUMS`, then commits it
  through GitHub's contents API. Re-releasing an older tag does not roll the tap
  backwards. Prerelease and build-metadata versions do not update the tap.
  `HOMEBREW_TAP` is optional and only changes the destination.

To inspect a formula locally:

```sh
cargo xtask homebrew --version X.Y.Z --checksums SHA256SUMS --output falcon.rb
```

## Next steps

See [configuration.md](./configuration.md) for configuring rules and options,
or run `falcon check .`. With no `falcon.json` present, falcon's 215 recommended
rules out of 512 shipped rules run at their default severity.
