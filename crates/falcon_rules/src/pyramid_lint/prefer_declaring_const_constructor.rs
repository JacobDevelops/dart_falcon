use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferDeclaringConstConstructor;

impl Rule for PreferDeclaringConstConstructor {
    fn name(&self) -> &'static str {
        "prefer_declaring_const_constructor"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let TopLevelDecl::Class(class_decl) = decl {
                check_class(class_decl, &mut diags, ctx);
            }
        }
        diags
    }
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "prefer_declaring_const_constructor",
        Severity::Warning,
        "Constructor could be declared as const.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// Conservatively decide whether an initializer expression evaluates to a
/// constant. Without element resolution we accept only literals and explicit
/// `const` constructor invocations (mirrors pyramid_lint accepting literals /
/// `inConstantContext`, minus the cases we cannot prove).
fn is_const_evaluable(expr: &Expr) -> bool {
    match expr {
        Expr::BoolLit { .. }
        | Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::StringLit(_)
        | Expr::NullLit { .. } => true,
        Expr::New { is_const, .. } => *is_const,
        _ => false,
    }
}

/// True when a type names bare `Object` (an implicit-or-explicit `Object`
/// superclass has a const constructor, so it does not block const-ness).
fn is_object_named(ty: &DartType) -> bool {
    match ty {
        DartType::Named(named) => named
            .segments
            .last()
            .map(|s| s.name == "Object")
            .unwrap_or(false),
        _ => false,
    }
}

fn all_fields_final(class_decl: &ClassDecl) -> bool {
    class_decl
        .members
        .iter()
        .all(|member| !matches!(member, ClassMember::Field(f) if !f.is_final && !f.is_const))
}

/// Every field-declaration initializer must itself be const-evaluable, else the
/// class cannot be const even with all-final fields.
fn field_initializers_const(class_decl: &ClassDecl) -> bool {
    for member in &class_decl.members {
        if let ClassMember::Field(field) = member {
            for d in &field.declarators {
                if let Some(init) = &d.initializer
                    && !is_const_evaluable(init)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn constructor_is_const_candidate(ctor: &ConstructorDecl) -> bool {
    if ctor.is_const || ctor.is_factory || ctor.is_external {
        return false;
    }

    for init in &ctor.initializers {
        match init {
            // We cannot verify that the target super/redirecting constructor is
            // itself const, so treat these as blocking.
            ConstructorInitializer::SuperCall { .. } | ConstructorInitializer::ThisCall { .. } => {
                return false;
            }
            ConstructorInitializer::FieldInit { value, .. } => {
                if !is_const_evaluable(value) {
                    return false;
                }
            }
            // Asserts are permitted in const constructors.
            ConstructorInitializer::Assert { .. } => {}
        }
    }

    // A const constructor cannot have a (non-empty) body.
    match &ctor.body {
        None => true,
        Some(FunctionBody::Block(block)) => block.stmts.is_empty(),
        Some(_) => false,
    }
}

fn check_class(class_decl: &ClassDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Skip subclasses of a non-Object superclass (unknown super const-ness) and
    // classes applying mixins (unknown mixin const-ness).
    if let Some(ext) = &class_decl.extends
        && !is_object_named(ext)
    {
        return;
    }
    if !class_decl.with_clause.is_empty() {
        return;
    }
    if !all_fields_final(class_decl) {
        return;
    }
    if !field_initializers_const(class_decl) {
        return;
    }

    for member in &class_decl.members {
        if let ClassMember::Constructor(ctor) = member
            && constructor_is_const_candidate(ctor)
        {
            flag(&ctor.span, diags, ctx);
        }
    }
}
