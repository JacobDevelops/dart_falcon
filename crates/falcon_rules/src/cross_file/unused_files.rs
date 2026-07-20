//! Report `lib/` files that no project entrypoint can reach.
//!
//! Models the file set as a directed graph (file → every file its
//! imports/exports/parts reference) and flags a `lib/src/**` file when no
//! entrypoint reaches it. Entrypoints ("roots") are: any analyzed file outside
//! `lib/` (bin/test/tool/example, or their own concern), any file declaring a
//! top-level `main`, and the public library surface — every `lib/` file not
//! under `lib/src/`, which external packages may import (this subsumes the
//! `lib/<pkg>.dart` barrel). Reachability, not mere reference count, is the test:
//! a cluster of `lib/src` files that reference only each other but that nothing
//! live reaches is dead and is flagged. `part of` files belong to their owning
//! library and are never flagged directly. Without a pubspec the scope narrows
//! to files with a `lib/` path component and the public-surface exemption is not
//! applied (no `lib/` root is known). This is a cross-file rule: it runs in the
//! cross-file pass and is configured under the top-level `cross-file` section.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::{Program, TopLevelDecl};

use super::{
    PackageInfo, build_reference_graph, canonical_or_lexical, detect_package, has_lib_component,
    is_under_lib,
};

pub struct UnusedFiles;

const NAME: &str = "unused-files";

impl CrossFileRule for UnusedFiles {
    fn name(&self) -> &'static str {
        NAME
    }

    fn analyze_project(&self, files: &[ProjectFile], _config: &FalconConfig) -> Vec<Diagnostic> {
        let pkg = detect_package(files);
        let canons: Vec<PathBuf> = files
            .iter()
            .map(|f| canonical_or_lexical(&f.path))
            .collect();
        let adj = build_reference_graph(files, &canons, pkg.as_ref());

        // Two spellings of one real file must share reachability, so both the
        // BFS and the flag check below go through the first index seen for a
        // canonical path (edge targets already collapse to it).
        let mut first_for_canon: HashMap<&Path, usize> = HashMap::with_capacity(canons.len());
        for (i, c) in canons.iter().enumerate() {
            first_for_canon.entry(c.as_path()).or_insert(i);
        }
        let repr: Vec<usize> = canons.iter().map(|c| first_for_canon[c.as_path()]).collect();

        // BFS from every entrypoint over the reference edges.
        let mut reachable = vec![false; files.len()];
        let mut queue = VecDeque::new();
        for (i, f) in files.iter().enumerate() {
            if is_root(&f.program, &canons[i], pkg.as_ref()) && !reachable[repr[i]] {
                reachable[repr[i]] = true;
                queue.push_back(repr[i]);
            }
        }
        while let Some(i) = queue.pop_front() {
            for &j in &adj[i] {
                if !reachable[j] {
                    reachable[j] = true;
                    queue.push_back(j);
                }
            }
        }

        let mut diags = Vec::new();
        for (i, f) in files.iter().enumerate() {
            let canon = &canons[i];
            // Only files under lib/ are candidates. Without a pubspec the rule's
            // documented scope still applies: require a `lib/` path component so
            // test/bin/tool files aren't flagged.
            if !is_under_lib(canon, pkg.as_ref()) {
                continue;
            }
            if pkg.is_none() && !has_lib_component(canon) {
                continue;
            }
            // A `part of` file belongs to its owner and is never a standalone unit.
            if f.program.part_of_directive.is_some() {
                continue;
            }
            if reachable[repr[i]] {
                continue;
            }
            // One diagnostic per real file: aliases defer to the representative.
            if repr[i] != i {
                continue;
            }
            diags.push(Diagnostic::new(
                NAME,
                Severity::Warning,
                "File is not reachable from any entrypoint (main, a non-lib file, or the public \
                 library surface); it may be dead code",
                f.path.to_string_lossy().into_owned(),
                DiagSpan { start: 0, end: 0 },
            ));
        }
        diags
    }
}

/// Whether `canon` is an entrypoint that is always considered used, seeding the
/// reachability search. See the module docs for the three root kinds.
fn is_root(program: &Program, canon: &Path, pkg: Option<&PackageInfo>) -> bool {
    if has_main(program) {
        return true;
    }
    match pkg {
        Some(pkg) => {
            // Anything outside lib/ is an entrypoint or its own concern.
            if !canon.starts_with(&pkg.lib_root) {
                return true;
            }
            // Public library surface: under lib/ but not lib/src/. External
            // packages may import these, so they are always used.
            !canon.starts_with(pkg.lib_root.join("src"))
        }
        // No pubspec: files outside lib/ are roots; every lib/ file is a
        // candidate (no lib root is known, so no public-surface exemption).
        None => !has_lib_component(canon),
    }
}

/// Whether the program declares a top-level `main` function.
fn has_main(program: &Program) -> bool {
    program.declarations.iter().any(|d| {
        matches!(d, TopLevelDecl::Function(f) if f.name.name == "main" && !f.is_getter && !f.is_setter)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parser::parse;

    fn pf(path: &str, src: &str) -> ProjectFile {
        let (program, _errors) = parse(src);
        ProjectFile {
            path: PathBuf::from(path),
            source: src.to_string(),
            program,
            has_parse_errors: false,
        }
    }

    fn flagged(files: &[ProjectFile]) -> Vec<String> {
        let cfg = FalconConfig::default();
        let mut v: Vec<String> = UnusedFiles
            .analyze_project(files, &cfg)
            .into_iter()
            .map(|d| d.file_path)
            .collect();
        v.sort();
        v
    }

    // The upgrade's reason for being: two lib/src files that reference only each
    // other, with no entrypoint reaching them, are a dead island — both flagged.
    // Reference counting alone would call each "used" by the other.
    #[test]
    fn mutually_referencing_island_both_flagged() {
        let files = vec![
            pf(
                "/proj/lib/src/a.dart",
                "import 'b.dart';\nvoid useB() { b(); }\n",
            ),
            pf(
                "/proj/lib/src/b.dart",
                "import 'a.dart';\nvoid b() {}\nvoid useA() { useB(); }\n",
            ),
        ];
        assert_eq!(
            flagged(&files),
            vec![
                "/proj/lib/src/a.dart".to_string(),
                "/proj/lib/src/b.dart".to_string()
            ]
        );
    }

    // A chain reachable from a `main` entrypoint is entirely live.
    #[test]
    fn reachable_chain_not_flagged() {
        let files = vec![
            pf(
                "/proj/lib/main.dart",
                "import 'src/a.dart';\nvoid main() { a(); }\n",
            ),
            pf(
                "/proj/lib/src/a.dart",
                "import 'b.dart';\nvoid a() { b(); }\n",
            ),
            pf("/proj/lib/src/b.dart", "void b() {}\n"),
        ];
        assert!(flagged(&files).is_empty(), "{:?}", flagged(&files));
    }

    // A `part of` file is reachable through its owning library's `part` edge and
    // is never flagged directly; an unreachable sibling is the control.
    #[test]
    fn part_of_reachable_library_not_flagged() {
        let files = vec![
            pf(
                "/proj/lib/entry.dart",
                "import 'src/feature.dart';\nvoid main() { runFeature(); }\n",
            ),
            pf(
                "/proj/lib/src/feature.dart",
                "part 'feature_part.dart';\nvoid runFeature() { helper(); }\n",
            ),
            pf(
                "/proj/lib/src/feature_part.dart",
                "part of 'feature.dart';\nvoid helper() {}\n",
            ),
            pf("/proj/lib/src/orphan.dart", "void orphan() {}\n"),
        ];
        assert_eq!(
            flagged(&files),
            vec!["/proj/lib/src/orphan.dart".to_string()]
        );
    }

    // Reachability propagates through a `part` edge: a file referenced only by a
    // reachable library's part is itself reachable (falls out of the edge set).
    #[test]
    fn reference_through_reachable_part_keeps_target_live() {
        let files = vec![
            pf(
                "/proj/lib/entry.dart",
                "import 'src/lib_file.dart';\nvoid main() {}\n",
            ),
            pf(
                "/proj/lib/src/lib_file.dart",
                "import 'used_by_lib.dart';\npart 'the_part.dart';\n",
            ),
            pf("/proj/lib/src/the_part.dart", "part of 'lib_file.dart';\n"),
            pf("/proj/lib/src/used_by_lib.dart", "void u() {}\n"),
        ];
        assert!(flagged(&files).is_empty(), "{:?}", flagged(&files));
    }

    // An edge ORIGINATING from a part file keeps its target live: deep.dart is
    // imported only by the_part.dart, which is a part of a reachable library.
    #[test]
    fn import_from_part_file_keeps_target_live() {
        let files = vec![
            pf(
                "/proj/lib/entry.dart",
                "import 'src/owner.dart';\nvoid main() {}\n",
            ),
            pf("/proj/lib/src/owner.dart", "part 'the_part.dart';\n"),
            pf(
                "/proj/lib/src/the_part.dart",
                "part of 'owner.dart';\nimport 'deep.dart';\n",
            ),
            pf("/proj/lib/src/deep.dart", "void d() {}\n"),
        ];
        assert!(flagged(&files).is_empty(), "{:?}", flagged(&files));
    }

    /// Unique on-disk dir so package-aware tests get a real pubspec.yaml.
    fn pkg_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "falcon_unused_{}_{}_{tag}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t").replace("::", "_")
        ));
        std::fs::create_dir_all(d.join("lib/src")).unwrap();
        std::fs::write(d.join("pubspec.yaml"), "name: app\n").unwrap();
        d
    }

    // A `package:` import of a DIFFERENT package must not create a local edge:
    // a dead lib/src file whose path coincides with the external URI's suffix
    // stays flagged (reviewer-confirmed false-negative regression).
    #[test]
    fn external_package_import_creates_no_false_edge() {
        let d = pkg_dir("ext");
        let entry = d.join("lib/app.dart");
        let dead = d.join("lib/src/widgets.dart");
        let files = vec![
            pf(
                entry.to_str().unwrap(),
                "import 'package:thirdparty/src/widgets.dart';\nvoid main() {}\n",
            ),
            pf(dead.to_str().unwrap(), "void w() {}\n"),
        ];
        let out = flagged(&files);
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(out, vec![dead.to_string_lossy().into_owned()]);
    }

    // Two spellings of one real file (via a symlinked parent) share reachability:
    // neither is flagged when the file is reachable, and no duplicate diagnostic
    // is emitted when it is not.
    #[cfg(unix)]
    #[test]
    fn duplicate_canonical_paths_share_reachability() {
        let d = pkg_dir("dup");
        std::fs::write(
            d.join("lib/app.dart"),
            "import 'src/real.dart';\nvoid main() {}\n",
        )
        .unwrap();
        std::fs::write(d.join("lib/src/real.dart"), "void r() {}\n").unwrap();
        std::os::unix::fs::symlink(d.join("lib/src"), d.join("lib/alias")).unwrap();

        let files = vec![
            pf(
                d.join("lib/app.dart").to_str().unwrap(),
                "import 'src/real.dart';\nvoid main() {}\n",
            ),
            pf(d.join("lib/src/real.dart").to_str().unwrap(), "void r() {}\n"),
            pf(d.join("lib/alias/real.dart").to_str().unwrap(), "void r() {}\n"),
        ];
        let out = flagged(&files);
        std::fs::remove_dir_all(&d).unwrap();
        assert!(out.is_empty(), "{out:?}");
    }

    // An UNREACHABLE file seen under two spellings yields exactly one
    // diagnostic, using the representative (first-seen) spelling.
    #[cfg(unix)]
    #[test]
    fn duplicate_canonical_paths_emit_one_diagnostic() {
        let d = pkg_dir("dup_dead");
        std::fs::write(d.join("lib/app.dart"), "void main() {}\n").unwrap();
        std::fs::write(d.join("lib/src/dead.dart"), "void x() {}\n").unwrap();
        std::os::unix::fs::symlink(d.join("lib/src"), d.join("lib/alias")).unwrap();

        let files = vec![
            pf(d.join("lib/app.dart").to_str().unwrap(), "void main() {}\n"),
            pf(d.join("lib/src/dead.dart").to_str().unwrap(), "void x() {}\n"),
            pf(d.join("lib/alias/dead.dart").to_str().unwrap(), "void x() {}\n"),
        ];
        let out = flagged(&files);
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].ends_with("lib/src/dead.dart"), "{out:?}");
    }
}
