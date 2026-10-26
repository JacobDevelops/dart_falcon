//! Cross-file rules receive the full analyzed file set.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use falcon_config::FalconConfig;
use falcon_diagnostics::{CONFIG_DIAGNOSTIC_ID, Diagnostic};
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
    pub fn refresh_file_body(&mut self, file: crate::FileId, parsed: &Arc<ParsedFile>) {
        self.identities.refresh_file_body(file, parsed);
        self.signatures.refresh_file_body(file, parsed.program());
    }
}

#[derive(Debug)]
pub struct ParsedFile {
    pub source: Box<str>,
    pub tree: SyntaxTree,
}

impl ParsedFile {
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
            if matches!(node.kind(), NodeKind::NamedType | NodeKind::TypeParam) {
                // Declaration occurrences keep their ordinal when an earlier body
                // changes size. Body-local occurrences occupy a separate namespace.
                let identity = if offset >= body_start && offset < body_end {
                    offset | (1 << (usize::BITS - 1))
                } else {
                    let identity = ordinal;
                    ordinal += 1;
                    identity
                };
                occurrences.insert(offset, identity);
            }
            if (offset < body_start || offset >= body_end)
                && let Some(body) = declaration_body(node)
            {
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
            .and_then(|function| function.body()),
        NodeKind::MethodDecl => MethodDecl::cast(node)
            .filter(|method| method.return_type().is_some())
            .and_then(|method| method.body()),
        NodeKind::GetterDecl => GetterDecl::cast(node)
            .filter(|getter| getter.return_type().is_some())
            .and_then(|getter| getter.body()),
        NodeKind::OperatorDecl => OperatorDecl::cast(node)
            .filter(|operator| operator.return_type().is_some())
            .and_then(|operator| operator.body()),
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
            .finish()
    }
}

impl NodeRef {
    pub fn new(file: Arc<ParsedFile>, id: NodeId) -> Self {
        Self { file, id }
    }

    pub fn view(&self) -> SyntaxNode<'_> {
        self.file
            .tree
            .node(&self.file.source, self.id)
            .expect("retained node belongs to its parsed file")
    }

    pub fn cast<'a, T: AstNode<'a>>(&'a self) -> T {
        T::cast(self.view()).expect("retained node has its original kind")
    }

    pub fn expr(&self) -> Expr<'_> {
        self.cast()
    }
}

pub struct ProjectFile {
    pub path: PathBuf,
    pub parsed: Arc<ParsedFile>,
    /// Recovery nodes can create false declarations; keep these files only for reference detection.
    pub has_parse_errors: bool,
}

impl ProjectFile {
    pub fn program(&self) -> Program<'_> {
        self.parsed.program()
    }

    pub fn source(&self) -> &str {
        &self.parsed.source
    }

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
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, name: &'static str, rule: Box<dyn CrossFileRule>) {
        self.rules.push((name, rule));
    }

    pub fn rules(&self) -> &[(&'static str, Box<dyn CrossFileRule>)] {
        &self.rules
    }

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
    fn retained_node_owns_its_source_and_arena() {
        let retained = {
            let source: Box<str> = "const value = 42;".into();
            let tree = falcon_dart_parser::parse(&source).tree;
            let file = Arc::new(ParsedFile { source, tree });
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
        assert!(std::ptr::eq(retained.view().tree(), &retained.file.tree));
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
