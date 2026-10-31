//! Report implementation declarations used across a package but absent from its public API.
//!
//! In packages with an analyzed public barrel, checks public top-level declarations
//! under `lib/src/`. Public libraries are analyzed non-part libraries under `lib/`
//! outside `lib/src/`; their own declarations and transitive same-package exports
//! form the public namespace, respecting `show`/`hide` and unioning every conditional
//! export alternative. Exempts `main`, `@internal`, and `@visibleForTesting`.
//! Usage is name-based in analyzed same-package files outside `lib/src/` and `test/`.
//! Declaration names, import/export directives, comments, and strings do not count.
//! Other identifiers, including member selectors, are matched by spelling without
//! binding resolution, so unrelated same-named references can count as usage.
//! Unknown packages are not checked. Packages containing malformed Dart files are
//! skipped because their public namespace is incomplete.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::cst::{ExportDirective, ForInInit, ImportDirective, NodeKind, SyntaxToken};

use super::barrels::ProjectExports;

#[derive(Default)]
pub struct UnexportedPublicApi;

impl CrossFileRule for UnexportedPublicApi {
    fn prepare_project(
        &self,
        environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let project = ProjectExports::new(environment, files, true);
        let exported: HashSet<_> = project
            .public
            .iter()
            .enumerate()
            .filter(|(_, public)| **public)
            .flat_map(|(index, _)| project.namespaces[index].iter().copied())
            .collect();
        let barrel_packages: HashSet<_> = project
            .barrels
            .iter()
            .filter_map(|&index| project.packages[index])
            .collect();
        let malformed_packages: HashSet<_> = files
            .iter()
            .enumerate()
            .filter(|(_, file)| file.has_parse_errors)
            .filter_map(|(index, _)| project.packages[index])
            .collect();
        let mut candidates: HashMap<_, Vec<usize>> = HashMap::new();
        let mut diagnostics = Vec::new();
        for (index, declaration) in project.declarations.iter().enumerate() {
            let file = declaration.file;
            let Some(package) = project.packages[file] else {
                continue;
            };
            if !project.internal[file]
                || declaration.exempt
                || exported.contains(&index)
                || !barrel_packages.contains(&package)
                || malformed_packages.contains(&package)
            {
                continue;
            }
            candidates
                .entry((package, declaration.name))
                .or_default()
                .push(index);
        }
        if candidates.is_empty() {
            return diagnostics;
        }
        let used = used_candidates(environment, files, &project, &candidates);
        for (index, declaration) in project.declarations.iter().enumerate() {
            if used.contains(&index) {
                diagnostics.push(Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!("Public `{}` is used across the package but not exported from any public library.", declaration.name),
                    files[declaration.file].path.to_string_lossy().as_ref(),
                    declaration.span,
                ));
            }
        }
        diagnostics
    }
}

fn used_candidates(
    environment: &falcon_analyze::AnalysisEnvironment,
    files: &[ProjectFile],
    project: &ProjectExports<'_>,
    candidates: &HashMap<(usize, &str), Vec<usize>>,
) -> HashSet<usize> {
    let mut used = HashSet::new();
    for (index, file) in files.iter().enumerate() {
        let Some(package) = project.packages[index] else {
            continue;
        };
        if project.internal[index]
            || environment
                .file_package(&file.path)
                .identity()
                .is_some_and(|identity| {
                    identity.lib_root.parent().is_some_and(|root| {
                        project.paths[index]
                            .strip_prefix(root)
                            .is_ok_and(|relative| relative.starts_with("test"))
                    })
                })
        {
            continue;
        }
        let directives: Vec<_> = file
            .program()
            .imports()
            .map(ImportDirective::span)
            .chain(file.program().exports().map(ExportDirective::span))
            .collect();
        for token in file.tree().identifier_tokens(file.source()) {
            if is_declaration_name(token) {
                continue;
            }
            if directives
                .iter()
                .any(|span| span.start <= token.offset() && token.offset() < span.end)
            {
                continue;
            }
            let Some(name) = file
                .source()
                .get(token.offset()..token.offset() + token.len())
            else {
                continue;
            };
            if let Some(declarations) = candidates.get(&(package, name)) {
                used.extend(
                    declarations
                        .iter()
                        .copied()
                        .filter(|&candidate| project.declarations[candidate].file != index),
                );
            }
        }
    }
    used
}

fn is_declaration_name(token: SyntaxToken<'_>) -> bool {
    let identifier = token.parent();
    let Some(parent) = identifier.parent() else {
        return false;
    };
    if matches!(
        parent.kind(),
        NodeKind::CatchClause
            | NodeKind::ConstructorDecl
            | NodeKind::EnumVariant
            | NodeKind::PrimaryConstructor
    ) {
        return true;
    }
    if parent.kind() == NodeKind::ForInInit {
        return ForInInit::cast(parent).is_some_and(|init| {
            init.declares_variable() && init.name().span() == identifier.span()
        });
    }
    if parent.kind() == NodeKind::VariablePattern
        && parent
            .ancestors()
            .take_while(|ancestor| {
                !matches!(
                    ancestor.kind(),
                    NodeKind::PatternDeclaration
                        | NodeKind::PatternForInInit
                        | NodeKind::CasePatternLabel
                        | NodeKind::CaseCondition
                        | NodeKind::SwitchExprArm
                        | NodeKind::Block
                        | NodeKind::FuncExpr
                )
            })
            .any(|ancestor| {
                matches!(
                    ancestor.kind(),
                    NodeKind::PatternAssignStmt | NodeKind::AssignExpr
                )
            })
    {
        return false;
    }
    matches!(
        parent.kind(),
        NodeKind::ClassDecl
            | NodeKind::ClassTypeAliasDecl
            | NodeKind::MixinDecl
            | NodeKind::MixinClassDecl
            | NodeKind::EnumDecl
            | NodeKind::ExtensionDecl
            | NodeKind::ExtensionTypeDecl
            | NodeKind::FunctionDecl
            | NodeKind::TypeAliasDecl
            | NodeKind::LegacyTypedef
            | NodeKind::VarDeclarator
            | NodeKind::MethodDecl
            | NodeKind::GetterDecl
            | NodeKind::SetterDecl
            | NodeKind::FormalParam(_)
            | NodeKind::FunctionTypeParam(_)
            | NodeKind::TypeParam
            | NodeKind::LocalFuncDecl
            | NodeKind::NamedRecordField
            | NodeKind::VariablePattern
            | NodeKind::LabeledStmt
            | NodeKind::BreakStmt
            | NodeKind::ContinueStmt
    ) && parent
        .children()
        .find(|child| child.kind() == NodeKind::Identifier)
        .is_some_and(|name| name.span() == identifier.span())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_library_suppresses_incomplete_namespace_reports() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/unexported-public-api/cross-file/bad");
        for malformed in [false, true] {
            let files: Vec<_> = [
                (
                    "lib/api.dart",
                    "export 'src/hidden.dart' hide Helper;",
                    malformed,
                ),
                ("lib/src/hidden.dart", "class Helper {}", false),
                ("main.dart", "Helper? value;", false),
            ]
            .into_iter()
            .map(|(path, source, has_parse_errors)| {
                let source = if has_parse_errors {
                    "export 'src/hidden.dart' hide Helper; class {"
                } else {
                    source
                };
                let parsed = falcon_dart_parser::parser::parse(source);
                ProjectFile {
                    path: root.join(path),
                    parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                        source: source.into(),
                        tree: parsed.tree,
                    }),
                    has_parse_errors,
                }
            })
            .collect();
            let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
                files.iter().map(|file| file.path.as_path()),
                |_| None,
            );
            let diagnostics = UnexportedPublicApi.prepare_project(
                &environment,
                &files,
                &FalconConfig::default(),
                None,
            );
            assert_eq!(diagnostics.len(), usize::from(!malformed));
        }
    }
}
