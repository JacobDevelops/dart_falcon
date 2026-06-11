// Falcon VS Code extension: minimal LSP client (Phase 1, plan M5.3).
//
// Launches `falcon lsp` over stdio for Dart documents and surfaces the
// diagnostics it publishes. falcon.json edits are forwarded to the server
// via workspace/didChangeWatchedFiles so rule config reloads live.

const { workspace } = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

let client;

function activate(context) {
  const binaryPath =
    workspace.getConfiguration('falcon').get('binaryPath') || 'falcon';

  const serverOptions = {
    command: binaryPath,
    args: ['lsp'],
  };

  const clientOptions = {
    documentSelector: [{ scheme: 'file', language: 'dart' }],
    synchronize: {
      // The server reloads falcon.json (and re-lints open files) on change.
      fileEvents: workspace.createFileSystemWatcher('**/falcon.json'),
    },
  };

  client = new LanguageClient(
    'falcon',
    'Falcon Dart Linter',
    serverOptions,
    clientOptions
  );

  context.subscriptions.push({ dispose: () => client && client.stop() });
  client.start();
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
