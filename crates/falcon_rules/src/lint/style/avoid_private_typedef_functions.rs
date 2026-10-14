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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_dart_type};

#[derive(Default)]
pub struct AvoidPrivateTypedefFunctions;

impl Rule for AvoidPrivateTypedefFunctions {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let candidates: Vec<&Identifier> = program
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                TopLevelDecl::TypeAlias(alias) if is_candidate(alias) => Some(&alias.name),
                _ => None,
            })
            .collect();
        if candidates.is_empty() {
            return Vec::new();
        }

        let has_parts = !program.part_directives.is_empty() || program.part_of_directive.is_some();
        let siblings: &[&Program] = match ctx.library {
            Some(library) if !library.has_unresolved_parts() => library.siblings(),
            _ if has_parts => return Vec::new(),
            _ => &[],
        };

        candidates
            .into_iter()
            .filter(|name| {
                let mut counter = UseCounter {
                    name: &name.name,
                    count: 0,
                };
                counter.visit_program(program);
                for sibling in siblings {
                    counter.visit_program(sibling);
                }
                counter.count <= 1
            })
            .map(|name| {
                ctx.diagnostic(
                    "avoid-private-typedef-functions",
                    "The typedef is unnecessary because it is only used in one place.",
                    name.span,
                )
            })
            .collect()
    }
}

/// A private typedef the analyzer considers: every legacy function typedef,
/// and non-generic `typedef _F = <function type>;` aliases.
fn is_candidate(alias: &TypeAliasDecl) -> bool {
    if !alias.name.name.starts_with('_') {
        return false;
    }
    let DartType::Function(function) = &alias.aliased else {
        return false;
    };
    let legacy = function.span.start <= alias.name.span.start;
    legacy || alias.type_params.is_empty()
}

/// Counts type annotations naming the typedef.
struct UseCounter<'a> {
    name: &'a str,
    count: usize,
}

impl Visitor for UseCounter<'_> {
    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Named(named) = node
            && named
                .segments
                .last()
                .is_some_and(|segment| segment.name == self.name)
        {
            self.count += 1;
        }
        walk_dart_type(self, node);
    }
}
