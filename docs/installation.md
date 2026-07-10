# Installation

falcon is distributed as a Nix flake. The default package is a **prebuilt static
binary** fetched from GitHub Releases — no Rust toolchain, no compilation.

## Supported systems

| System           | Binary                                   |
| ---------------- | ---------------------------------------- |
| `x86_64-linux`   | fully static (musl)                      |
| `aarch64-linux`  | fully static (musl)                      |
| `x86_64-darwin`  | native macOS (Intel)                     |
| `aarch64-darwin` | native macOS (Apple Silicon)             |

## Use it as a flake input

```nix
{
  inputs.falcon.url = "github:JacobDevelops/dart_falcon"; # once public
  # While the repo is private, use SSH instead:
  # inputs.falcon.url = "git+ssh://git@github.com/JacobDevelops/dart_falcon";
}
```

Then reference `falcon.packages.${system}.default` — for example in a devShell or
a package list:

```nix
devShells.default = pkgs.mkShell {
  packages = [ falcon.packages.${system}.default ];
};
```

Or run it directly without installing:

```sh
nix run github:JacobDevelops/dart_falcon
```

## Packages

- **`packages.<system>.default`** — the prebuilt static binary fetched from the
  GitHub Release for the flake's pinned version. This is a fixed-output
  `fetchurl` (the manifest pins each tarball's SRI hash), so it downloads the
  exact published bytes and runs no build. Because it's a fixed-output fetch, it
  is **immune to `inputs.nixpkgs.follows` overrides** — overriding a consumer's
  nixpkgs cannot change or rebuild the binary you get.
- **`packages.<system>.falcon`** — the build-from-source escape hatch (crane +
  the pinned Rust toolchain). Use this if you need to build against a specific
  nixpkgs, patch the source, or run on a system without a published binary.

When no release has been published yet, `default` transparently falls back to the
source build.

## Private-repo caveat (temporary)

While `JacobDevelops/dart_falcon` is private, fetching release **assets** requires
authentication that Nix does not provide out of the box. Until the repo is public,
private consumers should either:

- use the source package `packages.<system>.falcon` (clones over your existing
  git/SSH auth), or
- add a GitHub PAT to `~/.netrc` (or `netrc-file` in `nix.conf`) with
  `access-tokens`/machine `github.com` so `fetchurl` can authenticate against the
  release asset.

Once the repository is public this Just Works with no auth.

## How releases work

1. A maintainer pushes a tag `vX.Y.Z`.
2. CI (`.github/workflows/release.yml`) builds all four platform binaries
   (static musl for Linux via `cargo-zigbuild`, native builds for macOS).
3. The binaries are packaged as `falcon-X.Y.Z-<system>.tar.gz` and attached to a
   GitHub Release.
4. CI computes each tarball's SRI hash and commits `nix/binaries.json` back to
   `main`. That manifest is what the flake reads to define the prebuilt packages.

## Configuring the linter

See [configuration.md](./configuration.md) for how to configure falcon's rules
and options.
