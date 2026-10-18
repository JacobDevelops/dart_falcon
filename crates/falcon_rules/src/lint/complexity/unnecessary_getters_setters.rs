//! Flags a trivial getter/setter pair that only reads and writes a private field.
//!
//! When a getter merely returns `_x` and a setter merely assigns `_x` with no
//! validation or side effect, the pair buys nothing over a public field — Dart
//! lets you later replace a public field with a getter/setter without changing
//! call sites, so the wrapping is premature. The rule fires only when a getter
//! and setter share a name, both target the same backing field, and that field
//! is private (has a leading underscore). Metadata on either accessor exempts
//! the pair.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct UnnecessaryGettersSetters;

impl Rule for UnnecessaryGettersSetters {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if let Some(members) = crate::lint::declarations::members_of(decl) {
            check_members(members, self.ctx, &mut self.diags);
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn check_members<'a>(
    members: impl Iterator<Item = ClassMember<'a>>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let mut getters: HashMap<&str, GetterDecl<'a>> = HashMap::new();
    let mut setters: HashMap<&str, SetterDecl<'a>> = HashMap::new();
    for member in members {
        match member {
            ClassMember::Getter(g) => {
                getters.insert(g.name().name(), g);
            }
            ClassMember::Setter(s) => {
                setters.insert(s.name().name(), s);
            }
            _ => {}
        }
    }

    for (name, getter) in &getters {
        let Some(setter) = setters.get(name) else {
            continue;
        };
        if !getter.annotations().is_empty() || !setter.annotations().is_empty() {
            continue;
        }
        let (Some(gf), Some(sf)) = (getter_returns_field(*getter), setter_assigns_field(*setter))
        else {
            continue;
        };
        // Both accessors must wrap the same private backing field.
        if gf == sf && gf.starts_with('_') {
            diags.push(
                ctx.diagnostic(
                    "unnecessary-getters-setters",
                    "Avoid wrapping a field in trivial getters and setters; use a public field."
                        .to_string(),
                    getter.name().span(),
                ),
            );
        }
    }
}

fn field_name(expr: Expr<'_>) -> Option<&str> {
    match expr.unparenthesized() {
        Expr::Ident(id) => Some(id.name()),
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
            Some(field.field().name())
        }
        _ => None,
    }
}

pub(crate) fn getter_returns_field(getter: GetterDecl<'_>) -> Option<&str> {
    match getter.body()? {
        FunctionBody::Arrow(body) => field_name(body.expr()),
        FunctionBody::Block(block) => {
            let mut stmts = block.stmts();
            let stmt = stmts.next()?;
            if stmts.next().is_some() {
                return None;
            }
            match stmt {
                Stmt::Return(ret) => field_name(ret.value()?),
                _ => None,
            }
        }
        _ => None,
    }
}

fn setter_assigns_field(setter: SetterDecl<'_>) -> Option<&str> {
    let assign = match setter.body()? {
        FunctionBody::Arrow(body) => body.expr(),
        FunctionBody::Block(block) => {
            let mut stmts = block.stmts();
            let stmt = stmts.next()?;
            if stmts.next().is_some() {
                return None;
            }
            match stmt {
                Stmt::Expr(e) => e.expr(),
                _ => return None,
            }
        }
        _ => return None,
    };
    if let Expr::Assign(assign) = assign.unparenthesized()
        && assign.op() == AssignOp::Eq
        && let Expr::Ident(rhs) = assign.value().unparenthesized()
        && rhs.name() == setter.param().name()
    {
        return field_name(assign.target());
    }
    None
}
