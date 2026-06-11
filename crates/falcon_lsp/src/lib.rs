//! LSP server implementation (JSON-RPC 2.0, LSP 3.17).
//!
//! Single-threaded message loop over `lsp-server`; full-text document sync;
//! per-document AST cache with config-driven rule reloads. Architecture and
//! cache-invalidation rules: `.omc/docs/LSP_CACHING_DESIGN.md`.
//!
//! Handlers: initialize/initialized/shutdown, textDocument/didOpen,
//! didChange (debounced), didSave, didClose, textDocument/hover, and
//! workspace/didChangeWatchedFiles (falcon.json reload). Diagnostics are
//! published via textDocument/publishDiagnostics.

pub mod server;
pub mod state;

pub use server::{ServerOptions, run_server, run_with_connection};
pub use state::{DocumentState, LspState, uri_to_path};
