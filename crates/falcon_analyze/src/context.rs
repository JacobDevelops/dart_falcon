use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};

use crate::resolve::{IdentityIndex, LibraryUnit, SignatureIndex, TypeIndex};

/// Per-file analysis context passed to every rule.
///
/// Resolving drivers attach declaration identities, type facts, and signatures.
/// Non-resolving callers leave those optional indexes absent.
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,
    pub source: &'a str,
    pub config: &'a FalconConfig,
    /// Cross-file type facts, when resolving analysis is enabled.
    pub types: Option<&'a TypeIndex>,
    /// Project-wide declaration identity index for rules that must distinguish
    /// type literals from values. Absent outside resolving analysis.
    pub identities: Option<&'a IdentityIndex>,
    /// Project-wide generic signatures and instantiated supertype facts.
    pub signatures: Option<&'a SignatureIndex>,
    /// This file's library context — its part/owner siblings and the flag for an
    /// unresolved part. `None` when resolution is off or the file stands alone.
    pub library: Option<&'a LibraryUnit<'a>>,
}

impl<'a> AnalyzeContext<'a> {
    /// Construct a non-resolving context.
    pub fn new(file_path: &'a std::path::Path, source: &'a str, config: &'a FalconConfig) -> Self {
        Self {
            file_path,
            source,
            config,
            types: None,
            identities: None,
            signatures: None,
            library: None,
        }
    }

    pub fn diagnostic(
        &self,
        rule: &'static str,
        message: impl Into<String>,
        span: impl Into<Span>,
    ) -> Diagnostic {
        Diagnostic::new(
            rule,
            Severity::Warning,
            message,
            self.file_path.to_string_lossy().into_owned(),
            span,
        )
    }

    /// Attach a cross-file type index.
    pub fn with_types(mut self, types: &'a TypeIndex) -> Self {
        self.types = Some(types);
        self
    }

    /// Attach the project declaration identity index.
    pub fn with_identities(mut self, identities: &'a IdentityIndex) -> Self {
        self.identities = Some(identities);
        self
    }

    /// Attach project-wide resolved signatures.
    pub fn with_signatures(mut self, signatures: &'a SignatureIndex) -> Self {
        self.signatures = Some(signatures);
        self
    }

    /// Attach this file's library context.
    pub fn with_library(mut self, library: &'a LibraryUnit<'a>) -> Self {
        self.library = Some(library);
        self
    }

    /// Resolve a rule's options for the file under analysis: the base options
    /// patched by any `overrides` entry whose `includes` match this file (see
    /// [`FalconConfig::rule_options_for`]). Rules should prefer this over
    /// [`FalconConfig::rule_options`] so per-path override options take effect.
    pub fn rule_options(&self, group: &str, rule_name: &str) -> Option<&serde_json::Value> {
        self.config
            .rule_options_for(&self.file_path.to_string_lossy(), group, rule_name)
    }
}
