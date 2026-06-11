//! Falcon Zed extension: registers `falcon lsp` as a language server for
//! Dart buffers (Phase 1, plan M5.3).
//!
//! The binary is resolved from the worktree's PATH; users can override it in
//! Zed settings via the standard `lsp.falcon.binary` mechanism, which Zed
//! core applies before this command is consulted.

use zed_extension_api::{self as zed, LanguageServerId, Result};

struct FalconExtension;

impl zed::Extension for FalconExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let command = worktree.which("falcon").ok_or_else(|| {
            "falcon binary not found on PATH — build it with `cargo build --release` \
             and add it to PATH, or set `lsp.falcon.binary.path` in Zed settings"
                .to_string()
        })?;
        Ok(zed::Command {
            command,
            args: vec!["lsp".to_string()],
            env: Vec::new(),
        })
    }
}

zed::register_extension!(FalconExtension);
