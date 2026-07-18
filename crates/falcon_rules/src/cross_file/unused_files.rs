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

use super::{canonical_or_lexical, collect_references, detect_package, is_under_lib};

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
            // Only files under lib/ are candidates.
            if !is_under_lib(&canon, pkg.as_ref()) {
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
