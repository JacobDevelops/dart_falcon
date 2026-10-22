//! Detect likely copy-paste errors in trivial accessors when a different field
//! matching the accessor name is declared. Inherited contracts and computed
//! accessors are conservatively excluded.
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct MatchGetterSetterFieldNames;

impl Rule for MatchGetterSetterFieldNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            fields: HashSet::new(),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    fields: HashSet<&'a str>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        self.fields.clear();
        let standalone = match decl {
            TopLevelDecl::Class(class) => class.extends().is_none() && class.with_clause().is_empty() && class.implements().is_empty(),
            TopLevelDecl::MixinClass(class) => class.extends().is_none() && class.with_clause().is_empty() && class.implements().is_empty(),
            _ => false,
        };
        if !standalone { return StructuralTraversal::SkipChildren; }
        if let Some(members) = crate::lint::declarations::members_of(decl) {
            for member in members {
                if let ClassMember::Field(field) = member {
                    self.fields.extend(field.declarators().map(|declaration| declaration.name().name()));
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let accessor = match member {
            ClassMember::Getter(getter) if getter.annotations().is_empty() => {
                getter.body().and_then(|body| direct_expression(body, true))
                    .and_then(field_name).map(|field| (getter.name(), field))
            }
            ClassMember::Setter(setter) if setter.annotations().is_empty() => {
                setter.body().and_then(|body| direct_expression(body, false)).and_then(|expression| {
                    if let Expr::Assign(assign) = expression.unparenthesized()
                        && assign.op() == AssignOp::Eq
                        && let Expr::Ident(value) = assign.value().unparenthesized()
                        && value.name() == setter.param().name()
                        && let Some(field) = field_name(assign.target())
                        && field != setter.param().name()
                    {
                        Some((setter.name(), field))
                    } else { None }
                })
            }
            _ => None,
        };
        if let Some((name, field)) = accessor
            && let Some(backing_name) = field.strip_prefix('_')
            && self.fields.contains(field)
            && name.name().strip_prefix('_').unwrap_or(name.name()) != backing_name
            && self.fields.iter().any(|candidate| {
                *candidate != field && candidate.strip_prefix('_').unwrap_or(candidate) == name.name().strip_prefix('_').unwrap_or(name.name())
            })
        {
            self.diags.push(self.ctx.diagnostic(
                format!("Accessor `{}` uses `{field}` instead of the field matching its name.", name.name()),
                name.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
}
}

fn direct_expression(body: FunctionBody<'_>, getter: bool) -> Option<Expr<'_>> {
    match body {
        FunctionBody::Arrow(arrow) => Some(arrow.expr()),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            let statement = statements.next()?;
            if statements.next().is_some() {
                return None;
            }
            match statement {
                Stmt::Return(ret) if getter => ret.value(),
                Stmt::Expr(expr) if !getter => Some(expr.expr()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn field_name(expression: Expr<'_>) -> Option<&str> {
    match expression.unparenthesized() {
        Expr::Ident(identifier) => Some(identifier.name()),
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
            Some(field.field().name())
        }
        _ => None,
    }
}
