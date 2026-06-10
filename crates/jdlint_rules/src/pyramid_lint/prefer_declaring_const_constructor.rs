use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

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
        DiagSpan { start: span.start, end: span.end },
    ));
}

fn all_fields_const_or_final(class_decl: &ClassDecl) -> bool {
    for member in &class_decl.members {
        if let ClassMember::Field(field) = member
            && !field.is_final && !field.is_const {
                return false;
            }
    }
    true
}

fn constructor_body_is_const_safe(constructor: &ConstructorDecl) -> bool {
    // Check if body has no non-const operations
    // Empty body or only initializers is safe
    if constructor.body.is_none() {
        return true;
    }

    if let Some(FunctionBody::Block(block)) = &constructor.body {
        // If body has any actual statements (beyond initializers), it's not purely const-safe
        // For now, we allow empty blocks or blocks with only simple statements
        // The safest check: if there's a non-empty block, it's not const-safe
        return block.stmts.is_empty();
    }

    false
}

fn check_class(class_decl: &ClassDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Only check if all fields are final or const
    if !all_fields_const_or_final(class_decl) {
        return;
    }

    for member in &class_decl.members {
        if let ClassMember::Constructor(constructor) = member {
            // Skip if already const
            if constructor.is_const {
                continue;
            }

            // Skip factory constructors
            if constructor.is_factory {
                continue;
            }

            // Skip external constructors
            if constructor.is_external {
                continue;
            }

            // Check if body is safe for const
            if constructor_body_is_const_safe(constructor) {
                flag(&constructor.span, diags, ctx);
            }
        }
    }
}
