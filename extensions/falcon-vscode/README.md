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

## Quick fixes and fix on save

Put the cursor on a Falcon diagnostic and choose **Quick Fix** (`Ctrl+.` /
`Cmd+.`). Diagnostics with an available fix offer an individual action. Unsafe
fixes are labeled **(unsafe)** and are never preferred; the effective `fix`
policy in `falcon.json` can suppress or reclassify a rule's fixes.

Enable safe fixes on explicit save in VS Code's `settings.json`:

```json
{
  "[dart]": {
    "editor.codeActionsOnSave": {
      "source.fixAll.falcon": "explicit"
    }
  }
}
```

Use `"always"` instead of `"explicit"` to include supported Auto Saves.
Generic `"source.fixAll"` also includes only Falcon's safe fixes. Unsafe fix all
is a separate action, **Fix all Falcon issues (unsafe)**, with kind
`source.falcon.fixAllUnsafe`; it is deliberately outside `source.fixAll` and
should not be enabled on save without reviewing the behavior changes.

Fix all processes the current open buffer iteratively. If fixes cycle, hit the
iteration limit, or contain invalid edits, Falcon reports an error rather than
returning partially fixed text. Actions include the buffer version so supporting
clients can reject stale edits. Cross-file fixes are not offered.

See [VS Code's Code Actions on save documentation](https://code.visualstudio.com/docs/editing/refactoring#_code-actions-on-save)
for the editor's save modes.


## Configuration reloads

The extension watches `**/falcon.json`; saving rule config changes re-lints
all open Dart files immediately (no restart needed).

## Manual smoke test

1. `cargo build --release` in the falcon repo
2. Set `falcon.binaryPath` to `<repo>/target/release/falcon`
3. Open a `.dart` file in any Dart project → falcon diagnostics appear with source "falcon"
4. Introduce `dynamic x = 1;` → squiggle appears ~500ms after typing stops
5. Disable `avoid-dynamic` in falcon.json and save → squiggle disappears
