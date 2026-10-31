use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use falcon_analyze::{AnalysisEnvironment, ProjectFile};
use falcon_syntax::cst::*;

use super::{ResolvedRef, canonical_package_path, project_packages, resolve_directive_uri};

pub(super) struct Declaration<'a> {
    pub name: &'a str,
    pub span: Span,
    pub file: usize,
    pub exempt: bool,
}

pub(super) struct Export<'a> {
    pub target: usize,
    pub directive: ExportDirective<'a>,
}

impl Export<'_> {
    pub fn includes(&self, name: &str) -> bool {
        self.directive
            .combinators()
            .all(|combinator| match combinator {
                ImportCombinator::Show(names) => names.names().any(|item| item.name() == name),
                ImportCombinator::Hide(names) => names.names().all(|item| item.name() != name),
            })
    }
}

pub(super) struct ProjectExports<'a> {
    pub paths: Vec<PathBuf>,
    pub packages: Vec<Option<usize>>,
    pub public: Vec<bool>,
    pub internal: Vec<bool>,
    pub barrels: Vec<usize>,
    pub declarations: Vec<Declaration<'a>>,
    pub exports: Vec<Vec<Export<'a>>>,
    pub namespaces: Vec<HashSet<usize>>,
}

impl<'a> ProjectExports<'a> {
    pub fn new(
        environment: &AnalysisEnvironment,
        files: &'a [ProjectFile],
        include_conditional: bool,
    ) -> Self {
        let file_packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(&file.path))
            .collect();
        let identities = project_packages(&file_packages);
        let paths: Vec<_> = files
            .iter()
            .map(|file| canonical_package_path(&file.path))
            .collect();
        let packages: Vec<_> = file_packages
            .iter()
            .map(|file| {
                file.identity().and_then(|identity| {
                    identities
                        .iter()
                        .position(|package| package.lib_root == identity.lib_root)
                })
            })
            .collect();
        let mut public = vec![false; files.len()];
        let mut internal = vec![false; files.len()];
        let mut declarations = Vec::new();
        let mut namespaces = vec![HashSet::new(); files.len()];
        for (index, file) in files.iter().enumerate() {
            let Some(package) = packages[index] else {
                continue;
            };
            if file.has_parse_errors {
                continue;
            }
            if let Ok(relative) = paths[index].strip_prefix(&identities[package].lib_root) {
                internal[index] = relative.starts_with("src");
                public[index] = !internal[index] && file.program().part_of_directive().is_none();
            }
            for declaration in file.program().declarations() {
                collect_declaration(
                    declaration,
                    index,
                    &mut declarations,
                    &mut namespaces[index],
                );
            }
        }
        let exports = collect_exports(
            files,
            &paths,
            &packages,
            &identities,
            &mut namespaces,
            include_conditional,
        );
        let mut additions = Vec::new();
        loop {
            for (index, edges) in exports.iter().enumerate() {
                for edge in edges {
                    for &declaration in &namespaces[edge.target] {
                        if !namespaces[index].contains(&declaration)
                            && edge.includes(declarations[declaration].name)
                        {
                            additions.push((index, declaration));
                        }
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            for (index, declaration) in additions.drain(..) {
                namespaces[index].insert(declaration);
            }
        }
        let mut barrels: Vec<_> = files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                (public[index] && file.program().exports().next().is_some()).then_some(index)
            })
            .collect();
        barrels.sort_unstable_by(|&left, &right| paths[left].cmp(&paths[right]));
        Self {
            paths,
            packages,
            public,
            internal,
            barrels,
            declarations,
            exports,
            namespaces,
        }
    }

    pub fn reachable(&self, from: usize) -> HashSet<usize> {
        let mut seen = HashSet::new();
        let mut pending = vec![from];
        while let Some(index) = pending.pop() {
            if seen.insert(index) {
                pending.extend(self.exports[index].iter().map(|edge| edge.target));
            }
        }
        seen
    }
}

fn collect_exports<'a>(
    files: &'a [ProjectFile],
    paths: &[PathBuf],
    packages: &[Option<usize>],
    identities: &[falcon_analyze::PackageIdentity],
    namespaces: &mut [HashSet<usize>],
    include_conditional: bool,
) -> Vec<Vec<Export<'a>>> {
    let by_path: HashMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| (path.as_path(), index))
        .collect();
    let mut exports: Vec<Vec<Export<'a>>> = (0..files.len()).map(|_| Vec::new()).collect();
    let mut part_names = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let Some(package) = packages[index] else {
            continue;
        };
        if file.has_parse_errors || file.program().part_of_directive().is_some() {
            continue;
        }
        for directive in file.program().exports() {
            let alternatives = directive.configurable_uris();
            let uris = std::iter::once(directive.uri()).chain(
                alternatives
                    .filter(|_| include_conditional)
                    .map(ConfigurableUri::uri),
            );
            for uri in uris {
                let Some(decoded) = falcon_analyze::percent_decode(&uri.value()) else {
                    continue;
                };
                let Some(ResolvedRef::Path(path)) = resolve_directive_uri(
                    &paths[index],
                    &decoded,
                    Some(&identities[package]),
                    identities,
                ) else {
                    continue;
                };
                let Some(&target) = by_path.get(path.as_path()) else {
                    continue;
                };
                if packages[target] == Some(package)
                    && !files[target].has_parse_errors
                    && files[target].program().part_of_directive().is_none()
                {
                    exports[index].push(Export { target, directive });
                }
            }
        }
        for directive in file.program().part_directives() {
            let Some(decoded) = falcon_analyze::percent_decode(&directive.uri().value()) else {
                continue;
            };
            let Some(ResolvedRef::Path(path)) = resolve_directive_uri(
                &paths[index],
                &decoded,
                Some(&identities[package]),
                identities,
            ) else {
                continue;
            };
            if let Some(&target) = by_path.get(path.as_path())
                && packages[target] == Some(package)
                && files[target].program().part_of_directive().is_some()
            {
                part_names.extend(namespaces[target].iter().copied());
                namespaces[index].extend(part_names.drain(..));
            }
        }
    }
    exports
}

fn collect_declaration<'a>(
    declaration: TopLevelDecl<'a>,
    file: usize,
    declarations: &mut Vec<Declaration<'a>>,
    namespace: &mut HashSet<usize>,
) {
    let mut push = |name: Identifier<'a>, annotations: AstChildren<'a, Annotation<'a>>| {
        if name.name().starts_with('_') {
            return;
        }
        let exempt = name.name() == "main"
            || annotations.into_iter().any(|annotation| {
                annotation
                    .name()
                    .last()
                    .is_some_and(|name| matches!(name.name(), "internal" | "visibleForTesting"))
            });
        namespace.insert(declarations.len());
        declarations.push(Declaration {
            name: name.name(),
            span: name.span(),
            file,
            exempt,
        });
    };
    match declaration {
        TopLevelDecl::Class(value) => push(value.name(), value.annotations()),
        TopLevelDecl::ClassTypeAlias(value) => push(value.name(), value.annotations()),
        TopLevelDecl::Mixin(value) => push(value.name(), value.annotations()),
        TopLevelDecl::MixinClass(value) => push(value.name(), value.annotations()),
        TopLevelDecl::Enum(value) => push(value.name(), value.annotations()),
        TopLevelDecl::ExtensionType(value) => push(value.name(), value.annotations()),
        TopLevelDecl::TypeAlias(value) => push(value.name(), value.annotations()),
        TopLevelDecl::Function(value) => push(value.name(), value.annotations()),
        TopLevelDecl::Variable(value) => {
            for item in value.declarators() {
                push(item.name(), value.annotations());
            }
        }
        TopLevelDecl::Extension(value) => {
            if let Some(name) = value.name() {
                push(name, value.annotations());
            }
        }
        TopLevelDecl::Error(_) => {}
    }
}
