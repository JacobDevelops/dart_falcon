//! Documents public declarations in a package's exported API.
//!
//! UPSTREAM-NOOP: `package_api_docs` is absent from current upstream, having
//! been removed in Dart 3.7. Falcon retains its API-documentation implementation.
//!
//! Libraries under `lib` outside `lib/src` are entry points; local exports are
//! followed transitively with show/hide combinators, including library parts.
//! External and conditional exports are not resolved. Members marked
//! `@override` are exempt, without guessing unannotated overrides.

use super::{ResolvedRef, lib_dir_package, resolve_directive_uri};
use crate::lint::declarations::annotation;
use crate::lint::style::public_member_api_docs::{api_declarations, documented};
use falcon_analyze::{CrossFileRule, ProjectFile, SemanticModel, canonical_or_lexical};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::ast::ImportCombinator;
use std::collections::{BTreeSet, HashMap};

#[derive(Default)]
pub struct PackageApiDocs;

impl CrossFileRule for PackageApiDocs {
    fn analyze_project(
        &self,
        files: &[ProjectFile],
        _config: &FalconConfig,
        indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let paths: HashMap<_, _> = files
            .iter()
            .enumerate()
            .map(|(i, f)| (canonical_or_lexical(&f.path), i))
            .collect();
        let declarations: Vec<_> = files
            .iter()
            .map(|file| {
                let model = indexes.map(|indexes| {
                    SemanticModel::new(&file.path, &indexes.identities, Some(&indexes.types))
                });
                api_declarations(&file.program, &file.source, model.as_ref())
            })
            .collect();
        let packages: Vec<_> = files.iter().map(|f| lib_dir_package(&f.path)).collect();
        let mut visible: Vec<BTreeSet<(usize, &str)>> = declarations
            .iter()
            .enumerate()
            .map(|(i, ds)| {
                ds.iter()
                    .filter(|d| d.owner.is_none() && !d.name.starts_with('_'))
                    .map(|d| (i, d.name))
                    .collect()
            })
            .collect();
        let mut edges = Vec::new();
        for (i, file) in files.iter().enumerate() {
            let Some(package) = packages[i].as_ref() else {
                continue;
            };
            for export in &file.program.exports {
                if !export.configurable_uris.is_empty() {
                    continue;
                }
                if let Some(ResolvedRef::Path(path)) =
                    resolve_directive_uri(&file.path, &export.uri.value, Some(package))
                    && let Some(&target) = paths.get(&path)
                    && path.starts_with(&package.lib_root)
                {
                    edges.push((i, target, export.combinators.as_slice()));
                }
            }
            for part in &file.program.part_directives {
                if let Some(ResolvedRef::Path(path)) =
                    resolve_directive_uri(&file.path, &part.uri.value, Some(package))
                    && let Some(&target) = paths.get(&path)
                    && path.starts_with(&package.lib_root)
                {
                    edges.push((i, target, &[][..]));
                }
            }
        }
        let mut pending = Vec::new();
        loop {
            pending.clear();
            for &(from, target, combinators) in &edges {
                for &(origin, name) in &visible[target] {
                    let included = combinators.iter().all(|c| match c {
                        ImportCombinator::Show(names, _) => names.iter().any(|n| n.name == name),
                        ImportCombinator::Hide(names, _) => !names.iter().any(|n| n.name == name),
                    });
                    if included && !visible[from].contains(&(origin, name)) {
                        pending.push((from, origin, name));
                    }
                }
            }
            if pending.is_empty() {
                break;
            }
            for &(from, origin, name) in &pending {
                visible[from].insert((origin, name));
            }
        }
        let mut public = BTreeSet::new();
        for (i, package) in packages.iter().enumerate() {
            if let Some(package) = package
                && files[i].program.part_of_directive.is_none()
                && !canonical_or_lexical(&files[i].path).starts_with(package.lib_root.join("src"))
            {
                public.extend(visible[i].iter().copied());
            }
        }
        let mut diagnostics = Vec::new();
        for (i, file) in files.iter().enumerate() {
            if packages[i].is_none() {
                continue;
            }
            let tokens = falcon_dart_parser::lexer::Lexer::new(&file.source).tokenize();
            for d in &declarations[i] {
                if d.name.starts_with('_')
                    || annotation(d.annotations, "override")
                    || d.documented(&tokens)
                    || !public.contains(&(i, d.owner.unwrap_or(d.name)))
                {
                    continue;
                }
                diagnostics.push(Diagnostic::new(
                    "package-api-docs",
                    Severity::Warning,
                    "Missing documentation for public API.",
                    file.path.to_string_lossy().into_owned(),
                    d.span,
                ));
            }
            for declaration in &file.program.declarations {
                if let falcon_syntax::ast::TopLevelDecl::ExtensionType(node) = declaration
                    && let Some(representation) = node.representation()
                {
                    let field = &representation.name;
                    if !field.name.starts_with('_')
                        && public.contains(&(i, node.name.name.as_str()))
                        && !documented(&tokens, representation.span.start)
                    {
                        diagnostics.push(Diagnostic::new(
                            "package-api-docs",
                            Severity::Warning,
                            "Missing documentation for public API.",
                            file.path.to_string_lossy().into_owned(),
                            field.span,
                        ));
                    }
                }
            }
        }
        diagnostics
    }
}
