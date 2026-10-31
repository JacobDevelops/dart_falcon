//! Cross-file rules receive the full analyzed file set.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use falcon_config::FalconConfig;
use falcon_dart_parser::{ParseError, ParseErrorKind};
use falcon_diagnostics::{CONFIG_DIAGNOSTIC_ID, Diagnostic};
use falcon_syntax::LanguageVersion;
use falcon_syntax::cst::{
    AstNode, Expr, FunctionDecl, GetterDecl, MethodDecl, NodeId, NodeKind, OperatorDecl, Program,
    SyntaxNode, SyntaxTree,
};

/// Shared project facts; callers may retain these after per-file analysis.
pub struct SemanticIndexes {
    pub types: crate::TypeIndex,
    pub identities: crate::IdentityIndex,
    pub signatures: crate::SignatureIndex,
}

impl SemanticIndexes {
    /// Retain declaration facts after an edit with an unchanged declaration signature.
    ///
    /// # Panics
    /// Panics if the snapshot is rejected or the file is not an indexed project file.
    pub fn refresh_file_body(&mut self, file: crate::resolve::FileId, parsed: &Arc<ParsedFile>) {
        self.identities.refresh_file_body(file, parsed);
        self.signatures.refresh_file_body(file, parsed);
    }
}

/// An immutable source and syntax arena built together.
///
/// ```
/// use falcon_analyze::ParsedFile;
/// let parsed = ParsedFile::parse("const value = 42;");
/// assert!(!parsed.has_parse_errors());
/// assert_eq!(parsed.program().syntax().text(), parsed.source());
/// let _tree = parsed.tree();
/// ```
///
/// Source and syntax cannot be replaced independently.
///
/// ```compile_fail,E0616
/// let mut parsed = falcon_analyze::ParsedFile::parse("");
/// parsed.source = "different source".into();
/// ```
///
/// ```compile_fail,E0616
/// let mut parsed = falcon_analyze::ParsedFile::parse("");
/// parsed.tree = falcon_dart_parser::parse("class Other {}").tree;
/// ```
///
/// ```compile_fail,E0616
/// let mut parsed = falcon_analyze::ParsedFile::parse("");
/// parsed.outcome = panic!();
/// ```
#[derive(Debug)]
pub struct ParsedFile {
    source: Box<str>,
    tree: SyntaxTree,
    outcome: ParseOutcome,
}

/// Closed internal classification: worker rejection is not parser recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseOutcome {
    Clean,
    Recovered,
    Rejected,
}

impl ParsedFile {
    #[must_use]
    pub fn parse(source: impl Into<Box<str>>) -> Self {
        Self::parse_for_package(source, None).0
    }

    /// Parse with the package's language version, retaining recoverable diagnostics.
    ///
    /// A failed parser worker retains an opaque tree of the original source and
    /// reports the same worker-failure diagnostic as the parser's infallible API.
    ///
    /// ```
    /// let (parsed, errors) = falcon_analyze::ParsedFile::parse_for_package("class C {}", None);
    /// assert!(errors.is_empty());
    /// assert!(!parsed.has_parse_errors());
    /// ```
    #[must_use]
    pub fn parse_for_package(
        source: impl Into<Box<str>>,
        package_version: Option<LanguageVersion>,
    ) -> (Self, Vec<ParseError>) {
        Self::parse_result(Self::try_parse_for_package(source, package_version))
    }

    fn parse_result(
        result: Result<(Self, Vec<ParseError>), (Box<str>, std::io::Error)>,
    ) -> (Self, Vec<ParseError>) {
        match result {
            Ok(parsed) => parsed,
            Err((source, error)) => (
                Self::rejected(source),
                vec![ParseError {
                    kind: ParseErrorKind::Syntax,
                    message: format!("parser worker failed: {error}"),
                    offset: 0,
                }],
            ),
        }
    }

    /// # Errors
    /// Returns the original source and an error if the parser worker cannot start or panics.
    ///
    /// ```
    /// let (parsed, errors) = falcon_analyze::ParsedFile::try_parse_for_package("", None)?;
    /// assert!(errors.is_empty());
    /// assert_eq!(parsed.source(), "");
    /// # Ok::<(), (Box<str>, std::io::Error)>(())
    /// ```
    pub fn try_parse_for_package(
        source: impl Into<Box<str>>,
        package_version: Option<LanguageVersion>,
    ) -> Result<(Self, Vec<ParseError>), (Box<str>, std::io::Error)> {
        let source = source.into();
        let parsed = match falcon_dart_parser::try_parse_for_package(&source, package_version) {
            Ok(parsed) => parsed,
            Err(error) => return Err((source, error)),
        };
        let outcome = if parsed.errors.is_empty() {
            ParseOutcome::Clean
        } else {
            ParseOutcome::Recovered
        };
        Ok((
            Self {
                source,
                tree: parsed.tree,
                outcome,
            },
            parsed.errors,
        ))
    }

    /// Reparse this snapshot's owned source without copying it.
    ///
    /// # Errors
    /// Returns the original source and an error if the parser worker cannot start or panics.
    pub fn try_reparse_for_package(
        self,
        package_version: Option<LanguageVersion>,
    ) -> Result<(Self, Vec<ParseError>), (Box<str>, std::io::Error)> {
        Self::try_parse_for_package(self.source, package_version)
    }

    /// Retain non-Dart source, such as a manifest, without parsing it as Dart.
    #[must_use]
    pub fn lexical(source: impl Into<Box<str>>) -> Self {
        Self::with_source(source, falcon_dart_parser::lex_tree, ParseOutcome::Clean)
    }

    /// Retain unparsed source rejected before parsing or after a parser worker failure.
    #[must_use]
    pub fn rejected(source: impl Into<Box<str>>) -> Self {
        Self::with_source(source, SyntaxTree::unparsed, ParseOutcome::Rejected)
    }

    fn with_source(
        source: impl Into<Box<str>>,
        build: impl FnOnce(&str) -> SyntaxTree,
        outcome: ParseOutcome,
    ) -> Self {
        let source = source.into();
        let tree = build(&source);
        Self {
            source,
            tree,
            outcome,
        }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }

    #[must_use]
    pub fn has_parse_errors(&self) -> bool {
        self.outcome != ParseOutcome::Clean
    }

    /// Whether source was rejected before parsing or parsing failed without a usable tree.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        self.outcome == ParseOutcome::Rejected
    }

    pub fn program(&self) -> Program<'_> {
        self.tree.program(&self.source)
    }

    /// Hash declaration text, excluding bodies whose return types are explicit.
    /// Body edits cannot change these declarations' inferred signatures.
    pub fn declaration_signature(&self) -> u64 {
        let mut signature = DefaultHasher::new();
        let mut cursor = 0;
        let mut body_start = 0;
        let mut body_end = 0;
        for node in self.program().syntax().descendants() {
            if node.span().start >= body_start && node.span().start < body_end {
                continue;
            }
            if let Some(body) = declaration_body(node) {
                let span = body.span();
                self.source[cursor..span.start].hash(&mut signature);
                cursor = span.end;
                body_start = span.start;
                body_end = span.end;
            }
        }
        self.source[cursor..].hash(&mut signature);
        signature.finish()
    }

    pub(crate) fn type_occurrences(&self) -> rustc_hash::FxHashMap<usize, usize> {
        let mut occurrences = rustc_hash::FxHashMap::default();
        let mut ordinal = 0;
        let mut body_start = 0;
        let mut body_end = 0;
        for node in self.program().syntax().descendants() {
            let offset = node.span().start;
            if offset >= body_start && offset < body_end {
                continue;
            }
            if matches!(node.kind(), NodeKind::NamedType | NodeKind::TypeParam) {
                // Only declaration occurrences need a stable ordinal. Body-local
                // occurrences derive their separate namespace directly from offsets.
                occurrences.insert(offset, ordinal);
                ordinal += 1;
            }
            if let Some(body) = declaration_body(node) {
                body_start = body.span().start;
                body_end = body.span().end;
            }
        }
        occurrences
    }
}

fn declaration_body(node: SyntaxNode<'_>) -> Option<falcon_syntax::cst::FunctionBody<'_>> {
    match node.kind() {
        NodeKind::FunctionDecl => FunctionDecl::cast(node)
            .filter(|function| function.return_type().is_some())
            .and_then(falcon_syntax::cst::FunctionDecl::body),
        NodeKind::MethodDecl => MethodDecl::cast(node)
            .filter(|method| method.return_type().is_some())
            .and_then(falcon_syntax::cst::MethodDecl::body),
        NodeKind::GetterDecl => GetterDecl::cast(node)
            .filter(|getter| getter.return_type().is_some())
            .and_then(falcon_syntax::cst::GetterDecl::body),
        NodeKind::OperatorDecl => OperatorDecl::cast(node)
            .filter(|operator| operator.return_type().is_some())
            .and_then(falcon_syntax::cst::OperatorDecl::body),
        _ => None,
    }
}

#[derive(Clone)]
pub struct NodeRef {
    pub file: Arc<ParsedFile>,
    pub id: NodeId,
}

impl std::fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node = self.view();
        f.debug_struct("NodeRef")
            .field("id", &self.id)
            .field("kind", &node.kind())
            .field("span", &node.span())
            .finish_non_exhaustive()
    }
}

impl NodeRef {
    pub fn new(file: Arc<ParsedFile>, id: NodeId) -> Self {
        Self { file, id }
    }

    #[must_use]
    ///
    /// # Panics
    /// Panics if the retained node no longer belongs to its parsed file.
    pub fn view(&self) -> SyntaxNode<'_> {
        self.file
            .tree
            .node(&self.file.source, self.id)
            .expect("retained node belongs to its parsed file")
    }

    #[must_use]
    ///
    /// # Panics
    /// Panics if the retained node has a different kind from `T`.
    pub fn cast<'a, T: AstNode<'a>>(&'a self) -> T {
        T::cast(self.view()).expect("retained node has its original kind")
    }

    #[must_use]
    pub fn expr(&self) -> Expr<'_> {
        self.cast()
    }
}

/// A path paired with an immutable parsed snapshot.
///
/// ```
/// use std::{path::PathBuf, sync::Arc};
/// use falcon_analyze::{ParsedFile, ProjectFile};
/// let file = ProjectFile::new(PathBuf::from("main.dart"), Arc::new(ParsedFile::parse(""))).unwrap();
/// assert_eq!(file.path(), std::path::Path::new("main.dart"));
/// assert!(!file.has_parse_errors());
/// assert_eq!(file.parsed().source(), file.source());
/// ```
///
/// ```compile_fail,E0451
/// use std::{path::PathBuf, sync::Arc};
/// use falcon_analyze::{ParsedFile, ProjectFile};
/// let _file = ProjectFile {
///     path: PathBuf::from("main.dart"),
///     parsed: Arc::new(ParsedFile::parse("")),
/// };
/// ```
///
/// ```compile_fail,E0616
/// use std::{path::PathBuf, sync::Arc};
/// use falcon_analyze::{ParsedFile, ProjectFile};
/// let mut file = ProjectFile::new(PathBuf::from("main.dart"), Arc::new(ParsedFile::parse(""))).unwrap();
/// file.parsed = Arc::new(ParsedFile::parse("class Other {}"));
/// ```
///
/// ```compile_fail,E0616
/// use std::{path::PathBuf, sync::Arc};
/// use falcon_analyze::{ParsedFile, ProjectFile};
/// let mut file = ProjectFile::new(PathBuf::from("main.dart"), Arc::new(ParsedFile::parse(""))).unwrap();
/// file.path = PathBuf::from("other.dart");
/// ```
pub struct ProjectFile {
    path: PathBuf,
    parsed: Arc<ParsedFile>,
}

impl ProjectFile {
    #[must_use]
    pub fn new(path: PathBuf, parsed: Arc<ParsedFile>) -> Option<Self> {
        if parsed.is_rejected() {
            return None;
        }
        Some(Self { path, parsed })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn parsed(&self) -> &Arc<ParsedFile> {
        &self.parsed
    }

    /// Replace the snapshot while preserving this path; rejection leaves it unchanged.
    #[must_use]
    pub fn replace_parsed(&mut self, parsed: Arc<ParsedFile>) -> Option<()> {
        if parsed.is_rejected() {
            return None;
        }
        self.parsed = parsed;
        Some(())
    }

    /// Recovery nodes can create false declarations; retain these files only for reference detection.
    #[must_use]
    pub fn has_parse_errors(&self) -> bool {
        self.parsed.has_parse_errors()
    }

    #[must_use]
    pub fn program(&self) -> Program<'_> {
        self.parsed.program()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.parsed.source
    }

    #[must_use]
    pub fn tree(&self) -> &SyntaxTree {
        &self.parsed.tree
    }
}

pub trait CrossFileRule: Send + Sync {
    /// Provided indexes must describe the same Dart files and agree with their
    /// current semantic facts. Retained declaration-index syntax may predate
    /// body-only edits when declarations are unchanged and body facts are refreshed.
    /// The environment must be the same package snapshot used for file analysis.
    /// Returns collector output; use a registry or [`CrossFileRuleAnalysis`] to assign ids.
    fn prepare_project(
        &self,
        environment: &crate::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic>;
}

/// Standalone project analysis using the canonical id from the rule declaration.
pub trait CrossFileRuleAnalysis: CrossFileRule {
    fn analyze_project(
        &self,
        name: &'static str,
        environment: &crate::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic>;
}

impl<T: CrossFileRule + ?Sized> CrossFileRuleAnalysis for T {
    fn analyze_project(
        &self,
        name: &'static str,
        environment: &crate::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = self.prepare_project(environment, files, config, indexes);
        for diagnostic in &mut diagnostics {
            if diagnostic.rule != CONFIG_DIAGNOSTIC_ID {
                diagnostic.rule = name;
            }
        }
        diagnostics
    }
}

#[derive(Default)]
pub struct CrossFileRuleRegistry {
    rules: Vec<(&'static str, Box<dyn CrossFileRule>)>,
}

impl CrossFileRuleRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, name: &'static str, rule: Box<dyn CrossFileRule>) {
        self.rules.push((name, rule));
    }

    #[must_use]
    pub fn rules(&self) -> &[(&'static str, Box<dyn CrossFileRule>)] {
        &self.rules
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Callers apply inline suppression and per-path severity resolution.
    ///
    /// # Errors
    ///
    /// Returns an error if the shared worker pool cannot start or a rule panics;
    /// callers choose which files report it with [`crate::rules_thread_diagnostic`].
    pub fn run_all(
        &self,
        environment: &crate::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> std::io::Result<Vec<Diagnostic>> {
        crate::registry::with_rules_stack(|| {
            self.rules
                .iter()
                .flat_map(|(name, rule)| {
                    rule.analyze_project(name, environment, files, config, indexes)
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_snapshot_derives_recovery_state() {
        let (valid, errors) = ParsedFile::parse_for_package("class Good {}", None);
        assert!(errors.is_empty());
        assert!(!valid.has_parse_errors());
        assert!(!valid.is_rejected());
        let (invalid, errors) = ParsedFile::parse_for_package("class Bad extends {", None);
        assert!(!errors.is_empty());
        assert!(invalid.has_parse_errors());
        assert!(!invalid.is_rejected());
        let file = ProjectFile::new(PathBuf::from("bad.dart"), Arc::new(invalid)).unwrap();
        assert!(file.has_parse_errors());
    }

    #[test]
    fn lexical_and_rejected_snapshots_retain_their_original_source() {
        let source = "name: application\n";
        let lexical = ParsedFile::lexical(source);
        assert_eq!(lexical.source(), source);
        assert!(!lexical.has_parse_errors());
        assert!(lexical.tree().is_lossless(lexical.source()));
        assert!(!lexical.is_rejected());
        let rejected = ParsedFile::rejected(source);
        assert_eq!(rejected.source(), source);
        assert!(rejected.has_parse_errors());
        assert!(rejected.tree().is_lossless(rejected.source()));
        assert!(rejected.is_rejected());
        assert_eq!(rejected.program().syntax().text(), source);
        assert_eq!(rejected.program().declarations().count(), 0);
    }

    #[test]
    fn parse_for_package_worker_failure_retains_rejected_source() {
        let source = "class C { /* unterminated \u{1f980}";
        let (parsed, errors) = ParsedFile::parse_result(Err((
            source.into(),
            std::io::Error::other("deliberate parser failure"),
        )));
        assert!(parsed.is_rejected());
        assert!(parsed.has_parse_errors());
        assert_eq!(parsed.source(), source);
        assert_eq!(parsed.program().syntax().text(), source);
        assert!(parsed.tree().is_lossless(source));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, ParseErrorKind::Syntax);
        assert_eq!(errors[0].offset, 0);
        assert_eq!(
            errors[0].message,
            "parser worker failed: deliberate parser failure"
        );
    }

    #[test]
    fn rejected_snapshots_cannot_enter_a_project() {
        let rejected = Arc::new(ParsedFile::rejected("class Rejected {}"));
        assert!(ProjectFile::new(PathBuf::from("rejected.dart"), Arc::clone(&rejected)).is_none());
        let original = Arc::new(ParsedFile::parse("class Original {}"));
        let mut file =
            ProjectFile::new(PathBuf::from("original.dart"), Arc::clone(&original)).unwrap();
        assert_eq!(file.replace_parsed(rejected), None);
        assert!(Arc::ptr_eq(file.parsed(), &original));
        assert_eq!(file.source(), "class Original {}");
        assert_eq!(file.path(), Path::new("original.dart"));
    }

    #[test]
    fn rejected_signature_refresh_preserves_previous_body_facts() {
        let original = crate::resolve::parsed_fixture("void main() { var value = 1; value = 2; }");
        let rejected = Arc::new(ParsedFile::rejected(original.source()));
        let path = Path::new("/project/main.dart");
        let sources = [crate::resolve::IdentitySource {
            path,
            parsed: &original,
            has_parse_errors: false,
        }];
        let identities = crate::IdentityIndex::from_project_files(&sources, &[]);
        let file = identities.file(path).unwrap();
        let mut index = crate::SignatureIndex::default();
        index.refresh_file_body(file, &original);
        let retained = Arc::clone(index.flow_writes(file).unwrap());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            index.refresh_file_body(file, &rejected);
        }));
        assert!(result.is_err());
        assert!(Arc::ptr_eq(index.flow_writes(file).unwrap(), &retained));
        assert!(index.file_body_is_current(file, &original));
        assert!(!index.file_body_is_current(file, &rejected));
    }

    #[test]
    fn consuming_reparse_reuses_owned_source() {
        let parsed = ParsedFile::parse("class C {}");
        let source = parsed.source().as_ptr();
        let (reparsed, errors) = parsed
            .try_reparse_for_package(Some(LanguageVersion::CURRENT))
            .unwrap();
        assert!(errors.is_empty());
        assert_eq!(source, reparsed.source().as_ptr());
        assert_eq!(reparsed.program().syntax().text(), reparsed.source());
    }

    #[test]
    fn retained_node_owns_its_source_and_arena() {
        let retained = {
            let file = Arc::new(ParsedFile::parse("const value = 42;"));
            let expression = file
                .program()
                .syntax()
                .descendants()
                .find_map(|node| match Expr::cast(node) {
                    Some(expression @ Expr::IntLit(_)) => Some(expression),
                    _ => None,
                })
                .unwrap();
            NodeRef::new(Arc::clone(&file), expression.syntax().id())
        };
        assert_eq!(retained.expr().syntax().text(), "42");
        assert!(std::ptr::eq(retained.view().tree(), retained.file.tree()));
    }

    #[test]
    fn retained_node_keeps_original_snapshot_after_source_replacement() {
        let mut file = ProjectFile::new(
            PathBuf::from("main.dart"),
            Arc::new(ParsedFile::parse("const value = 42;")),
        )
        .unwrap();
        let expression = file
            .program()
            .syntax()
            .descendants()
            .find_map(|node| match Expr::cast(node) {
                Some(expression @ Expr::IntLit(_)) => Some(expression),
                _ => None,
            })
            .unwrap();
        let retained = NodeRef::new(Arc::clone(file.parsed()), expression.syntax().id());
        file.replace_parsed(Arc::new(ParsedFile::parse("const value = 99;")))
            .unwrap();
        assert_eq!(file.source(), "const value = 99;");
        assert_eq!(file.program().syntax().text(), "const value = 99;");
        assert_eq!(retained.file.source(), "const value = 42;");
        assert_eq!(retained.expr().syntax().text(), "42");
        assert!(!Arc::ptr_eq(file.parsed(), &retained.file));
        assert!(std::ptr::eq(retained.view().tree(), retained.file.tree()));
    }

    #[test]
    fn panicking_cross_file_rules_return_worker_errors() {
        struct PanickingRule;
        impl CrossFileRule for PanickingRule {
            fn prepare_project(
                &self,
                _environment: &crate::AnalysisEnvironment,
                _files: &[ProjectFile],
                _config: &FalconConfig,
                _indexes: Option<&SemanticIndexes>,
            ) -> Vec<Diagnostic> {
                panic!("deliberate cross-file panic");
            }
        }

        let mut registry = CrossFileRuleRegistry::new();
        registry.register("panicking-rule", Box::new(PanickingRule));
        let environment = crate::AnalysisEnvironment::with_sdk_locator(|_| None);
        let error = registry
            .run_all(&environment, &[], &FalconConfig::default(), None)
            .unwrap_err();
        assert!(error.to_string().contains("deliberate cross-file panic"));
        let diagnostic = crate::rules_thread_diagnostic(std::path::Path::new("main.dart"), &error);
        assert_eq!(
            diagnostic.rule,
            falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
        );
    }
}
