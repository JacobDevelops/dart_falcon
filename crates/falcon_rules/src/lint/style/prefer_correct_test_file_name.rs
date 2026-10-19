//! Requires entry-point files under package-root test/ or integration_test/ to use the _test.dart suffix.
//!
//! Files without main are test support libraries and do not need the suffix.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferCorrectTestFileName;

impl Rule for PreferCorrectTestFileName {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let in_test = in_test_runner_directory(ctx.file_path);
        let correct_name = crate::lint::declarations::has_test_file_suffix(ctx.file_path);
        if !in_test || correct_name {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

fn in_test_runner_directory(path: &std::path::Path) -> bool {
    let Some(pubspec) = crate::cross_file::enclosing_pubspec(path) else {
        return false;
    };
    let Some(root) = pubspec.parent() else {
        return false;
    };
    falcon_analyze::canonical_or_lexical(path)
        .strip_prefix(root)
        .is_ok_and(|relative| {
            relative.components().next().is_some_and(|first| {
                matches!(
                    first.as_os_str().to_str(),
                    Some("test" | "integration_test")
                )
            })
        })
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Function(function) = declaration
            && function.name().name() == "main"
        {
            self.diags.push(self.ctx.diagnostic("prefer-correct-test-file-name", "Use the _test.dart suffix for a test entry-point file.", function.name().span()));
        }
        StructuralTraversal::SkipChildren
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn recognizes_test_entry_points_without_requiring_imports() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("test/app");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pubspec.yaml"), "name: filename_rule_test\n").unwrap();
        let nested = root.join("test/fixtures/app");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("pubspec.yaml"), "name: nested_fixture\n").unwrap();
        for (path, source, expected) in [
            ("test/widget.dart", "void main() {}", 1),
            ("test/unit/service.dart", "void main() {}", 1),
            ("test/unit/service_test.dart", "void main() {}", 0),
            ("integration_test/app.dart", "void main() {}", 1),
            ("integration_test/app_test.dart", "void main() {}", 0),
            ("test_driver/app.dart", "void main() {}", 0),
            ("testing/helper.dart", "void main() {}", 0),
            ("lib/main.dart", "void main() {}", 0),
            ("lib/test/main.dart", "void main() {}", 0),
            ("tests/main.dart", "void main() {}", 0),
            ("test/helper.dart", "void helper() {}", 0),
            ("test/helper.dart", "class Helper { void main() {} }", 0),
            ("test/fixtures/app/lib/main.dart", "void main() {}", 0),
            ("test/fixtures/app/test/main.dart", "void main() {}", 1),
            (
                "test/fixtures/app/integration_test/main.dart",
                "void main() {}",
                1,
            ),
            (
                "test/fixtures/app/test_driver/app.dart",
                "void main() {}",
                0,
            ),
            ("test/fixtures/app/testing/helper.dart", "void main() {}", 0),
        ] {
            let result = falcon_dart_parser::parse(source);
            let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: result.tree,
            });
            let config = falcon_config::FalconConfig::default();
            let path = root.join(path);
            let ctx = AnalyzeContext::new(&path, &config, &parsed);
            let diagnostics = PreferCorrectTestFileName.analyze(parsed.program(), &ctx);
            assert_eq!(diagnostics.len(), expected, "{path:?}: {diagnostics:?}");
        }
    }
}
