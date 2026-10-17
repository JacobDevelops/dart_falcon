//! Flags positional fields in record types.
//!
//! Positional record fields are accessed as `$1`, `$2`, and so on, which convey
//! nothing about what each field means and break silently when fields are
//! reordered. Named fields (`({int x, int y})`) document intent at both the
//! declaration and every use site. The check recurses through type arguments
//! and nested record and function types, so a positional field buried inside a
//! generic is still reported.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidPositionalFieldsInRecords;

impl Rule for AvoidPositionalFieldsInRecords {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let visitor = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(visitor))
    }
}

const MESSAGE: &str = "Avoid positional fields in records. Use named fields instead.";

fn record_literal_has_positional(expr: &Expr) -> bool {
    matches!(expr.unparenthesized(), Expr::Record { fields, .. } if fields.iter().any(|f| f.name.is_none()))
}

// Explicit types are checked separately; do not also report their initializer.
fn check_declarators(
    declarators: &[VarDeclarator],
    has_type: bool,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    for d in declarators {
        if let Some(init) = &d.initializer
            && !has_type
            && record_literal_has_positional(init)
        {
            diags.push(ctx.diagnostic(
                "avoid-positional-fields-in-records",
                MESSAGE,
                *(init.span()),
            ));
        }
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_dart_type(&mut self, ty: &DartType) -> StructuralTraversal {
        if let DartType::Record(rec) = ty
            && !rec.positional.is_empty()
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-positional-fields-in-records",
                MESSAGE,
                rec.span,
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) -> StructuralTraversal {
        if let TopLevelDecl::Variable(v) = decl {
            check_declarators(
                &v.declarators,
                v.var_type.is_some(),
                &mut self.diags,
                self.ctx,
            );
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, field: &FieldDecl) -> StructuralTraversal {
        check_declarators(
            &field.declarators,
            field.field_type.is_some(),
            &mut self.diags,
            self.ctx,
        );
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, local: &LocalVarDecl) -> StructuralTraversal {
        check_declarators(
            &local.declarators,
            local.var_type.is_some(),
            &mut self.diags,
            self.ctx,
        );
        StructuralTraversal::Continue
    }
}
}
