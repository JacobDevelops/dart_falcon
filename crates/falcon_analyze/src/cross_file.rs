//! Cross-file rule infrastructure.
//!
//! Unlike [`crate::Rule`], which sees one file at a time, a [`CrossFileRule`]
//! receives every analyzed file's parsed [`Program`] at once so it can reason
//! about references that span files (unused files, unused public API, call-site
//! nullability). Both the CLI and the LSP run them: the CLI over its walked
//! file set, the LSP over the workspace (open buffers overlaid on disk),
//! triggered on didOpen/didSave/config-reload rather than on every keystroke.

use std::path::PathBuf;

use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

/// One analyzed file, retained with its parsed program for the cross-file pass.
///
/// The per-file pass normally drops each [`Program`] after analysis; these are
/// only collected when at least one cross-file rule is enabled.
pub struct ProjectFile {
    pub path: PathBuf,
    pub source: String,
    pub program: Program,
    /// Whether the parse produced any errors. Rules that inspect a file's own
    /// declarations (unused-code, unnecessary-nullable) skip such files, since
    /// error recovery can leak spurious top-level nodes; the file still counts
    /// toward cross-file usage/reference detection (over-inclusion is safe).
    pub has_parse_errors: bool,
}

/// A rule that analyzes the whole set of files together.
///
/// Thread safety mirrors [`crate::Rule`]: implementors are immutable and must
/// not use mutable `self` state.
pub trait CrossFileRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze_project(&self, files: &[ProjectFile], config: &FalconConfig) -> Vec<Diagnostic>;
}

/// Registry of enabled cross-file rules.
#[derive(Default)]
pub struct CrossFileRuleRegistry {
    rules: Vec<Box<dyn CrossFileRule>>,
}

impl CrossFileRuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn CrossFileRule>) {
        self.rules.push(rule);
    }

    pub fn rules(&self) -> &[Box<dyn CrossFileRule>] {
        &self.rules
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Run every registered cross-file rule over `files` and combine diagnostics.
    /// Inline suppression and per-path severity resolution are applied by the
    /// caller (the CLI pipeline), exactly as for the per-file pass.
    pub fn run_all(&self, files: &[ProjectFile], config: &FalconConfig) -> Vec<Diagnostic> {
        crate::registry::with_rules_stack(|| {
            self.rules
                .iter()
                .flat_map(|rule| rule.analyze_project(files, config))
                .collect()
        })
    }
}
