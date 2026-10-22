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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::lexical_usage::collection_for_inits;


#[derive(Default)]
pub struct PreferFinalInForEach;

impl Rule for PreferFinalInForEach {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mutated = ctx.mutated_declarations(program);
        let collector = Collector {
            ctx,
            mutated,
            diagnostics: Vec::new(),
            bodies: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    mutated: &'a HashSet<NodeId>,
    diagnostics: Vec<Diagnostic>,
    bodies: Vec<Span>,
}

impl Collector<'_, '_> {
    fn check(&mut self, init: ForInit<'_>) {
        let span = match init {
            ForInit::ForIn(for_in) => for_in.name().span(),
            ForInit::PatternForIn(for_in) => for_in.pattern().span(),
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
            ForInit::ForIn(for_in)
                if for_in.keyword() == VarKeyword::Var
                    && for_in.declares_variable()
                    && !self.mutated.contains(&for_in.name().syntax().id()) =>
            {
                self.diagnostics.push(self.ctx.diagnostic(format!("The variable `{}` should be final.", for_in.name().name()), for_in.name().span()));
            }
            ForInit::PatternForIn(for_in) if for_in.keyword() == VarKeyword::Var => {
                let pattern = for_in.pattern();
                let Some(variables) = top_level_variables(pattern) else {
                    return;
                };
                if variables
                    .iter()
                    .any(|name| self.mutated.contains(&name.syntax().id()))
                {
                    return;
                }
                self.diagnostics.push(self.ctx.diagnostic("The pattern should be final.", pattern.span()));
            }
            _ => {}
        }
    }
}

/// The variables of a top-level record/object/list/map pattern whose every
/// field or element is a plain variable pattern; `None` when any is not
/// (the analyzer then treats the pattern as potentially mutated).
fn top_level_variables(pattern: Pattern<'_>) -> Option<Vec<Identifier<'_>>> {
    fn variable(pattern: Pattern<'_>) -> Option<Identifier<'_>> {
        match pattern {
            Pattern::Variable(variable) => Some(variable.name()),
            _ => None,
        }
    }
    match pattern {
        Pattern::Record(record) => record
            .fields()
            .map(|field| variable(field.pattern()))
            .collect(),
        Pattern::Object(object) => object
            .fields()
            .map(|field| field.pattern().and_then(variable))
            .collect(),
        Pattern::List(list) => list
            .elements()
            .map(|element| match element {
                ListPatternElement::Pattern(pattern) => variable(pattern),
                ListPatternElement::Rest(..) => None,
            })
            .collect(),
        Pattern::Map(map) if !map.has_rest() => map
            .entries()
            .map(|entry| variable(entry.pattern()))
            .collect(),
        _ => None,
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Function(function) = node
            && let Some(body) = function.body()
        {
            self.bodies.push(body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        let body = match node {
            ClassMember::Method(node) => node.body(),
            ClassMember::Constructor(node) => node.body(),
            ClassMember::Getter(node) => node.body(),
            ClassMember::Setter(node) => node.body(),
            ClassMember::Operator(node) => node.body(),
            ClassMember::PrimaryConstructorBody(node) => node.body(),
            ClassMember::Field(_) | ClassMember::Error(_) => {
                return StructuralTraversal::Continue;
            }
        };
        if let Some(body) = body {
            self.bodies.push(body.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::For(statement) = node
            && let Some(init) = statement.header().init()
        {
            self.check(init);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            self.bodies.push(function.body().span());
        }
        // Collection `for` elements are for-each loops too.
        for init in collection_for_inits(node) {
            self.check(init);
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
