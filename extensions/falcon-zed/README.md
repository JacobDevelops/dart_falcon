# Falcon Dart Linter — Zed Extension

Registers the [falcon](../../) linter's LSP server (`falcon lsp`) for Dart
buffers in Zed. Diagnostics appear inline with source "falcon"; hovering a
flagged range shows the rule id and message.

## Requirements

- The `falcon` binary on your `PATH` (`cargo build --release` →
  `target/release/falcon`, or `nix build`).
- The **Dart** Zed extension (provides the Dart language this server
  attaches to).

## Install (Phase 1 — dev extension)

1. Open Zed → `zed: extensions` (or `Cmd/Ctrl-Shift-X`)
2. Click **Install Dev Extension** and select this directory
   (`extensions/falcon-zed`). Zed compiles the extension to WASM itself —
   no manual build step needed.
3. Open a `.dart` file; falcon diagnostics appear alongside any other
   language servers.

Phase 2 target: publish to the Zed extension registry.

## Settings

Zed's standard per-server override selects the binary if it is not on PATH:

```jsonc
// settings.json
{
  "lsp": {
    "falcon": {
      "binary": { "path": "/path/to/falcon", "arguments": ["lsp"] }
    }
  }
}
```

## Configuration reloads

The server reloads `falcon.json` when the editor reports it changed
(`workspace/didChangeWatchedFiles`); rule enable/disable takes effect on
open files without a restart.

## Manual smoke test

1. `cargo build --release` in the falcon repo; ensure `falcon` is on PATH
2. Install the dev extension and open a jfit `.dart` file
3. Introduce `dynamic x = 1;` → diagnostic appears ~500ms after typing stops
4. Disable `avoid-dynamic` in falcon.json and save → diagnostic disappears
