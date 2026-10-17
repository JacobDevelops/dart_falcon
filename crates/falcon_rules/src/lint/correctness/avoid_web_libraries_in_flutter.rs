//! Disallow importing web-only `dart:` libraries from Flutter code.
//!
//! Applies only to packages depending on Flutter, excluding Flutter web plugins.
//! The legacy `dart:html`, `dart:js`, and `dart:js_util` imports are checked;
//! modern JS interop libraries are allowed by the upstream lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidWebLibrariesInFlutter;

impl Rule for AvoidWebLibrariesInFlutter {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !program
            .imports()
            .any(|import| is_web_library(&import.uri().value()))
            || !checks_web_imports(ctx.file_path)
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mut diags = Vec::new();
        for import in program.imports() {
            if is_web_library(&import.uri().value()) {
                diags.push(ctx.diagnostic("avoid-web-libraries-in-flutter", "Avoid using web-only libraries in Flutter code; they are not portable across platforms.", import.span()));
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn is_web_library(uri: &str) -> bool {
    matches!(uri, "dart:html" | "dart:js" | "dart:js_util")
}

fn checks_web_imports(path: &std::path::Path) -> bool {
    let Some(manifest) = crate::cross_file::enclosing_pubspec(path) else {
        return false;
    };
    let Some(yaml) = std::fs::read_to_string(manifest)
        .ok()
        .and_then(|source| serde_yaml::from_str::<serde_yaml::Value>(&source).ok())
    else {
        return false;
    };
    let flutter = &yaml["dependencies"]["flutter"];
    !flutter.is_null() && yaml["flutter"]["plugin"]["platforms"]["web"].is_null()
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{ParsedFile, RuleAnalysis};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn project_fixtures_match_expectations() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/avoid-web-libraries-in-flutter");
        for file in [
            "bad.dart",
            "good_interop.dart",
            "non_flutter/good.dart",
            "web_plugin/good.dart",
        ] {
            let path = root.join(file);
            let source = std::fs::read_to_string(&path).unwrap();
            let tree = parse(&source).tree;
            let parsed = Arc::new(ParsedFile {
                source: source.into_boxed_str(),
                tree,
            });
            let source = &parsed.source;
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(&path, &config, &parsed);
            let lines: Vec<_> = AvoidWebLibrariesInFlutter
                .analyze(parsed.program(), &ctx)
                .into_iter()
                .map(|diagnostic| source[..diagnostic.span.start].matches('\n').count() + 1)
                .collect();
            let expected: Vec<_> = source
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains("/* expect: avoid-web-libraries-in-flutter */"))
                .map(|(index, _)| index + 1)
                .collect();
            assert_eq!(lines, expected, "{file}");
        }
    }
}
