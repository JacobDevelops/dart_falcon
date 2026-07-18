//! Flags class members declared with the type `Object` (or `Object?`).
//!
//! Typing a field or a method's return as `Object` discards nearly all static
//! information, forcing callers into casts or type checks and defeating the
//! type system's guarantees. A specific type documents the real contract, and
//! `dynamic` is the honest choice when a value genuinely may be anything. The
//! rule inspects only field types and the return types of methods, getters, and
//! operators; parameters, local variables, setters, and top-level declarations
//! are out of scope.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoObjectDeclaration;

impl Rule for NoObjectDeclaration {
    fn name(&self) -> &'static str {
        "no-object-declaration"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str = "Avoid using the Object type. Use a specific type or dynamic instead.";

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "no-object-declaration",
        Severity::Warning,
        MESSAGE,
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// Returns true when the type is exactly `Object` (or `Object?`).
fn is_object_type(ty: &DartType) -> bool {
    match ty {
        DartType::Named(named) => named
            .segments
            .last()
            .map(|s| s.name == "Object")
            .unwrap_or(false),
        _ => false,
    }
}

fn check_type(ty: Option<&DartType>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(ty) = ty
        && is_object_type(ty)
    {
        flag(ty.span(), diags, ctx);
    }
}

/// dart_code_linter's `no-object-declaration` only visits `FieldDeclaration`s
/// and `MethodDeclaration`s (which include getters and operators), checking the
/// field type and the member return type respectively. Parameters, local
/// variables, top-level members, and setters are out of scope.
fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let members = match decl {
        TopLevelDecl::Class(c) => &c.members,
        TopLevelDecl::Mixin(m) => &m.members,
        TopLevelDecl::MixinClass(mc) => &mc.members,
        TopLevelDecl::Enum(e) => &e.members,
        TopLevelDecl::Extension(ext) => &ext.members,
        _ => return,
    };
    for m in members {
        scan_member(m, diags, ctx);
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => check_type(f.field_type.as_ref(), diags, ctx),
        ClassMember::Method(m) => check_type(m.return_type.as_ref(), diags, ctx),
        ClassMember::Getter(g) => check_type(g.return_type.as_ref(), diags, ctx),
        ClassMember::Operator(o) => check_type(o.return_type.as_ref(), diags, ctx),
        _ => {}
    }
}
