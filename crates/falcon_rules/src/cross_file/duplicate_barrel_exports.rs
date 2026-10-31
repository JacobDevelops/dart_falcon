//! Report conflicting declarations exported through independent public barrels.
//!
//! Public barrels are analyzed non-part libraries under `lib/`, outside `lib/src/`,
//! containing an export directive. Export namespaces include public declarations
//! and transitive same-package exports, respecting `show`/`hide` and the default
//! conditional URI. If either barrel transitively exports the other, they are exempt, so
//! umbrella barrels are allowed. Reports once per export directive in the later
//! barrel by path, choosing the first matching earlier barrel and name in sorted
//! order. Private names, `main`, `@internal`, and `@visibleForTesting` declarations
//! are excluded. Re-exporting the same declaration through independent barrels is allowed.
//! Only analyzed, successfully parsed same-package files participate;
//! no SDK or dependency namespaces are inferred.

use std::collections::{BTreeSet, HashMap, HashSet};

use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};

use super::barrels::ProjectExports;

#[derive(Default)]
pub struct DuplicateBarrelExports;

impl CrossFileRule for DuplicateBarrelExports {
    fn prepare_project(
        &self,
        environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let project = ProjectExports::new(environment, files, false);
        let reachable: HashMap<_, _> = project
            .barrels
            .iter()
            .map(|&index| (index, project.reachable(index)))
            .collect();
        let names_by_barrel: HashMap<_, HashMap<_, HashSet<_>>> = project
            .barrels
            .iter()
            .map(|&index| {
                let mut names: HashMap<_, HashSet<_>> = HashMap::new();
                for &declaration in &project.namespaces[index] {
                    let item = &project.declarations[declaration];
                    if !item.exempt {
                        names.entry(item.name).or_default().insert(declaration);
                    }
                }
                (index, names)
            })
            .collect();
        let mut diagnostics = Vec::new();
        for (position, &later) in project.barrels.iter().enumerate() {
            let independent: Vec<_> = project.barrels[..position]
                .iter()
                .copied()
                .filter(|&earlier| {
                    project.packages[earlier] == project.packages[later]
                        && !reachable[&earlier].contains(&later)
                        && !reachable[&later].contains(&earlier)
                })
                .collect();
            for export in &project.exports[later] {
                let names: BTreeSet<_> = project.namespaces[export.target]
                    .iter()
                    .filter_map(|&index| {
                        let declaration = &project.declarations[index];
                        (!declaration.exempt && export.includes(declaration.name))
                            .then_some((declaration.name, index))
                    })
                    .collect();
                let duplicate = independent.iter().find_map(|&earlier| {
                    names
                        .iter()
                        .find(|&&(name, declaration)| {
                            names_by_barrel[&earlier].get(name).is_some_and(|others| {
                                others.iter().any(|&other| other != declaration)
                            })
                        })
                        .map(|&(name, _)| (earlier, name))
                });
                let Some((earlier, name)) = duplicate else {
                    continue;
                };
                let other = project.paths[earlier]
                    .strip_prefix(
                        environment
                            .file_package(&files[earlier].path)
                            .identity()
                            .and_then(|package| package.lib_root.parent())
                            .unwrap_or_else(|| std::path::Path::new("")),
                    )
                    .unwrap_or(&project.paths[earlier])
                    .to_string_lossy()
                    .replace('\\', "/");
                diagnostics.push(Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!("Another `{name}` is exported by `{other}`; importing both barrels is ambiguous."),
                    files[later].path.to_string_lossy().as_ref(),
                    export.directive.uri().span(),
                ));
            }
        }
        diagnostics
    }
}
