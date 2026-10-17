//! Cross-file rules receive the full analyzed file set.

use std::path::PathBuf;
use std::sync::Arc;

use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{AstNode, Expr, NodeId, Program, SyntaxNode, SyntaxTree};

/// Shared project facts; callers may retain these after per-file analysis.
pub struct SemanticIndexes {
    pub types: crate::TypeIndex,
    pub identities: crate::IdentityIndex,
    pub signatures: crate::SignatureIndex,
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
    /// Provided indexes must describe the same Dart files and source revisions.
    fn analyze_project(
        &self,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic>;
}

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

    /// Callers apply inline suppression and per-path severity resolution.
    ///
    /// # Errors
    ///
    /// Returns the spawn error when the rules thread cannot start; callers
    /// choose which files report it with [`crate::rules_thread_diagnostic`].
    pub fn run_all(
        &self,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> std::io::Result<Vec<Diagnostic>> {
        crate::registry::with_rules_stack(|| {
            self.rules
                .iter()
                .flat_map(|rule| rule.analyze_project(files, config, indexes))
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
}
