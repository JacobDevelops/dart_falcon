//! LSP server implementation (JSON-RPC 2.0, LSP 3.17).
//!
//! Handles initialize, textDocument/didOpen, textDocument/didChange,
//! and publishes diagnostics via textDocument/publishDiagnostics.

pub mod server;

pub use server::run_server;
