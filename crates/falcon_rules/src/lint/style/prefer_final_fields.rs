//! Flags a private field that is only ever initialized (at its declaration or
//! in constructors) and never reassigned, so it could be `final`
//! (`prefer-final-fields`, adopted from package:lints).
//!
//! Conservative. A whole-*library* scan collects every field name that is the
//! target of an assignment, compound assignment, or increment/decrement
//! expression — any such write disqualifies the name (matched by name across the
//! library, so a shared name in another class also protects it). A candidate is
//! flagged only when it is provably initialized through exactly one safe channel:
//!   * declaration initializer, and never touched by a constructor; or
//!   * no declaration initializer, but every non-redirecting generative
//!     constructor initializes it (via `: _x = ...` or a `this._x` formal).
//!
//! Fields marked `late`, `external`, or `abstract` are skipped as ambiguous.
//!
//! Library awareness (suppress-only): a private field can be written from a
//! sibling part file, so write collection unions this file with every sibling in
//! its [`AnalyzeContext::library`]. When the library has an unresolved part, or
//! this file declares `part`/`part of` directives but no library context is
//! available, the full set of writes cannot be seen — every private-field
//! diagnostic for the file is suppressed rather than risk a false positive. A
//! standalone file with no part directives keeps the exact single-file behavior.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};
use std::collections::HashSet;

pub struct PreferFinalFields;

impl Rule for PreferFinalFields {
    fn name(&self) -> &'static str {
        "prefer-final-fields"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Suppress-only: if this file participates in a library whose writes we
        // cannot fully see, stay silent. That is an unresolved part in the known
        // library, or part/part-of directives with no library context at all.
        let has_part_directives =
            !program.part_directives.is_empty() || program.part_of_directive.is_some();
        match ctx.library {
            Some(lib) if lib.has_unresolved_parts() => return Vec::new(),
            None if has_part_directives => return Vec::new(),
            _ => {}
        }

        // Name-keyed write union over the whole library (owner + parts), so a
        // field written only from a sibling part is not falsely flagged.
        let mut written = Written {
            names: HashSet::new(),
        };
        written.visit_program(program);
        if let Some(lib) = ctx.library {
            for sibling in lib.siblings() {
                written.visit_program(sibling);
            }
        }

        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = members_of(decl) {
                check_class(members, &written.names, ctx, &mut diags);
            }
        }
        diags
    }
}

fn members_of(decl: &TopLevelDecl) -> Option<&[ClassMember]> {
    match decl {
        TopLevelDecl::Class(c) => Some(&c.members),
        TopLevelDecl::Mixin(m) => Some(&m.members),
        TopLevelDecl::MixinClass(mc) => Some(&mc.members),
        TopLevelDecl::Enum(e) => Some(&e.members),
        TopLevelDecl::ExtensionType(e) => Some(&e.members),
        _ => None,
    }
}

fn check_class(
    members: &[ClassMember],
    written: &HashSet<String>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    // Non-redirecting generative constructors: the ones a final field must be
    // initialized by if it has no declaration initializer.
    let generative: Vec<&ConstructorDecl> = members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .filter(|c| !c.is_factory && !is_redirecting(c))
        .collect();

    for member in members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        if field.is_final
            || field.is_const
            || field.is_late
            || field.is_external
            || field.is_abstract
        {
            continue;
        }
        for d in &field.declarators {
            let name = d.name.name.as_str();
            if !name.starts_with('_') || written.contains(name) {
                continue;
            }
            let has_decl_init = d.initializer.is_some();
            let touched_by_ctor = generative.iter().any(|c| inits_field(c, name))
                || members.iter().any(|m| ctor_inits_any(m, name));

            let ok = if has_decl_init {
                !touched_by_ctor
            } else {
                !generative.is_empty() && generative.iter().all(|c| inits_field(c, name))
            };

            if ok {
                diags.push(Diagnostic::new(
                    "prefer-final-fields",
                    Severity::Warning,
                    format!("The private field '{name}' could be 'final'."),
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: d.name.span.start,
                        end: d.name.span.end,
                    },
                ));
            }
        }
    }
}

fn is_redirecting(c: &ConstructorDecl) -> bool {
    c.initializers
        .iter()
        .any(|i| matches!(i, ConstructorInitializer::ThisCall { .. }))
}

fn ctor_inits_any(member: &ClassMember, name: &str) -> bool {
    matches!(member, ClassMember::Constructor(c) if inits_field(c, name))
}

fn inits_field(c: &ConstructorDecl, name: &str) -> bool {
    let via_list = c.initializers.iter().any(
        |i| matches!(i, ConstructorInitializer::FieldInit { field, .. } if field.name == name),
    );
    let via_formal = c
        .params
        .positional
        .iter()
        .chain(&c.params.optional_positional)
        .chain(&c.params.named)
        .any(|p| p.is_field && p.name.name == name);
    via_list || via_formal
}

// Collects every field/variable name that is written (assigned, compound
// assigned, or incremented) in each visited program; the caller unions one or
// more programs (a file plus its library siblings) into one name set.
struct Written {
    names: HashSet<String>,
}

impl Written {
    fn record_target(&mut self, target: &Expr) {
        match target {
            Expr::Ident(id) => {
                self.names.insert(id.name.clone());
            }
            Expr::Field { field, .. } => {
                self.names.insert(field.name.clone());
            }
            _ => {}
        }
    }
}

impl Visitor for Written {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Assign { target, .. } => self.record_target(target),
            Expr::PostfixIncDec { operand, .. } => self.record_target(operand),
            Expr::Unary {
                op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                operand,
                ..
            } => self.record_target(operand),
            Expr::Cascade { sections, .. } => {
                for s in sections {
                    for op in &s.ops {
                        if let CascadeOp::Assign(target, _, _) = op {
                            self.record_target(target);
                        }
                    }
                }
            }
            _ => {}
        }
        visitor::walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{group_libraries, library_unit};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Run the rule over file `target`, building the real library grouping so
    /// `ctx.library` mirrors production (owner + part siblings, unresolved flag).
    fn run_in_library(files: &[(&str, &str)], target: usize) -> usize {
        let programs_owned: Vec<Program> = files.iter().map(|(_, s)| parse(s).0).collect();
        let programs: Vec<&Program> = programs_owned.iter().collect();
        let path_prog: Vec<(PathBuf, &Program)> = files
            .iter()
            .zip(&programs)
            .map(|((p, _), prog)| (PathBuf::from(p), *prog))
            .collect();
        let grouping = group_libraries(&path_prog);
        let unit = library_unit(&grouping, &programs, target);
        let config = FalconConfig::default();
        let path = PathBuf::from(files[target].0);
        let ctx = AnalyzeContext::new(&path, files[target].1, &config).with_library(&unit);
        PreferFinalFields.analyze(programs[target], &ctx).len()
    }

    /// Run with no library context at all (standalone / degraded single-file).
    fn run_standalone(source: &str) -> usize {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        PreferFinalFields.analyze(&program, &ctx).len()
    }

    #[test]
    fn write_from_sibling_part_suppresses_owner_field() {
        // `_count` looks final-able in the owner alone, but the part writes it.
        let owner = "part 'part.dart';\nclass Counter { int _count = 0; int get count => _count; }";
        let part = "part of 'owner.dart';\nvoid reset(Counter c) { c._count = 0; }";
        let files = [("/p/owner.dart", owner), ("/p/part.dart", part)];
        assert_eq!(run_in_library(&files, 0), 0, "part write must suppress");
    }

    #[test]
    fn field_never_written_across_library_still_fires() {
        // The part only reads `_label`; a genuinely-never-written field must
        // still fire — the library union must not over-suppress.
        let owner =
            "part 'part.dart';\nclass Labeled { String _label = 'x'; String get l => _label; }";
        let part = "part of 'owner.dart';\nString describe(Labeled x) => x._label;";
        let files = [("/p/owner.dart", owner), ("/p/part.dart", part)];
        assert_eq!(
            run_in_library(&files, 0),
            1,
            "read-only sibling keeps candidate"
        );
    }

    #[test]
    fn unresolved_part_suppresses_all() {
        // The owner names a part not in the analyzed set — writes may be hidden.
        let owner = "part 'missing.dart';\nclass A { int _a = 1; int get a => _a; }";
        let files = [("/p/owner.dart", owner)];
        assert_eq!(
            run_in_library(&files, 0),
            0,
            "unresolved part must suppress"
        );
    }

    #[test]
    fn part_directive_without_library_context_suppresses() {
        // File declares `part of` but has no library view — cannot see owner
        // writes, so stay silent.
        let src = "part of 'owner.dart';\nclass A { int _a = 1; int get a => _a; }";
        assert_eq!(
            run_standalone(src),
            0,
            "no library + part directive suppresses"
        );
    }

    #[test]
    fn standalone_file_without_parts_is_unchanged() {
        // The single-file baseline: a final-able private field still fires with
        // no library context and no part directives.
        let src = "class A { int _a = 1; int get a => _a; }";
        assert_eq!(run_standalone(src), 1, "single-file baseline preserved");
    }
}
