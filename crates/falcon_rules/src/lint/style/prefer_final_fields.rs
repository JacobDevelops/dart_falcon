//! Flags a private field that is initialized but never reassigned, so it could
//! be `final` (`prefer-final-fields`, adopted from package:lints).
//!
//! Marking such fields `final` documents that they never change after
//! construction and lets the compiler enforce it, preventing accidental
//! mutation. The analysis is deliberately conservative to avoid false positives:
//! a whole-*library* scan first records every name that is ever assigned,
//! compound assigned, or incremented, and any such write disqualifies that name
//! (matched by identifier across the library, so a like-named field in another
//! class also protects it). A candidate is reported only when it is provably
//! initialized through the channels recognized by the analyzer: every declared
//! constructor setting it via `: _x = ...` or a `this._x` formal, or a declaration
//! initializer when no constructor sets it. Enum fields are exempt. Only
//! private (underscore-prefixed) fields are considered, and those already
//! `final`, `const`, `late`, `external`, or `abstract` are skipped.
//!
//! Library awareness (suppress-only): a private field can be written from a
//! sibling part file, so write collection unions this file with every sibling in
//! its [`AnalyzeContext::library`]. When the library has an unresolved part, or
//! this file declares `part`/`part of` directives but no library context is
//! available, the full set of writes cannot be seen — every private-field
//! diagnostic for the file is suppressed rather than risk a false positive. A
//! standalone file with no part directives keeps the exact single-file behavior.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{self as visitor, Visitor};
use falcon_syntax::cst::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct PreferFinalFields;

impl Rule for PreferFinalFields {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        // Suppress-only: if this file participates in a library whose writes we
        // cannot fully see, stay silent. That is an unresolved part in the known
        // library, or part/part-of directives with no library context at all.
        let has_part_directives =
            !program.part_directives().is_empty() || program.part_of_directive().is_some();
        match ctx.library {
            Some(lib) if lib.has_unresolved_parts() => {
                return RulePreparation::Diagnostics(Vec::new());
            }
            None if has_part_directives => return RulePreparation::Diagnostics(Vec::new()),
            _ => {}
        }

        // Name-keyed write union over the whole library (owner + parts), so a
        // field written only from a sibling part is not falsely flagged.
        let mut written = Written {
            names: HashSet::new(),
        };
        if let Some(lib) = ctx.library {
            for sibling in lib.siblings() {
                written.visit_program(*sibling);
            }
        }

        RulePreparation::Structural(Box::new(Collector {
            ctx,
            written,
            candidates: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    written: Written<'a>,
    candidates: Vec<(&'a str, Span)>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if !matches!(decl, TopLevelDecl::Enum(_))
            && let Some(members) = crate::lint::declarations::members_of(decl)
        {
            // Initialization eligibility is local; write eligibility is known
            // only after every expression in this file has been observed.
            check_class(members, self.ctx.source, &mut self.candidates);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.written.record_expr(node);
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates
            .into_iter()
            .filter(|(name, _)| !self.written.names.contains(*name))
            .map(|(name, span)| {
                self.ctx.diagnostic(
                    "prefer-final-fields",
                    format!("The private field '{name}' could be 'final'."),
                    span,
                )
            })
            .collect()
    }
}
}

fn check_class<'a>(
    members: AstChildren<'a, ClassMember<'a>>,
    source: &'a str,
    candidates: &mut Vec<(&'a str, Span)>,
) {
    // The analyzer requires every declared constructor to initialize a field
    // when any constructor does, including redirecting and factory constructors.
    let constructors: Vec<ConstructorDecl<'a>> = members
        .clone()
        .filter_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .collect();

    for member in members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        if field.keyword() != VarKeyword::Var
            || field.is_late()
            || field.is_external()
            || field.is_abstract()
        {
            continue;
        }
        for d in field.declarators() {
            let name = d.name().name();
            if !name.starts_with('_') {
                continue;
            }
            let has_decl_init = d.initializer().is_some();
            let touched_by_ctor = constructors.iter().any(|c| inits_field(*c, name));

            let ok = if touched_by_ctor {
                constructors.iter().all(|c| inits_field(*c, name))
            } else {
                has_decl_init
            };

            if ok {
                let span = d.name().span();
                candidates.push((&source[span.start..span.end], span));
            }
        }
    }
}

fn inits_field(c: ConstructorDecl<'_>, name: &str) -> bool {
    let via_list = c.initializers().any(
        |i| matches!(i, ConstructorInitializer::FieldInit(field) if field.field().name() == name),
    );
    let via_formal = c
        .params()
        .params()
        .any(|p| p.initializing() == Some(InitializingFormal::This) && p.name().name() == name);
    via_list || via_formal
}

// Collects every field/variable name that is written (assigned, compound
// assigned, or incremented) in each visited program; the caller unions one or
// more programs (a file plus its library siblings) into one name set.
struct Written<'a> {
    names: HashSet<&'a str>,
}

impl<'a> Written<'a> {
    fn record_target(&mut self, target: Expr<'a>) {
        match target.unparenthesized() {
            Expr::Ident(id) => {
                self.names.insert(id.name());
            }
            Expr::Field(field) => {
                self.names.insert(field.field().name());
            }
            _ => {}
        }
    }
}

impl<'a> Written<'a> {
    fn record_expr(&mut self, node: Expr<'a>) {
        match node {
            Expr::Assign(assignment) => self.record_target(assignment.target()),
            Expr::PostfixIncDec(postfix) => self.record_target(postfix.operand()),
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                self.record_target(unary.operand());
            }
            Expr::Cascade(cascade) => {
                for s in cascade.sections() {
                    for op in s.ops() {
                        if let CascadeOp::Assign(assignment) = op {
                            self.record_target(assignment.target());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl<'a> Visitor<'a> for Written<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        self.record_expr(node);
        visitor::walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_analyze::{group_libraries, library_unit};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Run the rule over file `target`, building the real library grouping so
    /// `ctx.library` mirrors production (owner + part siblings, unresolved flag).
    fn run_in_library(files: &[(&str, &str)], target: usize) -> usize {
        let parsed: Vec<_> = files
            .iter()
            .map(|(_, source)| {
                std::sync::Arc::new(falcon_analyze::ParsedFile {
                    source: (*source).into(),
                    tree: parse(source).tree,
                })
            })
            .collect();
        let programs: Vec<Program<'_>> = parsed.iter().map(|file| file.program()).collect();
        let path_prog: Vec<(PathBuf, Program<'_>)> = files
            .iter()
            .zip(&programs)
            .map(|((p, _), prog)| (PathBuf::from(p), *prog))
            .collect();
        let grouping = group_libraries(&path_prog);
        let parsed_refs: Vec<_> = parsed.iter().collect();
        let unit = library_unit(&grouping, &parsed_refs, target);
        let config = FalconConfig::default();
        let path = PathBuf::from(files[target].0);
        let ctx = AnalyzeContext::new(&path, &config, &parsed[target]).with_library(&unit);
        PreferFinalFields.analyze(programs[target], &ctx).len()
    }

    /// Run with no library context at all (standalone / degraded single-file).
    fn run_standalone(source: &str) -> usize {
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parse(source).tree,
        });
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        PreferFinalFields.analyze(parsed.program(), &ctx).len()
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
