//! Report mutually dependent components of imported or exported library files.
//!
//! Cycles couple libraries and make dependency boundaries harder to maintain.
//! Each diagnostic includes the complete chain, ending back at its first file.
//! Import and export targets include every conditional alternative; `part` and
//! `part of` directives are not dependency edges. Only analyzed, resolved files
//! participate; package URIs without a known package identity are skipped.
//! Canonical path aliases and repeated directives share one edge.
//! Each mutually dependent component is reported once, including self-imports.
//! The chain is a shortest cycle through its lexicographically first canonical
//! file, not an enumeration of all cycles within that component.
//! Configure this project-wide rule under the `cross-file` section.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};

use super::{
    ResolvedRef, canonical_package_path, enclosing_package, enclosing_pubspec,
    resolve_directive_uri,
};

#[derive(Default)]
pub struct NoImportCycles;

impl CrossFileRule for NoImportCycles {
    fn prepare_project(
        &self,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut representatives = BTreeMap::new();
        for (index, file) in files.iter().enumerate() {
            let canonical = canonical_package_path(&file.path);
            let representative = representatives.entry(canonical).or_insert(index);
            // Path ordering normalizes `.` components; compare original spellings.
            if file.path.as_os_str() < files[*representative].path.as_os_str() {
                *representative = index;
            }
        }
        let (paths, representatives): (Vec<PathBuf>, Vec<usize>) =
            representatives.into_iter().unzip();
        let by_path: HashMap<&Path, usize> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.as_path(), index))
            .collect();
        let mut graph = vec![Vec::new(); paths.len()];
        for (index, &representative) in representatives.iter().enumerate() {
            let file = &files[representative];
            let package = enclosing_package(&paths[index]);
            let program = file.program();
            let imports = program.imports().flat_map(|directive| {
                std::iter::once(directive.uri()).chain(
                    directive
                        .configurable_uris()
                        .map(|alternative| alternative.uri()),
                )
            });
            let exports = program.exports().flat_map(|directive| {
                std::iter::once(directive.uri()).chain(
                    directive
                        .configurable_uris()
                        .map(|alternative| alternative.uri()),
                )
            });
            for uri in imports.chain(exports) {
                let target =
                    match resolve_directive_uri(&paths[index], &uri.value(), package.as_ref()) {
                        Some(ResolvedRef::Path(path)) => by_path
                            .get(canonical_package_path(&path).as_path())
                            .copied(),
                        // Without package identity a suffix match cannot prove an import.
                        Some(ResolvedRef::Suffix(_)) | None => None,
                    };
                if let Some(target) = target {
                    let span = uri.span();
                    graph[index].push((
                        target,
                        Span {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }
            graph[index].sort_by_key(|(target, span)| (*target, span.start));
            graph[index].dedup_by_key(|(target, _)| *target);
        }

        let mut reverse = vec![Vec::new(); graph.len()];
        for (source, edges) in graph.iter().enumerate() {
            for &(target, _) in edges {
                reverse[target].push(source);
            }
        }
        let (components, sizes) = strongly_connected_components(&graph, &reverse);
        let mut diagnostics = Vec::new();
        let mut reported = vec![false; sizes.len()];
        let mut parents = vec![usize::MAX; graph.len()];
        let mut pending = Vec::with_capacity(graph.len());
        let mut cycle = Vec::with_capacity(graph.len() + 1);
        for (start, edges) in graph.iter().enumerate() {
            let component = components[start];
            if reported[component]
                || (sizes[component] == 1 && !edges.iter().any(|&(target, _)| target == start))
            {
                continue;
            }
            reported[component] = true;
            parents[start] = start;
            pending.clear();
            pending.push(start);
            let mut cursor = 0;
            let mut closing = None;
            'search: while let Some(&source) = pending.get(cursor) {
                cursor += 1;
                for &(target, _) in &graph[source] {
                    if target == start {
                        closing = Some(source);
                        break 'search;
                    }
                    if components[target] == component && parents[target] == usize::MAX {
                        parents[target] = source;
                        pending.push(target);
                    }
                }
            }
            let mut source =
                closing.expect("A cyclic component contains a cycle through each node");
            cycle.clear();
            cycle.push(source);
            while source != start {
                source = parents[source];
                cycle.push(source);
            }
            cycle.reverse();
            cycle.push(start);
            let span = edges
                .iter()
                .find(|&&(target, _)| target == cycle[1])
                .map(|&(_, span)| span)
                .unwrap();
            let root = enclosing_pubspec(&paths[start])
                .and_then(|manifest| manifest.parent().map(Path::to_path_buf));
            let mut message = String::from("Remove a dependency to break this import/export cycle: `");
            for (index, &node) in cycle.iter().enumerate() {
                if index > 0 {
                    message.push_str("` -> `");
                }
                let path = paths[node].as_path();
                let shown = root
                    .as_deref()
                    .and_then(|root| path.strip_prefix(root).ok())
                    .unwrap_or(path);
                message.push_str(&shown.to_string_lossy());
            }
            message.push_str("`.");
            diagnostics.push(Diagnostic::new(
                "",
                Severity::Warning,
                message,
                files[representatives[start]]
                    .path
                    .to_string_lossy()
                    .into_owned(),
                span,
            ));
        }
        diagnostics
    }
}

fn strongly_connected_components(
    graph: &[Vec<(usize, Span)>],
    reverse: &[Vec<usize>],
) -> (Vec<usize>, Vec<usize>) {
    let mut visited = vec![false; graph.len()];
    let mut order = Vec::with_capacity(graph.len());
    let mut stack = Vec::with_capacity(graph.len());
    for (start, _) in graph.iter().enumerate() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        stack.push((start, 0));
        while let Some((source, next)) = stack.last_mut() {
            if let Some(&(target, _)) = graph[*source].get(*next) {
                *next += 1;
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(*source);
                stack.pop();
            }
        }
    }
    let mut components = vec![usize::MAX; graph.len()];
    let mut sizes = Vec::new();
    let mut pending = Vec::with_capacity(graph.len());
    for &start in order.iter().rev() {
        if components[start] != usize::MAX {
            continue;
        }
        let component = sizes.len();
        let mut size = 0;
        components[start] = component;
        pending.push(start);
        while let Some(target) = pending.pop() {
            size += 1;
            for &source in &reverse[target] {
                if components[source] == usize::MAX {
                    components[source] = component;
                    pending.push(source);
                }
            }
        }
        sizes.push(size);
    }
    (components, sizes)
}

#[cfg(test)]
mod cycle_regressions {
    use super::*;

    #[test]
    fn scc_partition_keeps_cycle_reachable_from_lower_acyclic_node() {
        let span = Span { start: 0, end: 0 };
        let graph = vec![vec![(1, span)], vec![(2, span)], vec![(1, span)]];
        let reverse = vec![vec![], vec![0, 2], vec![1]];
        let (components, sizes) = strongly_connected_components(&graph, &reverse);
        assert_ne!(components[0], components[1]);
        assert_eq!(components[1], components[2]);
        assert_eq!(sizes[components[0]], 1);
        assert_eq!(sizes[components[1]], 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::CrossFileRuleAnalysis;
    use falcon_dart_parser::parser::parse;

    fn file(path: &str, source: &str) -> ProjectFile {
        let falcon_dart_parser::ParseResult { tree, .. } = parse(source);
        ProjectFile {
            path: PathBuf::from(path),
            parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree,
            }),
            has_parse_errors: false,
        }
    }

    fn diagnostics(files: &[ProjectFile]) -> Vec<Diagnostic> {
        NoImportCycles.analyze_project("no-import-cycles", files, &FalconConfig::default(), None)
    }

    #[test]
    fn reports_one_shortest_cycle_per_mutually_dependent_component() {
        let files = [
            file("/cycle/a.dart", "import 'b.dart'; import 'c.dart';"),
            file("/cycle/b.dart", "import 'a.dart'; import 'c.dart';"),
            file("/cycle/c.dart", "import 'a.dart';"),
        ];
        let diagnostics = diagnostics(&files);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("`/cycle/a.dart` -> `/cycle/b.dart` -> `/cycle/a.dart`")
        );
    }

    #[test]
    fn unique_unknown_package_suffix_is_not_a_local_import() {
        let files = [
            file("/cycle/lib/a.dart", "import 'package:http/http.dart';"),
            file("/cycle/lib/http.dart", "import 'a.dart';"),
        ];
        assert!(diagnostics(&files).is_empty());
    }

    #[test]
    fn reports_subcycle_after_acyclic_predecessor_and_separate_component() {
        let files = [
            file("/cycle/a.dart", "import 'b.dart';"),
            file("/cycle/b.dart", "import 'c.dart';"),
            file("/cycle/c.dart", "import 'b.dart';"),
            file("/cycle/d.dart", "import 'e.dart';"),
            file("/cycle/e.dart", "import 'd.dart';"),
        ];
        let diagnostics = diagnostics(&files);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].file_path, "/cycle/b.dart");
        assert!(
            diagnostics[0]
                .message
                .contains("`/cycle/b.dart` -> `/cycle/c.dart` -> `/cycle/b.dart`")
        );
        assert_eq!(diagnostics[1].file_path, "/cycle/d.dart");
        assert!(
            diagnostics[1]
                .message
                .contains("`/cycle/d.dart` -> `/cycle/e.dart` -> `/cycle/d.dart`")
        );
    }

    #[test]
    fn canonical_aliases_and_input_order_do_not_duplicate_or_rotate_cycles() {
        let mut files = [
            file("/cycle/b.dart", "import 'a.dart';"),
            file("/cycle/a.dart", "import './b.dart'; export 'b.dart';"),
            file("/cycle/./a.dart", "import './b.dart'; export 'b.dart';"),
        ];
        let first = diagnostics(&files);
        assert_eq!(first.len(), 1);
        files.reverse();
        let second = diagnostics(&files);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0].file_path, second[0].file_path);
        assert_eq!(first[0].message, second[0].message);
        assert_eq!(first[0].span, second[0].span);
    }

    #[test]
    fn part_edges_cannot_complete_an_import_cycle() {
        let files = [
            file("/cycle/a.dart", "part 'b.dart';"),
            file("/cycle/b.dart", "import 'a.dart';"),
        ];
        assert!(diagnostics(&files).is_empty());
    }

    #[test]
    fn both_conditional_directive_kinds_participate() {
        for directive in ["import", "export"] {
            let files = [
                file(
                    "/cycle/a.dart",
                    &format!("{directive} 'stub.dart' if (dart.library.io) 'b.dart';"),
                ),
                file("/cycle/b.dart", "import 'a.dart';"),
                file("/cycle/stub.dart", "class Stub {}"),
            ];
            let diagnostics = diagnostics(&files);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(
                &files[0].parsed.source[diagnostics[0].span.start..diagnostics[0].span.end],
                "'b.dart'"
            );
        }
    }

    #[test]
    fn ambiguous_package_suffix_is_not_a_confirmed_dependency() {
        let files = [
            file("/cycle/lib/a.dart", "import 'package:unknown/b.dart';"),
            file("/one/lib/b.dart", "import '../../cycle/lib/a.dart';"),
            file("/two/lib/b.dart", "class Other {}"),
        ];
        assert!(diagnostics(&files).is_empty());
    }

    #[test]
    fn self_cycle_is_reported_once_for_repeated_directives() {
        let files = [file("/cycle/a.dart", "import 'a.dart'; export './a.dart';")];
        let diagnostics = diagnostics(&files);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("`/cycle/a.dart` -> `/cycle/a.dart`")
        );
    }
}
