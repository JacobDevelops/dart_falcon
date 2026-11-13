use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use falcon_analyze::{AnalysisEnvironment, ProjectFile};
use falcon_syntax::cst::*;

use super::{ResolvedRef, project_packages, resolve_directive_uri};

pub(super) struct Declaration<'a> {
    pub name: &'a str,
    pub span: Span,
    pub file: usize,
    pub exempt: bool,
    kind: DeclarationKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeclarationKind {
    Getter,
    Setter,
    Other,
}

impl Declaration<'_> {
    pub fn conflicts_with(&self, other: &Self) -> bool {
        !matches!(
            (self.kind, other.kind),
            (DeclarationKind::Getter, DeclarationKind::Setter)
                | (DeclarationKind::Setter, DeclarationKind::Getter)
        )
    }
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
            .map(|file| environment.file_package(file.path()))
            .collect();
        let identities = project_packages(&file_packages);
        let paths: Vec<_> = files
            .iter()
            .map(|file| environment.canonical_path(file.path()))
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
            if file.has_parse_errors() {
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
            environment,
            files,
            &paths,
            &packages,
            &identities,
            &mut namespaces,
            include_conditional,
        );
        propagate_exports(&declarations, &exports, &mut namespaces);
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

fn propagate_exports(
    declarations: &[Declaration<'_>],
    exports: &[Vec<Export<'_>>],
    namespaces: &mut [HashSet<usize>],
) {
    // Parts have already contributed their declarations to the owner's locals.
    let locals: Vec<HashMap<_, Vec<_>>> = namespaces
        .iter()
        .map(|namespace| {
            let mut names: HashMap<_, Vec<_>> = HashMap::with_capacity(namespace.len());
            for &id in namespace {
                names.entry(declarations[id].name).or_default().push(id);
            }
            names
        })
        .collect();
    let mut additions = Vec::new();
    loop {
        for (index, edges) in exports.iter().enumerate() {
            for edge in edges {
                for &declaration in &namespaces[edge.target] {
                    if !namespaces[index].contains(&declaration)
                        && edge.includes(declarations[declaration].name)
                        && !locals[index]
                            .get(declarations[declaration].name)
                            .is_some_and(|ids| {
                                ids.iter().any(|&id| {
                                    declarations[id].conflicts_with(&declarations[declaration])
                                })
                            })
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
}

fn collect_exports<'a>(
    environment: &AnalysisEnvironment,
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
    let mut parts = vec![Vec::new(); files.len()];
    for (index, file) in files.iter().enumerate() {
        let Some(package) = packages[index] else {
            continue;
        };
        if file.has_parse_errors() {
            continue;
        }
        for directive in file
            .program()
            .exports()
            .filter(|_| file.program().part_of_directive().is_none())
        {
            let alternatives = directive.configurable_uris();
            let uris = std::iter::once(directive.uri()).chain(
                alternatives
                    .filter(|_| include_conditional)
                    .map(ConfigurableUri::uri),
            );
            for uri in uris {
                let Some(ResolvedRef::Path(path)) = resolve_directive_uri(
                    environment,
                    &paths[index],
                    &uri.value(),
                    Some(&identities[package]),
                    identities,
                ) else {
                    continue;
                };
                let Some(&target) = by_path.get(path.as_path()) else {
                    continue;
                };
                if packages[target] == Some(package)
                    && !files[target].has_parse_errors()
                    && files[target].program().part_of_directive().is_none()
                {
                    exports[index].push(Export { target, directive });
                }
            }
        }
        for directive in file.program().part_directives() {
            let Some(ResolvedRef::Path(path)) = resolve_directive_uri(
                environment,
                &paths[index],
                &directive.uri().value(),
                Some(&identities[package]),
                identities,
            ) else {
                continue;
            };
            if let Some(&target) = by_path.get(path.as_path())
                && packages[target] == Some(package)
                && !files[target].has_parse_errors()
                && files[target].program().part_of_directive().is_some()
            {
                parts[index].push(target);
            }
        }
    }
    let mut additions = Vec::new();
    loop {
        for (index, targets) in parts.iter().enumerate() {
            for &target in targets {
                for &declaration in &namespaces[target] {
                    if !namespaces[index].contains(&declaration) {
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
    exports
}

fn collect_declaration<'a>(
    declaration: TopLevelDecl<'a>,
    file: usize,
    declarations: &mut Vec<Declaration<'a>>,
    namespace: &mut HashSet<usize>,
) {
    let kind = match declaration {
        TopLevelDecl::Function(value) => match value.kind() {
            FunctionKind::Getter => DeclarationKind::Getter,
            FunctionKind::Setter(_) => DeclarationKind::Setter,
            FunctionKind::Function(_) => DeclarationKind::Other,
        },
        _ => DeclarationKind::Other,
    };
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
            kind,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cross_file::{build_reference_graph, exported_library_files};

    fn files(sources: &[(&str, &str)]) -> Vec<ProjectFile> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/duplicate-barrel-exports/cross-file/good_accessors");
        sources
            .iter()
            .map(|(path, source)| {
                ProjectFile::new(
                    root.join(path),
                    std::sync::Arc::new(falcon_analyze::ParsedFile::parse(*source)),
                )
                .unwrap()
            })
            .collect()
    }

    fn environment(files: &[ProjectFile]) -> AnalysisEnvironment {
        AnalysisEnvironment::discover_with_sdk_locator(files.iter().map(ProjectFile::path), |_| {
            None
        })
    }

    #[cfg(unix)]
    #[test]
    fn exported_parts_resolve_from_the_same_spelling_as_exports() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("alias")).unwrap();
        std::fs::create_dir_all(root.path().join("real")).unwrap();
        std::fs::write(root.path().join("real/owner.dart"), "part 'child.dart';").unwrap();
        std::os::unix::fs::symlink("../real/owner.dart", root.path().join("alias/owner.dart"))
            .unwrap();
        let files: Vec<_> = [
            ("api.dart", "export 'alias/owner.dart';"),
            ("alias/owner.dart", "part 'child.dart';"),
            ("alias/child.dart", "part of 'owner.dart'; class Child {}"),
        ]
        .into_iter()
        .map(|(path, source)| {
            ProjectFile::new(
                root.path().join(path),
                std::sync::Arc::new(falcon_analyze::ParsedFile::parse(source)),
            )
            .unwrap()
        })
        .collect();
        let environment = environment(&files);
        let packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(file.path()))
            .collect();
        let exported = exported_library_files(
            &environment,
            &files,
            &packages,
            &project_packages(&packages),
        );
        assert!(exported.contains(&environment.canonical_path(files[2].path())));
    }

    #[test]
    fn local_accessors_preserve_complementary_exports() {
        let files = files(&[
            ("lib/src/write.dart", "set count(int value) {}"),
            (
                "lib/api.dart",
                "export 'src/write.dart'; int get count => 0;",
            ),
            ("lib/src/read.dart", "int get value => 0;"),
            (
                "lib/write.dart",
                "export 'src/read.dart'; set value(int v) {}",
            ),
        ]);
        let environment = environment(&files);
        let project = ProjectExports::new(&environment, &files, true);
        assert_eq!(project.namespaces[1].len(), 2);
        assert_eq!(project.namespaces[3].len(), 2);
    }

    #[test]
    fn nested_parts_contribute_to_exported_libraries() {
        let files = files(&[
            ("lib/api.dart", "export 'src/owner.dart';"),
            (
                "lib/src/nested.dart",
                "part of 'part.dart'; class Nested {}",
            ),
            (
                "lib/src/part.dart",
                "part of 'owner.dart'; part 'nested.dart';",
            ),
            ("lib/src/owner.dart", "part 'part.dart';"),
        ]);
        let environment = environment(&files);
        let project = ProjectExports::new(&environment, &files, true);
        assert_eq!(project.namespaces[0].len(), 1);
        let file_packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(file.path()))
            .collect();
        let exported = exported_library_files(
            &environment,
            &files,
            &file_packages,
            &project_packages(&file_packages),
        );
        assert!(exported.contains(&environment.canonical_path(files[1].path())));
    }

    #[test]
    fn encoded_scheme_separator_stays_a_relative_path() {
        let from = std::path::Path::new("/project/lib/main.dart");
        let Some(ResolvedRef::Path(path)) = resolve_directive_uri(
            &AnalysisEnvironment::default(),
            from,
            "package%3Afoo/a.dart",
            None,
            &[],
        ) else {
            panic!("Encoded scheme separator must remain relative");
        };
        assert_eq!(
            path,
            std::path::Path::new("/project/lib/package:foo/a.dart")
        );
    }

    #[test]
    fn getters_and_setters_share_a_name_without_conflicting() {
        let files = files(&[
            ("lib/read.dart", "int get count => 0;"),
            ("lib/write.dart", "set count(int value) {}"),
            ("lib/another.dart", "int get count => 1;"),
            ("lib/api.dart", "export 'read.dart'; export 'write.dart';"),
        ]);
        let environment = environment(&files);
        let project = ProjectExports::new(&environment, &files, true);
        let getter = &project.declarations[0];
        let setter = &project.declarations[1];
        assert!(!getter.conflicts_with(setter));
        assert!(!setter.conflicts_with(getter));
        assert!(getter.conflicts_with(&project.declarations[2]));
        assert_eq!(project.namespaces[3].len(), 2);
    }

    #[test]
    fn parts_contribute_to_the_exported_library() {
        let files = files(&[
            ("lib/api.dart", "export 'src/owner.dart';"),
            (
                "lib/src/owner.dart",
                "part 'plain.dart'; part '%67enerated.dart';",
            ),
            ("lib/src/plain.dart", "part of 'owner.dart'; class Plain {}"),
            (
                "lib/src/generated.dart",
                "part of 'owner.dart'; class Generated {}",
            ),
        ]);
        let environment = environment(&files);
        let project = ProjectExports::new(&environment, &files, true);
        assert_eq!(project.namespaces[0].len(), 2);
        assert_eq!(project.namespaces[0], project.namespaces[1]);
        let file_packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(file.path()))
            .collect();
        let exported = exported_library_files(
            &environment,
            &files,
            &file_packages,
            &project_packages(&file_packages),
        );
        for file in &files[1..] {
            assert!(exported.contains(&environment.canonical_path(file.path())));
        }
    }

    #[test]
    fn every_conditional_export_branch_contributes_api() {
        let files = files(&[
            (
                "lib/api.dart",
                "export 'stub.dart' if (dart.library.io) 'io.dart' if (dart.library.html) 'web.dart';",
            ),
            ("lib/stub.dart", "class Stub {}"),
            ("lib/io.dart", "class Native {}"),
            ("lib/web.dart", "class Web {}"),
        ]);
        let environment = environment(&files);
        let project = ProjectExports::new(&environment, &files, true);
        assert_eq!(project.namespaces[0].len(), 3);
        assert_eq!(
            ProjectExports::new(&environment, &files, false).namespaces[0].len(),
            1
        );
        let file_packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(file.path()))
            .collect();
        let exported = exported_library_files(
            &environment,
            &files,
            &file_packages,
            &project_packages(&file_packages),
        );
        for file in &files[1..] {
            assert!(exported.contains(&environment.canonical_path(file.path())));
        }
    }

    #[test]
    fn local_names_shadow_exports_before_transitive_propagation() {
        let files = files(&[
            ("lib/src/original.dart", "class Name {} class Other {}"),
            ("lib/a.dart", "export 'src/original.dart'; class Name {}"),
            ("lib/b.dart", "export 'a.dart' show Name;"),
            ("lib/c.dart", "export 'b.dart'; export 'a.dart' hide Name;"),
        ]);
        let environment = environment(&files);
        let project = ProjectExports::new(&environment, &files, true);
        let names = |file: usize| {
            project.namespaces[file]
                .iter()
                .map(|&id| (project.declarations[id].name, project.declarations[id].file))
                .collect::<HashSet<_>>()
        };
        assert_eq!(names(1), HashSet::from([("Name", 1), ("Other", 0)]));
        assert_eq!(names(2), HashSet::from([("Name", 1)]));
        assert_eq!(names(3), HashSet::from([("Name", 1), ("Other", 0)]));
    }

    #[test]
    fn encoded_uris_are_decoded_once_in_fast_and_fallback_resolution() {
        let files = files(&[
            (
                "lib/root.dart",
                "import '%61.dart'; import '%2561.dart'; import 'src/../%61.dart'; import 'package:fixture/%61.dart'; import 'package:fixture/%2561.dart';",
            ),
            ("lib/a.dart", ""),
            ("lib/%61.dart", ""),
        ]);
        let environment = environment(&files);
        let file_packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(file.path()))
            .collect();
        let packages = project_packages(&file_packages);
        let paths: Vec<_> = files
            .iter()
            .map(|file| environment.canonical_path(file.path()))
            .collect();
        let graph = build_reference_graph(&environment, &files, &paths, &file_packages, &packages);
        assert_eq!(graph[0], [1, 2, 1, 1, 2]);
        for (uri, target) in [("%61.dart", 1), ("%2561.dart", 2), ("src/../%61.dart", 1)] {
            let Some(ResolvedRef::Path(path)) = resolve_directive_uri(
                &environment,
                &paths[0],
                uri,
                file_packages[0].identity(),
                &packages,
            ) else {
                panic!("URI should resolve: {uri}");
            };
            assert_eq!(path, paths[target]);
        }
        let Some(ResolvedRef::Path(literal)) =
            resolve_directive_uri(&environment, &paths[0], "%zz.dart", None, &[])
        else {
            panic!("A literal percent sign is a valid path");
        };
        assert_eq!(
            literal,
            environment.canonical_path(&paths[0].parent().unwrap().join("%zz.dart"))
        );
    }
}
