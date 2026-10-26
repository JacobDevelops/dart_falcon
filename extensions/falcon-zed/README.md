# Falcon Dart Linter — Zed Extension

Registers the [falcon](../../) linter's LSP server (`falcon lsp`) for Dart
buffers in Zed. Diagnostics appear inline with source "falcon"; hovering a
flagged range shows the rule id and message.

## Requirements

- The `falcon` binary on your `PATH` (see [installation](../../docs/installation.md)).
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

## Quick fixes and safe fixes on save

Open the code actions menu on a Falcon diagnostic to apply its individual quick
fix. Unsafe fixes are labeled **(unsafe)** and are never preferred. Falcon honors
the effective per-rule `fix` policy in `falcon.json`, including suppression and
safety overrides.

Run safe Falcon fixes on save using a code-action formatter in Zed's
`settings.json`:

```json
{
  "languages": {
    "Dart": {
      "formatter": [
        { "code_action": "source.fixAll.falcon" },
        "language_server"
      ],
      "format_on_save": "on"
    }
  }
}
```

The second formatter retains your Dart language server's formatting. If you
already use a formatter array, add the Falcon code action to that array.
See [Zed's formatting and linting documentation](https://zed.dev/docs/configuring-languages#integrating-formatting-and-linting).

Generic `source.fixAll` requests never include unsafe Falcon fix all. The
explicit **Fix all Falcon issues (unsafe)** action uses
`source.falcon.fixAllUnsafe`, outside the generic fix-all hierarchy; leave it out
of save settings unless you intend to allow behavior-changing edits.

Fix all iterates over the current buffer, and reports cycles, iteration limits,
or invalid edits without returning partial results. Actions include the buffer
version so supporting clients can reject stale edits. Cross-file fixes are not offered.


## Configuration reloads

The server reloads `falcon.json` when the editor reports it changed
(`workspace/didChangeWatchedFiles`); rule enable/disable takes effect on
open files without a restart.

## Manual smoke test

1. `cargo build --release` in the falcon repo; ensure `falcon` is on PATH
2. Install the dev extension and open a jfit `.dart` file
3. Introduce `dynamic x = 1;` → diagnostic appears ~500ms after typing stops
4. Disable `avoid-dynamic` in falcon.json and save → diagnostic disappears
