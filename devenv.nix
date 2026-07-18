{
  pkgs,
  inputs,
  ...
}:
let
  system = pkgs.stdenv.hostPlatform.system;

  # Reconstruct the flake's package set (same nixpkgs rev + rust-overlay, see
  # devenv.yaml) so the toolchain derivation is byte-identical to the flake's
  # and resolves from the binary cache instead of rebuilding.
  flakePkgs = import inputs.nixpkgs {
    inherit system;
    overlays = [ (import inputs.rust-overlay) ];
  };
  toolchain = import ./nix/toolchain.nix { pkgs = flakePkgs; };
in
{
  # ── Packages ────────────────────────────────────────────────────────────────
  # toolchain.dev = stable rust + rust-src/rust-analyzer + wasm32-wasip2 (for
  # the Zed extension in extensions/falcon-zed).
  packages = [
    toolchain.dev
  ]
  ++ (with pkgs; [
    cargo-watch
    cargo-nextest

    # Nix editing/formatting (nix-fmt script below, nixfmt flake check)
    nil
    nixfmt

    # Docs website (website/): Bun runtime + Cloudflare Workers deploy CLI.
    # `bun run deploy` in website/ shells out to wrangler.
    bun
    wrangler
  ]);

  # ── Environment ─────────────────────────────────────────────────────────────
  # rust-analyzer resolves the stdlib from the Nix toolchain.
  env.RUST_SRC_PATH = "${toolchain.dev}/lib/rustlib/src/rust/library";

  # ── Scripts ─────────────────────────────────────────────────────────────────
  scripts = {
    nix-fmt.exec = ''
      set -euo pipefail
      find "$DEVENV_ROOT" -name '*.nix' \
        -not -path '*/.git/*' -not -path '*/target/*' -not -path '*/.devenv/*' -print0 \
        | xargs -0 ${pkgs.nixfmt}/bin/nixfmt
    '';
  };
}
