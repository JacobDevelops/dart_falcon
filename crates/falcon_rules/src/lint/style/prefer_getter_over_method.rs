//! Prefer getters for synchronous, zero-argument methods that only read a field.
//!
//! Only unannotated instance methods on classes without inheritance are checked.
//! The result must be a directly declared, non-late instance field, not another
//! getter or an arbitrary expression. Generic methods and overridden contracts
//! are excluded. Both arrow bodies and single-return blocks are supported.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferGetterOverMethod;

impl Rule for PreferGetterOverMethod {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

fn returned_field(body: FunctionBody<'_>) -> Option<&str> {
    let expression = match body {
        FunctionBody::Arrow(body) => body.expr(),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            let Stmt::Return(statement) = statements.next()? else {
                return None;
            };
            if statements.next().is_some() {
                return None;
            }
            statement.value()?
        }
        FunctionBody::Native(_) => return None,
    };
    match expression.unparenthesized() {
        Expr::Ident(name) => Some(name.name()),
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
            Some(field.field().name())
        }
        _ => None,
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        let TopLevelDecl::Class(class) = node else { return StructuralTraversal::SkipChildren; };
        if class.extends().is_some() || !class.with_clause().is_empty() || !class.implements().is_empty() {
            return StructuralTraversal::SkipChildren;
        }
        for member in class.members() {
            let ClassMember::Method(method) = member else { continue; };
            if method.is_static() || !method.annotations().is_empty() || !method.type_params().is_empty()
                || matches!(method.name().name(), "toString" | "toJson" | "call")
                || !method.params().params().is_empty() || method.async_marker() != AsyncMarker::Sync {
                continue;
            }
            let Some(field_name) = method.body().and_then(returned_field) else { continue; };
            let is_field = class.members().any(|member| matches!(member,
                ClassMember::Field(field) if !field.is_static() && !field.is_late() && !field.is_external()
                    && field.declarators().any(|declarator| declarator.name().name() == field_name)));
            if is_field {
                self.diags.push(self.ctx.diagnostic("This zero-argument method only reads a field; expose it as a getter instead.", method.name().span()));
            }
        }
        StructuralTraversal::SkipChildren
    }
}
}
