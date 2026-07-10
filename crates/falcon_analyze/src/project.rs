//! Project-level (cross-file) rule infrastructure.
//!
//! Unlike [`crate::Rule`], which sees one file at a time, a [`ProjectRule`]
//! receives every analyzed file's parsed [`Program`] at once so it can reason
//! about references that span files (unused files, unused public API, call-site
//! nullability). Project rules are CLI-only: the LSP analyzes a single open
//! buffer and has no whole-project view, so it never runs them.

use std::path::PathBuf;

use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

/// One analyzed file, retained with its parsed program for the project pass.
///
/// The per-file pass normally drops each [`Program`] after analysis; these are
/// only collected when at least one project rule is enabled.
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
pub trait ProjectRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze_project(&self, files: &[ProjectFile], config: &FalconConfig) -> Vec<Diagnostic>;
}

/// Registry of enabled project rules.
#[derive(Default)]
pub struct ProjectRuleRegistry {
    rules: Vec<Box<dyn ProjectRule>>,
}

impl ProjectRuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn ProjectRule>) {
        self.rules.push(rule);
    }

    pub fn rules(&self) -> &[Box<dyn ProjectRule>] {
        &self.rules
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Run every registered project rule over `files` and combine diagnostics.
    /// Inline suppression and per-path severity resolution are applied by the
    /// caller (the CLI pipeline), exactly as for the per-file pass.
    pub fn run_all(&self, files: &[ProjectFile], config: &FalconConfig) -> Vec<Diagnostic> {
        self.rules
            .iter()
            .flat_map(|rule| rule.analyze_project(files, config))
            .collect()
    }
}
