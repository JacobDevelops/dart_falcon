//! Requires class names in matching file paths to match configured regular expressions.
//!
//! `entries` defaults to an empty list, disabling the rule. Each entry requires a
//! `path` regex and a `pattern` regex. Paths are relative to the package root,
//! including `lib/`, with `/` separators; without a package, the invoked path is
//! used. `ignore-private` defaults to false. Use Falcon's per-path configuration
//! overrides to customize severity.

use crate::lint::options;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{ClassDecl, ClassTypeAliasDecl, Identifier, MixinClassDecl, Program};

#[derive(Default)]
pub struct MatchClassNamePattern;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Path-specific class naming requirements. Empty entries disable the rule.
    entries: Vec<Entry>,
}

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Entry {
    /// Regular expression selecting file paths.
    path: options::Regex,
    /// Regular expression that class names must match.
    pattern: options::Regex,
    /// Whether private class names are exempt.
    #[serde(default)]
    ignore_private: bool,
}

options::impl_options!(Options);

impl Rule for MatchClassNamePattern {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let configured = options::get::<Options>(ctx, Self::NAME);
        if configured.entries.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let path = matching_path(ctx);
        let entries: Vec<_> = configured
            .entries
            .iter()
            .filter(|entry| entry.path.is_match(&path))
            .collect();
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            entries,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: Vec<&'a Entry>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>) {
        for entry in &self.entries {
            if (entry.ignore_private && name.name().starts_with('_'))
                || entry.pattern.is_match(name.name())
            {
                continue;
            }
            let diagnostic = self.ctx.diagnostic(
                format!(
                    "Class name must match the configured pattern `{}`.",
                    entry.pattern.as_str()
                ),
                name.span(),
            );
            self.diagnostics.push(diagnostic);
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_class_decl(&mut self, class: ClassDecl<'a>) -> StructuralTraversal {
        self.check(class.name());
        StructuralTraversal::Continue
    }
    fn visit_class_type_alias(&mut self, class: ClassTypeAliasDecl<'a>) -> StructuralTraversal {
        self.check(class.name());
        StructuralTraversal::Continue
    }
    fn visit_mixin_class(&mut self, class: MixinClassDecl<'a>) -> StructuralTraversal {
        self.check(class.name());
        StructuralTraversal::Continue
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}

fn matching_path(ctx: &AnalyzeContext<'_>) -> String {
    let file_package = ctx.file_package();
    let absolute =
        std::path::absolute(ctx.file_path()).unwrap_or_else(|_| ctx.file_path().to_path_buf());
    let canonical = falcon_analyze::canonical_or_lexical(&absolute);
    let root = file_package
        .metadata()
        .and_then(|metadata| metadata.manifest.parent())
        .map(falcon_analyze::canonical_or_lexical);
    let relative = root
        .as_ref()
        .and_then(|root| canonical.strip_prefix(root).ok());
    if let Some(relative) = relative {
        return relative.to_string_lossy().replace('\\', "/");
    }
    if let Some(relative) = file_package
        .identity()
        .and_then(|package| canonical.strip_prefix(&package.lib_root).ok())
    {
        return format!("lib/{}", relative.to_string_lossy().replace('\\', "/"));
    }
    ctx.file_path().to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::Options;
    use serde_json::json;

    #[test]
    fn rejects_invalid_entries() {
        for entry in [
            json!({"path": "[", "pattern": "State$"}),
            json!({"path": ".*", "pattern": "["}),
            json!({"path": ".*"}),
            json!({"path": ".*", "pattern": "State$", "ignorePrivate": true}),
            json!({"path": ".*", "pattern": "State$", "severity": "error"}),
        ] {
            assert!(serde_json::from_value::<Options>(json!({"entries": [entry]})).is_err());
        }
    }

    #[test]
    fn paths_are_package_relative_for_absolute_and_relative_inputs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/match-class-name-pattern");
        let absolute = root.join("lib/bad_package.dart");
        let cwd = std::env::current_dir().unwrap();
        let relative = absolute.strip_prefix(&cwd).unwrap();
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: "".into(),
            tree: falcon_dart_parser::parser::parse("").tree,
        });
        let config = falcon_config::FalconConfig::default();
        for path in [absolute.as_path(), relative] {
            let ctx = falcon_analyze::AnalyzeContext::new(path, &config, &parsed);
            assert_eq!(super::matching_path(&ctx), "lib/bad_package.dart");
        }
    }

    #[cfg(unix)]
    #[test]
    fn paths_are_package_relative_through_symlinked_roots() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("package");
        std::fs::create_dir_all(root.join("test")).unwrap();
        std::fs::write(root.join("pubspec.yaml"), "name: example\n").unwrap();
        std::fs::write(root.join("test/value.dart"), "").unwrap();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let path = alias.join("test/value.dart");
        let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            [path.as_path()],
            |_| None,
        );
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: "".into(),
            tree: falcon_dart_parser::parser::parse("").tree,
        });
        let config = falcon_config::FalconConfig::default();
        let ctx = falcon_analyze::AnalyzeContext::new(&path, &config, &parsed)
            .with_environment(Some(&environment));
        assert_eq!(super::matching_path(&ctx), "test/value.dart");
    }
}
