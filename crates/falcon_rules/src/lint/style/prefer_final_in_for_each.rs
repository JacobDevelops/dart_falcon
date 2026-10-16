//! Prefer `final` for a for-each loop variable that is never reassigned.
//!
//! `for (var item in items)` where `item` is never written in its scope reads
//! better as `for (final item in items)`. Destructuring loops are covered when
//! the top-level record, object, list, or map pattern binds only plain
//! variables and none of them is reassigned. A loop over an existing variable
//! (`for (item in items)`) declares nothing and is ignored. Collection loops
//! outside function bodies (top-level and field initializers) are also ignored.
//!
//! ```dart
//! for (var name in names) print(name); // flagged
//! for (final name in names) print(name); // ok
//! ```
//!
//! Adopted from the official `prefer_final_in_for_each` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, for_each_declared_body, walk_expr, walk_stmt};

use crate::lint::lexical_usage::collection_for_inits;

const RULE: &str = "prefer-final-in-for-each";

#[derive(Default)]
pub struct PreferFinalInForEach;

impl Rule for PreferFinalInForEach {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mutated = ctx.mutated_declarations(program);
        let mut bodies = Vec::new();
        for_each_declared_body(program, ctx.source, &mut |body, _, _| {
            bodies.push(*body.span());
        });
        let mut collector = Collector {
            ctx,
            mutated,
            diagnostics: Vec::new(),
            bodies,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diagnostics)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    mutated: &'a HashSet<usize>,
    diagnostics: Vec<Diagnostic>,
    bodies: Vec<Span>,
}

impl Collector<'_, '_> {
    fn check(&mut self, init: &ForInit) {
        let span = match init {
            ForInit::ForIn { name, .. } => name.span,
            ForInit::PatternForIn { pattern, .. } => *pattern.span(),
            ForInit::VarDecl(_) | ForInit::Exprs(_) => return,
        };
        if !self
            .bodies
            .iter()
            .any(|body| body.start <= span.start && span.end <= body.end)
        {
            return;
        }
        match init {
            ForInit::ForIn {
                keyword: VarKeyword::Var,
                declares_variable: true,
                name,
                ..
            } if !self.mutated.contains(&name.span.start) => {
                self.diagnostics.push(self.ctx.diagnostic(
                    RULE,
                    format!("The variable '{}' should be final.", name.name),
                    name.span,
                ));
            }
            ForInit::PatternForIn {
                keyword: VarKeyword::Var,
                pattern,
                ..
            } => {
                let Some(variables) = top_level_variables(pattern) else {
                    return;
                };
                if variables
                    .iter()
                    .any(|name| self.mutated.contains(&name.span.start))
                {
                    return;
                }
                self.diagnostics.push(self.ctx.diagnostic(
                    RULE,
                    "The pattern should be final.",
                    *pattern.span(),
                ));
            }
            _ => {}
        }
    }
}

/// The variables of a top-level record/object/list/map pattern whose every
/// field or element is a plain variable pattern; `None` when any is not
/// (the analyzer then treats the pattern as potentially mutated).
fn top_level_variables(pattern: &Pattern) -> Option<Vec<&Identifier>> {
    fn variable(pattern: &Pattern) -> Option<&Identifier> {
        match pattern {
            Pattern::Variable { name, .. } => Some(name),
            _ => None,
        }
    }
    match pattern {
        Pattern::Record(record) => record
            .fields
            .iter()
            .map(|field| variable(&field.pattern))
            .collect(),
        Pattern::Object(object) => object
            .fields
            .iter()
            .map(|field| field.pattern.as_ref().and_then(variable))
            .collect(),
        Pattern::List(list) => list
            .elements
            .iter()
            .map(|element| match element {
                ListPatternElement::Pattern(pattern) => variable(pattern),
                ListPatternElement::Rest(..) => None,
            })
            .collect(),
        Pattern::Map(map) if !map.has_rest => map
            .entries
            .iter()
            .map(|entry| variable(&entry.pattern))
            .collect(),
        _ => None,
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::For(statement) = node
            && let Some(init) = &statement.header.init
        {
            self.check(init);
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { body, .. } = node {
            self.bodies.push(*body.span());
        }
        // Collection `for` elements are for-each loops too.
        for init in collection_for_inits(node) {
            self.check(init);
        }
        walk_expr(self, node);
    }
}
