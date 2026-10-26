//! LSP server implementation (JSON-RPC 2.0, LSP 3.17).
//!
//! Responsive transport with a dedicated analysis-state worker; full-text document
//! sync and per-document CST caches with config-driven rule reloads.
//!
//! Handlers: initialize/initialized/shutdown, textDocument/didOpen,
//! didChange (debounced), didSave, didClose, textDocument/hover, and
//! workspace/didChangeWatchedFiles (falcon.json reload). Diagnostics are
//! published once per file after each merged analysis round. Newer generations
//! retain only the latest pending result per URI until that file is current;
//! unrelated results remain publishable and requests support cancellation.

pub mod server;
pub mod state;

pub use server::{ServerOptions, run_server, run_with_connection};
pub use state::LspState;
