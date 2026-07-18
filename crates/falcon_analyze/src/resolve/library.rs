//! Library grouping — partition a project's files into Dart *libraries*.
//!
//! A Dart library is an owner file plus the `part` files it stitches in. Rules
//! and the type index sometimes need a file's siblings (a member declared in a
//! part completes the class in the owner) and a flag for when that stitching is
//! incomplete (a `part`/`part of` we cannot resolve to a file in the set), which
//! must poison any proof-of-absence a resolver draws.
//!
//! Grouping is by union-find over both link directions: an owner's `part 'uri'`
//! directives and a part's `part of` (URI or dotted-name) directive. A link that
//! resolves to a file in the set merges the two; a link that resolves to nothing
//! flags the file's group `has_unresolved_parts`. Files with no directives are
//! each their own single-file library. This mirrors the tolerant path handling
//! in `falcon_rules::project`, re-implemented here to keep `falcon_analyze` free
//! of an upward dependency.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use falcon_syntax::ast::Program;

/// A file's library context: the sibling programs that complete its library and
/// whether that library has an unresolved part.
pub struct LibraryUnit<'a> {
    siblings: Vec<&'a Program>,
    has_unresolved_parts: bool,
}

impl<'a> LibraryUnit<'a> {
    /// The other programs in the same library (the owner's parts, or the owner
    /// for a part file). Empty for a standalone single-file library.
    pub fn siblings(&self) -> &[&'a Program] {
        &self.siblings
    }

    /// Whether the library has a `part`/`part of` link that could not be resolved
    /// to a file in the analyzed set — its declarations may be incomplete.
    pub fn has_unresolved_parts(&self) -> bool {
        self.has_unresolved_parts
    }
}

/// The result of grouping: per-file group membership and unresolved flags.
///
/// Indices are positions in the input file slice.
pub struct LibraryGrouping {
    /// Per file: sibling file indices in the same library (excluding self).
    siblings: Vec<Vec<usize>>,
    /// Per file: whether its library has an unresolved part.
    unresolved: Vec<bool>,
}

impl LibraryGrouping {
    /// Sibling file indices of file `i` (the same-library files, excluding `i`).
    pub fn siblings(&self, i: usize) -> &[usize] {
        &self.siblings[i]
    }

    /// Whether file `i`'s library has an unresolved part.
    pub fn is_unresolved(&self, i: usize) -> bool {
        self.unresolved[i]
    }

    pub fn len(&self) -> usize {
        self.siblings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.siblings.is_empty()
    }
}

/// Group `files` (each a `(path, program)`) into libraries. `path` is only used
/// for `part`-URI resolution; it need not exist on disk (grouping falls back to
/// lexical path normalization when it does not).
pub fn group_libraries(files: &[(PathBuf, &Program)]) -> LibraryGrouping {
    let n = files.len();
    let mut uf = UnionFind::new(n);
    let mut unresolved = vec![false; n];

    // Index files by canonical path and by declared `library <name>;`. A name
    // declared by more than one file is ambiguous (`None`) — links using it stay
    // unresolved rather than silently binding to the first declarer.
    let mut by_path: HashMap<PathBuf, usize> = HashMap::with_capacity(n);
    let mut by_lib_name: HashMap<String, Option<usize>> = HashMap::new();
    for (i, (path, program)) in files.iter().enumerate() {
        by_path.insert(canonical_or_lexical(path), i);
        if let Some(lib) = &program.library_directive {
            let name = dotted(&lib.name);
            if !name.is_empty() {
                by_lib_name
                    .entry(name)
                    .and_modify(|slot| *slot = None)
                    .or_insert(Some(i));
            }
        }
    }

    for (i, (path, program)) in files.iter().enumerate() {
        let dir = path.parent().unwrap_or_else(|| Path::new(""));

        // Owner → part links (`part 'uri';`).
        for part in &program.part_directives {
            match resolve_relative(dir, &part.uri.value).and_then(|p| by_path.get(&p).copied()) {
                Some(j) => uf.union(i, j),
                None => unresolved[i] = true,
            }
        }

        // Part → owner link (`part of 'uri';` or `part of a.b;`).
        if let Some(po) = &program.part_of_directive {
            let owner = if let Some(uri) = &po.uri {
                resolve_relative(dir, &uri.value).and_then(|p| by_path.get(&p).copied())
            } else if !po.name.is_empty() {
                by_lib_name.get(&dotted(&po.name)).copied().flatten()
            } else {
                None
            };
            match owner {
                Some(j) => uf.union(i, j),
                None => unresolved[i] = true,
            }
        }
    }

    // Collect group membership and propagate the unresolved flag to every member.
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        groups.entry(uf.find(i)).or_default().push(i);
    }
    let mut siblings = vec![Vec::new(); n];
    let mut final_unresolved = vec![false; n];
    for members in groups.values() {
        let group_unresolved = members.iter().any(|&m| unresolved[m]);
        for &m in members {
            final_unresolved[m] = group_unresolved;
            siblings[m] = members.iter().copied().filter(|&x| x != m).collect();
        }
    }

    LibraryGrouping {
        siblings,
        unresolved: final_unresolved,
    }
}

/// Build the [`LibraryUnit`] for file `i` from a grouping and the parsed programs.
pub fn library_unit<'a>(
    grouping: &LibraryGrouping,
    programs: &[&'a Program],
    i: usize,
) -> LibraryUnit<'a> {
    let siblings = grouping.siblings(i).iter().map(|&j| programs[j]).collect();
    LibraryUnit {
        siblings,
        has_unresolved_parts: grouping.is_unresolved(i),
    }
}

/// Join dotted identifier segments into `a.b.c`.
fn dotted(segments: &[falcon_syntax::ast::Identifier]) -> String {
    segments
        .iter()
        .map(|id| id.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

/// Resolve a *relative* directive URI against a directory. `dart:` and
/// `package:` URIs (which need SDK/pubspec context this layer intentionally
/// lacks) resolve to `None`, conservatively marking the link unresolved.
fn resolve_relative(dir: &Path, uri: &str) -> Option<PathBuf> {
    if uri.starts_with("dart:") || uri.starts_with("package:") {
        return None;
    }
    Some(canonical_or_lexical(&dir.join(uri)))
}

/// Canonical path when it exists (so two spellings compare equal), else a
/// lexical normalization that resolves `.`/`..` without touching the FS.
fn canonical_or_lexical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| lexical_normalize(path))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            // Only cancel a preceding *normal* component; at the root `..` has no
            // effect, and an unmatched `..` (nothing to cancel) is preserved.
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir) => {}
                _ => out.push(comp.as_os_str()),
            },
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A minimal disjoint-set for merging library members.
struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, x: usize) -> usize {
        let mut root = x;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        // Path compression.
        let mut cur = x;
        while self.parent[cur] != root {
            let next = self.parent[cur];
            self.parent[cur] = root;
            cur = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    fn prog(src: &str) -> Program {
        let (p, errors) = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        p
    }

    #[test]
    fn standalone_files_are_single_file_libraries() {
        let a = prog("class A {}");
        let b = prog("class B {}");
        let files = vec![
            (PathBuf::from("/proj/a.dart"), &a),
            (PathBuf::from("/proj/b.dart"), &b),
        ];
        let g = group_libraries(&files);
        assert!(g.siblings(0).is_empty());
        assert!(g.siblings(1).is_empty());
        assert!(!g.is_unresolved(0));
        assert!(!g.is_unresolved(1));
    }

    #[test]
    fn owner_and_part_by_relative_uri_group_together() {
        let owner = prog("part 'part.dart'; class Owner {}");
        let part = prog("part of 'owner.dart'; class Extra {}");
        let files = vec![
            (PathBuf::from("/proj/owner.dart"), &owner),
            (PathBuf::from("/proj/part.dart"), &part),
        ];
        let g = group_libraries(&files);
        assert_eq!(g.siblings(0), &[1]);
        assert_eq!(g.siblings(1), &[0]);
        assert!(!g.is_unresolved(0));
        assert!(!g.is_unresolved(1));
    }

    #[test]
    fn part_of_by_library_name_matches_owner() {
        let owner = prog("library my.lib; part 'p.dart';");
        let part = prog("part of my.lib; class Extra {}");
        let files = vec![
            (PathBuf::from("/proj/owner.dart"), &owner),
            (PathBuf::from("/proj/p.dart"), &part),
        ];
        let g = group_libraries(&files);
        assert_eq!(g.siblings(0), &[1]);
        assert_eq!(g.siblings(1), &[0]);
        assert!(!g.is_unresolved(0));
    }

    #[test]
    fn ambiguous_library_name_leaves_part_of_unresolved() {
        // Two files declare the same library name, so a `part of <name>;` cannot
        // pick a single owner — the link stays unresolved and no group forms.
        let owner_a = prog("library my.lib; class A {}");
        let owner_b = prog("library my.lib; class B {}");
        let part = prog("part of my.lib; class Extra {}");
        let files = vec![
            (PathBuf::from("/proj/a.dart"), &owner_a),
            (PathBuf::from("/proj/b.dart"), &owner_b),
            (PathBuf::from("/proj/p.dart"), &part),
        ];
        let g = group_libraries(&files);
        assert!(g.siblings(2).is_empty());
        assert!(g.is_unresolved(2));
    }

    #[test]
    fn unmatched_parent_dir_is_preserved_in_lexical_normalize() {
        assert_eq!(
            lexical_normalize(Path::new("src/../../outside.dart")),
            PathBuf::from("../outside.dart")
        );
    }

    #[test]
    fn unresolved_part_uri_flags_the_library() {
        let owner = prog("part 'missing.dart'; class Owner {}");
        let files = vec![(PathBuf::from("/proj/owner.dart"), &owner)];
        let g = group_libraries(&files);
        assert!(g.is_unresolved(0));
        assert!(g.siblings(0).is_empty());
    }

    #[test]
    fn unresolved_part_of_name_flags_the_part() {
        let part = prog("part of nonexistent.lib; class Extra {}");
        let files = vec![(PathBuf::from("/proj/p.dart"), &part)];
        let g = group_libraries(&files);
        assert!(g.is_unresolved(0));
    }

    #[test]
    fn unresolved_flag_propagates_to_whole_group() {
        // The owner stitches one resolvable part and one missing part. The
        // missing part flags the owner, and the flag propagates to every member
        // of the merged group (the resolvable part inherits it).
        let owner = prog("part 'p.dart'; part 'missing.dart'; class Owner {}");
        let part = prog("part of 'owner.dart'; class Extra {}");
        let files = vec![
            (PathBuf::from("/proj/owner.dart"), &owner),
            (PathBuf::from("/proj/p.dart"), &part),
        ];
        let g = group_libraries(&files);
        assert_eq!(g.siblings(0), &[1]);
        assert!(g.is_unresolved(0));
        assert!(g.is_unresolved(1));
    }

    #[test]
    fn package_uri_part_is_unresolved() {
        let owner = prog("part 'package:foo/bar.dart'; class Owner {}");
        let files = vec![(PathBuf::from("/proj/owner.dart"), &owner)];
        let g = group_libraries(&files);
        assert!(g.is_unresolved(0));
    }

    #[test]
    fn library_unit_maps_siblings_to_programs() {
        let owner = prog("part 'part.dart'; class Owner {}");
        let part = prog("part of 'owner.dart'; class Extra {}");
        let files = vec![
            (PathBuf::from("/proj/owner.dart"), &owner),
            (PathBuf::from("/proj/part.dart"), &part),
        ];
        let g = group_libraries(&files);
        let programs = vec![&owner, &part];
        let unit = library_unit(&g, &programs, 0);
        assert_eq!(unit.siblings().len(), 1);
        assert!(!unit.has_unresolved_parts());
    }
}
