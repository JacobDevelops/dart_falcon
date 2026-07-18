//! Flags a redundant `late` modifier on an initialized static or top-level variable.
//!
//! Static fields and top-level variables in Dart are already initialized lazily:
//! their initializer does not run until the variable is first read. Marking such
//! a declaration `late` therefore buys nothing, and it misleads readers into
//! thinking deferred initialization is significant here. The rule only fires when
//! the declarator actually has an initializer, since `late` on an uninitialized
//! static or top-level variable does carry meaning. Drop the `late` keyword.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

pub struct UnnecessaryLate;

impl Rule for UnnecessaryLate {
    fn name(&self) -> &'static str {
        "unnecessary-late"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Variable(var) if var.is_late => {
                    flag_initialized(&var.declarators, ctx, &mut diags);
                }
                TopLevelDecl::Class(c) => static_late_fields(&c.members, ctx, &mut diags),
                TopLevelDecl::Mixin(m) => static_late_fields(&m.members, ctx, &mut diags),
                TopLevelDecl::MixinClass(mc) => static_late_fields(&mc.members, ctx, &mut diags),
                TopLevelDecl::Enum(e) => static_late_fields(&e.members, ctx, &mut diags),
                TopLevelDecl::Extension(e) => static_late_fields(&e.members, ctx, &mut diags),
                TopLevelDecl::ExtensionType(e) => static_late_fields(&e.members, ctx, &mut diags),
                _ => {}
            }
        }

        diags
    }
}

fn static_late_fields(members: &[ClassMember], ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    for member in members {
        if let ClassMember::Field(field) = member
            && field.is_static
            && field.is_late
        {
            flag_initialized(&field.declarators, ctx, diags);
        }
    }
}

fn flag_initialized(
    declarators: &[VarDeclarator],
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    for decl in declarators {
        if decl.initializer.is_some() {
            diags.push(Diagnostic::new(
                "unnecessary-late",
                Severity::Warning,
                "Unnecessary 'late' on an already-lazy static or top-level variable.".to_string(),
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: decl.name.span.start,
                    end: decl.name.span.end,
                },
            ));
        }
    }
}
