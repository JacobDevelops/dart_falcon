//! Flags a private function typedef that is used at most once.
//!
//! A private typedef for a function type only saves typing when it is reused.
//! Used once (or never), an inline `Function(...)` type is clearer:
//!
//! ```dart
//! typedef _Callback = void Function(int);   // flagged: used once
//! void register(_Callback cb) {}
//!
//! typedef void _Legacy(int x);              // flagged: never used
//! ```
//!
//! Generic `typedef _F<T> = ...` aliases and aliases of non-function types are
//! not reported; legacy `typedef void _F(...)` forms always are. Uses are
//! counted as type annotations across every file of the library. When the
//! library has `part` files that falcon cannot see, the rule stays silent.
//!
//! Adopted from the official `avoid_private_typedef_functions` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_dart_type};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidPrivateTypedefFunctions;

impl Rule for AvoidPrivateTypedefFunctions {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut candidates: Vec<(Identifier<'a>, usize)> = program
            .declarations()
            .filter_map(|declaration| match declaration {
                TopLevelDecl::TypeAlias(alias) if is_candidate(alias) => Some((alias.name(), 0)),
                _ => None,
            })
            .collect();
        if candidates.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }

        let has_parts =
            !program.part_directives().is_empty() || program.part_of_directive().is_some();
        let siblings: &[Program<'a>] = match ctx.library() {
            Some(library) if !library.has_unresolved_parts() => library.siblings(),
            _ if has_parts => return RulePreparation::Diagnostics(Vec::new()),
            _ => &[],
        };

        let mut counter = UseCounter {
            candidates: &mut candidates,
        };
        for sibling in siblings {
            counter.visit_program(*sibling);
        }
        RulePreparation::Structural(Box::new(Collector { ctx, candidates }))
    }
}

/// A private typedef the analyzer considers: every legacy function typedef,
/// and non-generic `typedef _F = <function type>;` aliases.
fn is_candidate(alias: TypeAliasDecl<'_>) -> bool {
    if !alias.name().name().starts_with('_') {
        return false;
    }
    let DartType::Function(function) = alias.aliased() else {
        return false;
    };
    let legacy = function.span().start <= alias.name().span().start;
    legacy || alias.type_params().is_empty()
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    candidates: Vec<(Identifier<'a>, usize)>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        UseCounter::count_type(&mut self.candidates, node);
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates.into_iter().filter(|(_, count)| *count <= 1).map(|(name, _)| {
            self.ctx.diagnostic(
                "avoid-private-typedef-functions",
                "The typedef is unnecessary because it is only used in one place.",
                name.span(),
            )
        }).collect()
    }
}
}

/// Counts type annotations naming the candidate typedefs.
struct UseCounter<'a, 'p> {
    candidates: &'a mut [(Identifier<'p>, usize)],
}

impl UseCounter<'_, '_> {
    fn count_type(candidates: &mut [(Identifier<'_>, usize)], node: DartType<'_>) {
        if let DartType::Named(named) = node
            && let Some(segment) = named.segments().next_back()
        {
            for (name, count) in candidates {
                if segment.name() == name.name() {
                    *count += 1;
                }
            }
        }
    }
}

impl<'a> Visitor<'a> for UseCounter<'_, '_> {
    fn visit_dart_type(&mut self, node: DartType<'a>) {
        Self::count_type(self.candidates, node);
        walk_dart_type(self, node);
    }
}
