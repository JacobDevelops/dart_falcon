//! Use `lowercase_with_underscores` for package names.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics. This is a cross-file rule: it runs in the cross-file pass over
//! the whole analyzed file set and is configured under the top-level
//! `cross-file` section rather than `linter`.

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;

pub struct PackageNames;

impl CrossFileRule for PackageNames {
    fn name(&self) -> &'static str {
        "package-names"
    }

    fn analyze_project(&self, _files: &[ProjectFile], _config: &FalconConfig) -> Vec<Diagnostic> {
        Vec::new()
    }
}
