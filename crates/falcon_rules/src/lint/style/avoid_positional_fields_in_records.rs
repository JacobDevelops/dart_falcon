//! Flags positional fields in record types.
//!
//! Positional record fields are accessed as `$1`, `$2`, and so on, which convey
//! nothing about what each field means and break silently when fields are
//! reordered. Named fields (`({int x, int y})`) document intent at both the
//! declaration and every use site. The check recurses through type arguments
//! and nested record and function types, so a positional field buried inside a
//! generic is still reported.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidPositionalFieldsInRecords;

impl Rule for AvoidPositionalFieldsInRecords {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str = "Avoid positional fields in records. Use named fields instead.";

/// Flag any record *type* that declares positional fields, recursing through
/// type arguments and nested record/function types.
fn check_type(ty: Option<&DartType>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let Some(ty) = ty else { return };
    match ty {
        DartType::Record(rec) => {
            if !rec.positional.is_empty() {
                diags.push(ctx.diagnostic("avoid-positional-fields-in-records", MESSAGE, rec.span));
            }
            for p in &rec.positional {
                check_type(Some(p), diags, ctx);
            }
            for n in &rec.named {
                check_type(Some(&n.field_type), diags, ctx);
            }
        }
        DartType::Named(named) => {
            for arg in &named.type_args {
                check_type(Some(arg), diags, ctx);
            }
        }
        DartType::Function(f) => {
            if let Some(rt) = &f.return_type {
                check_type(Some(rt), diags, ctx);
            }
            for p in &f.params {
                check_type(Some(&p.param_type), diags, ctx);
            }
        }
        _ => {}
    }
}

fn check_params(params: &FormalParamList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for p in params.params.iter() {
        check_type(p.param_type.as_ref(), diags, ctx);
    }
}

/// True when a record literal carries at least one positional (unnamed) field.
fn record_literal_has_positional(expr: &Expr) -> bool {
    matches!(expr, Expr::Record { fields, .. } if fields.iter().any(|f| f.name.is_none()))
}

/// A record literal is flagged only when it is the initializer of a declaration
/// WITHOUT an explicit record type — when a record type is present it is already
/// flagged at the type, so we avoid double-reporting on the same construct.
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

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            check_type(f.return_type.as_ref(), diags, ctx);
            if let Some(params) = f.params() {
                check_params(params, diags, ctx);
            }
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx);
            }
        }
        TopLevelDecl::Variable(v) => {
            check_type(v.var_type.as_ref(), diags, ctx);
            check_declarators(&v.declarators, v.var_type.is_some(), diags, ctx);
        }
        _ => {
            for member in crate::lint::declarations::members_of(decl).unwrap_or_default() {
                scan_member(member, diags, ctx);
            }
        }
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => {
            check_type(f.field_type.as_ref(), diags, ctx);
            check_declarators(&f.declarators, f.field_type.is_some(), diags, ctx);
        }
        ClassMember::Method(m) => {
            check_type(m.return_type.as_ref(), diags, ctx);
            check_params(&m.params, diags, ctx);
            if let Some(body) = &m.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Constructor(c) => {
            check_params(&c.params, diags, ctx);
            if let Some(body) = &c.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            check_type(g.return_type.as_ref(), diags, ctx);
            if let Some(body) = &g.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            check_type(s.param_type.as_ref(), diags, ctx);
            if let Some(body) = &s.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Operator(o) => {
            check_type(o.return_type.as_ref(), diags, ctx);
            check_params(&o.params, diags, ctx);
            if let Some(body) = &o.body {
                scan_body(body, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let FunctionBody::Block(b) = body {
        struct Locals<'a, 'ctx> {
            diags: &'a mut Vec<Diagnostic>,
            ctx: &'a AnalyzeContext<'ctx>,
        }

        impl falcon_syntax::visitor::Visitor for Locals<'_, '_> {
            fn visit_local_var(&mut self, lv: &LocalVarDecl) {
                check_type(lv.var_type.as_ref(), self.diags, self.ctx);
                check_declarators(&lv.declarators, lv.var_type.is_some(), self.diags, self.ctx);
                falcon_syntax::visitor::walk_local_var(self, lv);
            }
        }

        let mut visitor = Locals { diags, ctx };
        for stmt in &b.stmts {
            falcon_syntax::visitor::Visitor::visit_stmt(&mut visitor, stmt);
        }
    }
}
