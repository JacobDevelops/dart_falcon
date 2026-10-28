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
3. An Ubuntu runner uses the pinned `rcodesign` 0.29.0 build to sign and verify
   the Apple Silicon binary, then submits it to Apple's notary service without
   waiting. The release workflow invokes `.github/workflows/notarisation.yml`
   as a reusable wait/check: its wait job uses the `notarisation` environment,
   then a Linux job checks the submission after about 10 minutes. If accepted,
   packaging and publishing continue in that same release run. If still
   pending, the workflow dispatches standalone polling attempts 2 through 6;
   each waits and checks on Linux. Rejected or invalid submissions fail. If
   still pending after six checks (about an hour total), rerun the poll workflow
   manually. No macOS or paid runner stays occupied while Apple processes the
   request.
4. Only an accepted submission allows packaging as
   `falcon-X.Y.Z-<system>.tar.gz` plus a versionless `falcon-<system>.tar.gz`
   copy for `releases/latest/download/` links, checksumming into `SHA256SUMS`,
   signing with Minisign, adding a build provenance attestation, and publishing
   on GitHub. CI removes the temporary signed-binary asset before publishing,
   then dispatches pub.dev publishing at the newly created tag and updates
   Homebrew. Existing pub.dev versions are skipped.

The signed binary is kept on the draft release as
`falcon-X.Y.Z-aarch64-darwin.signed`. On a rerun, CI reuses that exact binary
and its SHA-256 code-directory hash (CDHash) to find the named notarisation
submission, rather than signing or submitting it again. If the first delayed
check accepts, the original release run continues. Later polling attempts run
standalone; on acceptance they redispatch the release workflow with the same
immutable commit and tag. The commit must be on the default branch, and CI
confirms the draft still targets that commit. The tag is created only at publish
time. This process does not alter the existing pub.dev dispatch behaviour.

### Release publishing setup

Configure the required repository Actions secrets:

- **macOS signing and notarisation:** export a **Developer ID Application**
  certificate with its private key as a password-protected `.p12` file. Store
  its base64-encoded contents as `APPLE_CERTIFICATE_P12`, the export password
  as `APPLE_CERTIFICATE_PASSWORD`, and the team ID as `APPLE_TEAM_ID`. Create
  a **team** App Store Connect API key and set `APPLE_API_ISSUER_ID`,
  `APPLE_API_KEY_ID`, and `APPLE_API_KEY_P8` to its issuer ID, key ID, and raw
  `.p8` private-key contents, respectively. Set the key's role to **Developer**
  or higher. Apple documents team-key roles in its
  [API key guide](https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api)
  and use of App Store Connect API keys for notarisation in its
  [Notary API guide](https://developer.apple.com/documentation/notaryapi/submitting-software-for-notarization-over-the-web).
  CI signs with hardened runtime and a timestamp, verifies the binary, and
  obtains the CDHash from its code directory. A raw CLI binary cannot have a
  notarisation ticket stapled to it; the published archive contains the signed
  binary, and macOS may need network access to check the ticket.
- **Automatic polling:** create a GitHub Actions environment named
  `notarisation` with a **10-minute wait timer** and no required reviewers or
  deployment branch/tag restrictions that would block automatic workflow
  runs. Configure the environment timer in repository settings. The workflow
  makes up to six automatic status checks (about one hour total); after that,
  manually rerun the poll workflow.
- **Archive signing:** create a Minisign key pair (`minisign -G`). Store the
  complete secret key file as `MINISIGN_SECRET_KEY`. For a password-protected
  key, store its password as `MINISIGN_PASSWORD`; leave it unset for an
  unprotected key generated with `minisign -G -W`. Keep the secret key offline
  too, and distribute the public key through a trusted channel. Releases
  include `.minisig` signatures for every archive and `SHA256SUMS`. Verify
  them with `minisign -Vm <asset> -P <trusted-public-key>`.
- **Homebrew:** create the public repository `JacobDevelops/homebrew-tap` with
  an initial commit. Set `HOMEBREW_TAP_TOKEN` to a fine-grained PAT with
  **Contents: read and write** on that tap repository. The single Actions variable
  `HOMEBREW_TAP` controls the destination (default `JacobDevelops/homebrew-tap`);
  if changed, substitute that owner/tap in the install command above. Release CI
  generates `Formula/falcon.rb` from the published `SHA256SUMS`, then commits it
  through GitHub's contents API. Re-releasing an older tag does not roll the tap
  backwards. Prerelease and build-metadata versions do not update the tap.
  `HOMEBREW_TAP` is optional and only changes the destination.

`rcodesign` is pinned to 0.29.0, including the Linux musl release tarball's
SHA-256 checksum, so release signing uses a reproducible, verified tool build.
The workflow uses `rcodesign encode-app-store-connect-api-key` to create the
temporary API-key JSON and `rcodesign notary-list` to match existing submissions
by name and poll by submission ID.

To inspect a formula locally:

```sh
cargo xtask homebrew --version X.Y.Z --checksums SHA256SUMS --output falcon.rb
```


## Next steps

See [configuration.md](./configuration.md) for configuring rules and options,
or run `falcon check .`. With no `falcon.json` present, falcon's 215 recommended
rules out of 512 shipped rules run at their default severity.
