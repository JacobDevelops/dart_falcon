use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};
use falcon_syntax::ast::{FunctionKind, Program, TopLevelDecl};
use falcon_syntax::token::{SyntaxStore, Token};

use crate::environment::{AnalysisEnvironment, LanguageVersion};
use crate::resolve::flow_writes::FlowWrites;
use crate::resolve::lexical_usage::{LocalWrite, local_writes};
use crate::resolve::{
    IdentityIndex, LibraryUnit, ResolvedType, SemanticModel, SignatureIndex, TypeIndex,
    TypeParameterScope,
};

/// Lazily computed facts shared by all rules analyzing one file.
#[derive(Default)]
pub struct AnalysisCache {
    flow_writes: OnceLock<Arc<FlowWrites>>,
    local_writes: OnceLock<LocalWrites>,
    globals: OnceLock<HashMap<String, ResolvedType>>,
}

struct LocalWrites {
    writes: Vec<LocalWrite>,
    mutated: HashSet<usize>,
}

/// Per-file analysis context passed to every rule.
///
/// Resolving drivers attach declaration identities, type facts, and signatures.
/// Non-resolving callers leave those optional indexes absent.
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,
    pub source: &'a str,
    pub syntax: &'a SyntaxStore,
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
    /// Package configurations, language versions and the SDK of the run.
    /// `None` means no package configuration or SDK is known.
    pub environment: Option<&'a AnalysisEnvironment>,
    /// The file's package `languageVersion`, cached by
    /// [`Self::with_environment`]; `None` when no package configuration
    /// covers the file. [`Self::language_version`] prefers the file's
    /// `// @dart` override, then this, then the analyzer's default.
    pub package_language_version: Option<LanguageVersion>,
    /// Per-file facts; create a fresh cache when constructing a context for another file.
    pub cache: AnalysisCache,
}

impl<'a> AnalyzeContext<'a> {
    /// Construct a non-resolving context.
    pub fn new(
        file_path: &'a std::path::Path,
        source: &'a str,
        config: &'a FalconConfig,
        syntax: &'a SyntaxStore,
    ) -> Self {
        Self {
            file_path,
            source,
            syntax,
            config,
            types: None,
            identities: None,
            signatures: None,
            library: None,
            environment: None,
            package_language_version: None,
            cache: AnalysisCache::default(),
        }
    }

    /// Lexical writes collected once for this file's parsed program.
    pub fn flow_writes(&self, program: &Program) -> &FlowWrites {
        self.cache.flow_writes.get_or_init(|| {
            self.identities
                .and_then(|identities| identities.file(self.file_path))
                .and_then(|file| self.signatures?.flow_writes(file))
                .cloned()
                .unwrap_or_else(|| Arc::new(FlowWrites::program(program)))
        })
    }

    pub fn local_writes(&self, program: &Program) -> &[LocalWrite] {
        &self.lexical_writes(program).writes
    }

    pub fn mutated_declarations(&self, program: &Program) -> &HashSet<usize> {
        &self.lexical_writes(program).mutated
    }

    fn lexical_writes(&self, program: &Program) -> &LocalWrites {
        self.cache.local_writes.get_or_init(|| {
            let writes = local_writes(program);
            let mutated = writes.iter().map(|write| write.declaration).collect();
            LocalWrites { writes, mutated }
        })
    }

    /// Top-level variable and setter types shared by semantic rule visitors.
    pub fn globals(&self, program: &Program) -> &HashMap<String, ResolvedType> {
        self.cache.globals.get_or_init(|| {
            let mut globals = HashMap::new();
            let Some(identities) = self.identities else {
                return globals;
            };
            let model = SemanticModel::new(self.file_path, identities, self.types);
            for declaration in &program.declarations {
                match declaration {
                    TopLevelDecl::Variable(variable) => {
                        let declared = variable.var_type.as_ref().map(|ty| model.resolve_type(ty));
                        for declarator in &variable.declarators {
                            globals.insert(
                                declarator.name.name.clone(),
                                declared.clone().unwrap_or(ResolvedType::Unknown),
                            );
                        }
                    }
                    TopLevelDecl::Function(function) => {
                        if let FunctionKind::Setter(parameters) = &function.kind
                            && let Some(parameter) = parameters.params.first()
                        {
                            globals.insert(
                                function.name.name.clone(),
                                model.resolve_formal_parameter_in(
                                    parameter,
                                    &TypeParameterScope::default(),
                                ),
                            );
                        }
                    }
                    _ => {}
                }
            }
            globals
        })
    }

    /// Retained source tokens, including trivia.
    pub fn tokens(&self) -> &[Token] {
        self.syntax.tokens()
    }

    /// The language version the file is analyzed at, as the analyzer
    /// determines it: a leading `// @dart = X.Y` comment, else the package's
    /// `languageVersion`, else [`LanguageVersion::CURRENT`].
    pub fn language_version(&self) -> LanguageVersion {
        LanguageVersion::file_override(self.source)
            .or(self.package_language_version)
            .unwrap_or(LanguageVersion::CURRENT)
    }

    /// Attach the run's analysis environment and cache the file's package
    /// language version from it.
    pub fn with_environment(mut self, environment: Option<&'a AnalysisEnvironment>) -> Self {
        self.environment = environment;
        self.package_language_version = environment
            .and_then(|environment| environment.package_language_version(self.file_path));
        self
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
