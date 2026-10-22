//! Flags repeated literal keys in map literals, including literal spreads and collection `if` entries.
//!
//! A later key replaces the earlier value. Only scalar literal keys with equal
//! token spelling are compared; arbitrary expressions and unknown spreads are
//! left alone. Mutually exclusive `if` branches are checked independently.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{self, Visitor};
use falcon_syntax::cst::*;

use crate::lint::expressions::equivalent;
use crate::lint::support::syntax::scalar_literal;

#[derive(Default)]
pub struct AvoidDuplicateMapKeys;

impl Rule for AvoidDuplicateMapKeys {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            checked: HashSet::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    checked: HashSet<NodeId>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>) -> StructuralTraversal {
        if let Expr::Map(map) = expression {
            MapScan { collector: self, seen: Vec::new() }.visit_expr(Expr::Map(map));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}

struct MapScan<'s, 'a> {
    collector: &'s mut Collector<'a>,
    seen: Vec<Expr<'a>>,
}

impl<'a> Visitor<'a> for MapScan<'_, 'a> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        if let Expr::Map(map) = expression.unparenthesized()
            && self.collector.checked.insert(map.syntax().id())
        {
            visit::walk_expr(self, Expr::Map(map));
        }
    }

    fn visit_if_condition(&mut self, _condition: IfCondition<'a>) {}

    fn visit_map_element(&mut self, element: MapElement<'a>) {
        if !matches!(element, MapElement::For(_)) {
            visit::walk_map_element(self, element);
        }
    }

    fn visit_map_entry(&mut self, entry: MapEntry<'a>) {
        if entry.key_null_aware() || entry.value_null_aware() {
            return;
        }
        let key = entry.key().unparenthesized();
        if !scalar_literal(key) {
            return;
        }
        if self.seen.iter().any(|previous| {
            equivalent(*previous, key, self.collector.ctx) && compatible_branches(*previous, key)
        }) {
            self.collector
                .diagnostics
                .push(self.collector.ctx.diagnostic("Duplicate map key replaces an earlier entry. Remove or change the key.", key.span()));
        }
        self.seen.push(key);
    }
}

fn compatible_branches(left: Expr<'_>, right: Expr<'_>) -> bool {
    left.syntax()
        .ancestors()
        .filter_map(MapIfElement::cast)
        .all(|branch| {
            let then_span = branch.then_entry().span();
            let in_then = |span: Span| then_span.contains(span);
            let in_branch = branch.span().contains(right.span());
            !in_branch || in_then(left.span()) == in_then(right.span())
        })
}

