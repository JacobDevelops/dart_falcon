{
  description = "falcon — a fast Dart linter built in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Crane builds the cargo dependency graph ONCE (buildDepsOnly) and shares
    # those artifacts across the package and every cargo check, so
    # clippy/test/validate-rules only ever compile workspace code.
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
    }:
    let
      lib = nixpkgs.lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      # Prebuilt-binary manifest, maintained by CI (.github/workflows/release.yml):
      # each release commits nix/binaries.json mapping every system to the SRI
      # hash of its release tarball. Absent until the first release — the flake
      # must (and does) eval without it, so `falcon-bin`/`default = prebuilt` only
      # appear once a release has populated the file.
      manifest =
        if builtins.pathExists ./nix/binaries.json then
          builtins.fromJSON (builtins.readFile ./nix/binaries.json)
        else
          null;

      outputsFor =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = import ./nix/toolchain.nix { inherit pkgs; };
          craneLib = (crane.mkLib pkgs).overrideToolchain (_: toolchain.build);

          # Only what cargo needs: manifests, sources, and in-tree fixtures
          # (the rule corpus .dart/.json files live under crates/). Excludes
          # extensions/, docs, and the nix/devenv files themselves, so
          # unrelated edits never invalidate a cargo derivation.
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
              ./crates
              ./xtask
            ];
          };

          commonArgs = {
            inherit src;
            pname = "falcon";
            version = "0.2.1";
            strictDeps = true;
          };

          # Dependency-only artifacts (release, all targets so dev-deps like
          # criterion/insta are covered too). Invalidated only by manifest
          # changes — crane builds them against dummy sources.
          # (buildDepsOnly's internal `cargo check` already adds --all-targets.)
          cargoArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // {
              cargoExtraArgs = "--workspace";
            }
          );

          falcon = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              # Only the falcon binary ships; xtask stays a dev tool.
              cargoExtraArgs = "-p dart_falcon";
              # Tests run in the dedicated `test` check, not on package builds.
              doCheck = false;
            }
          );

          # Prebuilt binary for this system, when the CI manifest covers it.
          # It's a fixed-output fetchurl of the published release tarball, so it
          # compiles nothing and is immune to `inputs.nixpkgs.follows` overrides
          # (the hash pins the exact bytes). Lazy: never forced when hasBin is
          # false, so a null manifest can't break eval.
          hasBin = manifest != null && manifest.systems ? ${system};
          falcon-bin = pkgs.stdenvNoCC.mkDerivation {
            pname = "falcon-bin";
            version = manifest.version;
            src = pkgs.fetchurl {
              url = "https://github.com/JacobDevelops/dart_falcon/releases/download/v${manifest.version}/falcon-${manifest.version}-${system}.tar.gz";
              hash = manifest.systems.${system};
            };
            # Tarball holds a single top-level `falcon` file (not a directory),
            # so pin sourceRoot instead of letting unpackPhase guess.
            sourceRoot = ".";
            dontConfigure = true;
            dontBuild = true;
            # Static musl (linux) / self-contained (darwin) binary: keep patchelf
            # and strip away from it.
            dontStrip = true;
            installPhase = ''
              runHook preInstall
              install -Dm755 falcon $out/bin/falcon
              runHook postInstall
            '';
            meta.mainProgram = "falcon";
          };

          # nix-fmt check sees ONLY .nix files — editing Rust or docs never
          # re-runs it.
          nixSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.fileFilter (f: f.hasExt "nix") ./.;
          };
        in
        {
          # falcon: always available, built from source. falcon-bin: prebuilt,
          # only when the manifest covers this system. default prefers the
          # prebuilt binary (no compile) and falls back to source.
          packages = {
            inherit falcon;
            default = if hasBin then falcon-bin else falcon;
          }
          // lib.optionalAttrs hasBin { inherit falcon-bin; };

          checks = {
            build = falcon;

            # Formatting needs sources only — no dependency compilation at all.
            fmt = craneLib.cargoFmt (
              commonArgs
              // {
                cargoFmtExtraArgs = "--all";
              }
            );

            clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
              }
            );

            test = craneLib.cargoTest (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoTestExtraArgs = "--workspace";
                # The sandbox build dir is /build, which collides with the
                # `**/build/**` exclude pattern in file_walker's tests (the
                # tempdir's absolute path would match). /tmp is writable in
                # the sandbox and has no `build` path component.
                preBuild = ''
                  export TMPDIR=/tmp
                '';
              }
            );

            # Golden-corpus validation: compiles ONLY the small xtask crate on
            # top of the shared artifacts and points it at the already-built
            # falcon package (no second falcon compile, no debug-profile
            # rebuild).
            validate-rules = craneLib.mkCargoDerivation (
              commonArgs
              // {
                inherit cargoArtifacts;
                pnameSuffix = "-validate-rules";
                buildPhaseCargoCommand = ''
                  cargo run --locked --release --package xtask -- \
                    validate-rules --falcon-bin ${falcon}/bin/falcon
                '';
                doInstallCargoArtifacts = false;
                installPhaseCommand = "mkdir -p $out";
              }
            );

            nix-fmt = pkgs.runCommand "falcon-nix-fmt" { } ''
              ${pkgs.nixfmt}/bin/nixfmt --check ${nixSrc}
              touch $out
            '';

            # Fails if devenv.yaml's pins drift from flake.lock — drift silently
            # causes a second nixpkgs/toolchain eval and cache misses in the
            # dev shell. Depends only on flake.lock + devenv.yaml.
            devenv-pin = pkgs.runCommand "falcon-devenv-pin" { nativeBuildInputs = [ pkgs.jq ]; } ''
              check() {
                input=$1 prefix=$2
                lock_rev=$(${pkgs.jq}/bin/jq -r ".nodes.\"$input\".locked.rev" ${./flake.lock})
                yaml_rev=$(grep -oE "$prefix/[0-9a-f]{40}" ${./devenv.yaml} | grep -oE '[0-9a-f]{40}' | head -n1)
                if [ "$lock_rev" != "$yaml_rev" ]; then
                  echo "devenv.yaml $input rev ($yaml_rev) != flake.lock ($lock_rev)." >&2
                  echo "Set devenv.yaml inputs.$input to $prefix/$lock_rev (or run 'devenv update')." >&2
                  exit 1
                fi
                echo "$input pin matches flake.lock ($lock_rev)"
              }
              check nixpkgs "github:NixOS/nixpkgs"
              check rust-overlay "github:oxalica/rust-overlay"
              touch $out
            '';
          };
        };

      perSystem = lib.genAttrs systems outputsFor;
    in
    {
      packages = lib.mapAttrs (_: o: o.packages) perSystem;
      checks = lib.mapAttrs (_: o: o.checks) perSystem;
      # The dev environment is defined by devenv (devenv.nix / devenv.yaml);
      # this flake only builds and checks.
    };
}
