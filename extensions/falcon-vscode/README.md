# Falcon Dart Linter — VS Code Extension

Minimal language client for the [falcon](../../) Dart linter (Phase 1).
Launches `falcon lsp` over stdio and shows falcon diagnostics inline for
`.dart` files. Hovering a flagged range shows the rule id and message.

## Requirements

- The `falcon` binary on your `PATH` (`cargo build --release` →
  `target/release/falcon`, or `nix build`), or set `falcon.binaryPath`.

## Install (Phase 1 — local)

```sh
cd extensions/falcon-vscode
npm install          # pulls vscode-languageclient
npx @vscode/vsce package
code --install-extension falcon-vscode-0.1.0.vsix
```

Alternative for development: symlink this directory into
`~/.vscode/extensions/falcon-vscode` and reload VS Code, or open the repo in
VS Code and use `Run > Start Debugging` with an Extension Development Host.

Phase 2 target: publish to the VS Code Marketplace (auto-updates).

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `falcon.binaryPath` | `falcon` | Path to the falcon binary |
| `falcon.trace.server` | `off` | Log LSP traffic (`messages` / `verbose`) |

## Configuration reloads

The extension watches `**/falcon.json`; saving rule config changes re-lints
all open Dart files immediately (no restart needed).

## Manual smoke test (plan M5.3)

1. `cargo build --release` in the falcon repo
2. Set `falcon.binaryPath` to `<repo>/target/release/falcon`
3. Open any jfit `.dart` file → falcon diagnostics appear with source "falcon"
4. Introduce `dynamic x = 1;` → squiggle appears ~500ms after typing stops
5. Disable `avoid-dynamic` in falcon.json and save → squiggle disappears
