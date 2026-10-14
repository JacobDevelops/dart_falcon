# Falcon Dart Linter — VS Code Extension

Minimal language client for the [falcon](../../) Dart linter.
Launches `falcon lsp` over stdio and shows falcon diagnostics inline for
`.dart` files. Hovering a flagged range shows the rule id and message.

## Requirements

- The `falcon` binary on your `PATH` (see [installation](../../docs/installation.md)), or set `falcon.binaryPath`.

## Install locally

```sh
cd extensions/falcon-vscode
npm install          # pulls vscode-languageclient
npx @vscode/vsce package
code --install-extension falcon-vscode-0.1.0.vsix
```

Alternative for development: symlink this directory into
`~/.vscode/extensions/falcon-vscode` and reload VS Code, or open the repo in
VS Code and use `Run > Start Debugging` with an Extension Development Host.

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `falcon.binaryPath` | `falcon` | Path to the falcon binary |
| `falcon.trace.server` | `off` | Log LSP traffic (`messages` / `verbose`) |

## Configuration reloads

The extension watches `**/falcon.json`; saving rule config changes re-lints
all open Dart files immediately (no restart needed).

## Manual smoke test

1. `cargo build --release` in the falcon repo
2. Set `falcon.binaryPath` to `<repo>/target/release/falcon`
3. Open a `.dart` file in any Dart project → falcon diagnostics appear with source "falcon"
4. Introduce `dynamic x = 1;` → squiggle appears ~500ms after typing stops
5. Disable `avoid-dynamic` in falcon.json and save → squiggle disappears
