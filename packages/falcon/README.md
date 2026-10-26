# falcon

A launcher for [falcon](https://github.com/JacobDevelops/dart_falcon), a native Dart linter.

```sh
dart pub global activate falcon
falcon --help
```

Add the Dart pub cache's `bin` directory to your `PATH` if the `falcon` command is not found.

On first run, the launcher downloads the matching GitHub release, verifies its SHA256 checksum, and caches the native binary. Later runs use the cached binary. Internet access and the system `tar` and `chmod` commands are required for the first run.

Update or select the native release without reinstalling the launcher:

```sh
falcon self-update          # latest GitHub Release
falcon self-update X.Y.Z    # select a published release
FALCON_VERSION=X.Y.Z falcon check .
```

Successful updates save the selected version for later runs of the same launcher
package version. Upgrading or downgrading the launcher ignores the previous
selection and defaults to the newly installed package version; legacy selections
without a launcher version are also ignored. Run `self-update` again to select a
different native release. `FALCON_VERSION` always overrides the saved selection;
an explicit `self-update` version overrides the environment for that update.
Otherwise, `self-update` honors the environment pin instead of fetching latest.
Before any update, the launcher verifies the archive against the release's
`SHA256SUMS`, then atomically replaces the cached binary. A checksum failure
leaves the existing binary and selection unchanged. Without a selection or
environment pin, the launcher uses its package version.

`self-update` belongs to this launcher, not the standalone native binary.
Update the launcher itself with `dart pub global activate falcon`.

Supports Linux and macOS on x86_64 and arm64. Windows is not supported.

See the [installation guide](https://dart-falcon.jacobdevelops.com/docs/installation) for other installation methods.
