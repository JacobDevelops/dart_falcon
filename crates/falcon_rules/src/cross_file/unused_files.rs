//! Report `lib/` files that nothing in the project references.
//!
//! Flags a file under `lib/` that no other file imports, exports, or includes as
//! a `part`, and that is not itself an entrypoint. Such a file is typically dead
//! code left behind after its callers were removed — it bloats the package and
//! misleads readers into thinking it is live. Files that declare a top-level
//! `main`, and `part of` files (which belong to their owning library), are never
//! reported. This is a cross-file rule: it runs in the cross-file pass over the
//! whole analyzed file set and is configured under the top-level `cross-file`
//! section rather than `linter`.

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::{Program, TopLevelDecl};

use std::path::Path;

use super::{
    PackageInfo, canonical_or_lexical, collect_references, detect_package, has_lib_component,
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
        let refs = collect_references(files, pkg.as_ref());

        let mut diags = Vec::new();
        for f in files {
            let canon = canonical_or_lexical(&f.path);
            // Only files under lib/ are candidates. Without a pubspec the rule's
            // documented scope still applies: require a `lib/` path component so
            // test/bin/tool files aren't flagged.
            if !is_under_lib(&canon, pkg.as_ref()) {
                continue;
            }
            if pkg.is_none() && !has_lib_component(&canon) {
                continue;
            }
            // A `part of` file belongs to its owner and is never a standalone unit.
            if f.program.part_of_directive.is_some() {
                continue;
            }
            // Entrypoints (`main`) are referenced by the toolchain, not by imports.
            if has_main(&f.program) {
                continue;
            }
            // The package barrel (`lib/<pkg>.dart`) is the public entry point,
            // imported by external consumers, so it is never referenced from
            // within its own package.
            if is_package_barrel(&canon, pkg.as_ref()) {
                continue;
            }
            // Referenced by some import/export/part directive anywhere → used.
            if refs.contains(&canon) {
                continue;
            }
            diags.push(Diagnostic::new(
                NAME,
                Severity::Warning,
                "File is never referenced by any other file and is not an entrypoint; \
                 it may be dead code",
                f.path.to_string_lossy().into_owned(),
                DiagSpan { start: 0, end: 0 },
            ));
        }
        diags
    }
}

/// Whether the program declares a top-level `main` function.
fn has_main(program: &Program) -> bool {
    program.declarations.iter().any(|d| {
        matches!(d, TopLevelDecl::Function(f) if f.name.name == "main" && !f.is_getter && !f.is_setter)
    })
}

/// Whether `path` is the package's public barrel `lib/<pkg-name>.dart`, i.e.
/// sits directly in `lib/` and its stem matches the pubspec `name`.
fn is_package_barrel(path: &Path, pkg: Option<&PackageInfo>) -> bool {
    let Some(pkg) = pkg else { return false };
    path.parent() == Some(pkg.lib_root.as_path())
        && path.file_stem().and_then(|s| s.to_str()) == Some(pkg.name.as_str())
        && path.extension().and_then(|e| e.to_str()) == Some("dart")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pkg() -> PackageInfo {
        PackageInfo {
            name: "mypkg".to_string(),
            lib_root: PathBuf::from("/proj/lib"),
        }
    }

    #[test]
    fn barrel_is_exempt() {
        assert!(is_package_barrel(
            Path::new("/proj/lib/mypkg.dart"),
            Some(&pkg())
        ));
    }

    #[test]
    fn non_barrel_lib_files_are_not_exempt() {
        assert!(!is_package_barrel(
            Path::new("/proj/lib/src/foo.dart"),
            Some(&pkg())
        ));
        assert!(!is_package_barrel(
            Path::new("/proj/lib/other.dart"),
            Some(&pkg())
        ));
    }

    #[test]
    fn no_package_means_no_barrel() {
        assert!(!is_package_barrel(Path::new("/proj/lib/mypkg.dart"), None));
    }
}
