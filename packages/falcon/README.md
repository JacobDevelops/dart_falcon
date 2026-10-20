# falcon

A launcher for [falcon](https://github.com/JacobDevelops/dart_falcon), a native Dart linter.

```sh
dart pub global activate falcon
falcon --help
```

Add the Dart pub cache's `bin` directory to your `PATH` if the `falcon` command is not found.

On first run, the launcher downloads the matching GitHub release, verifies its SHA256 checksum, and caches the native binary. Later runs use the cached binary. Internet access and the system `tar` and `chmod` commands are required for the first run.

Supports Linux and macOS on x86_64 and arm64. Windows is not supported.

See the [installation guide](https://dart-falcon.jacobdevelops.com/docs/installation) for other installation methods.
