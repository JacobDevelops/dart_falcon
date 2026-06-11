# Rust toolchains, shared by flake.nix and devenv.nix so both resolve the SAME
# derivations (one source of truth, and devenv hits the binary cache instead of
# re-evaluating a different toolchain).
#
# `pkgs` must carry the rust-overlay (rust-bin).
{ pkgs }:
{
  # Minimal toolchain for nix builds and checks. The stable default profile
  # already bundles cargo/rustc/clippy/rustfmt — no rust-src, rust-analyzer,
  # or wasm targets in the check closure.
  build = pkgs.rust-bin.stable.latest.default;

  # Full dev toolchain for the devenv shell: IDE support (rust-src,
  # rust-analyzer) plus wasm32-wasip2 for the Zed extension
  # (extensions/falcon-zed).
  dev = pkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-src"
      "rust-analyzer"
    ];
    targets = [ "wasm32-wasip2" ];
  };
}
