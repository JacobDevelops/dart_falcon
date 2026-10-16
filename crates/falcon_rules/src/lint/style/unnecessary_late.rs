//! Flags a redundant `late` modifier on an initialized static or top-level variable.
//!
//! Static fields and top-level variables in Dart are already initialized lazily:
//! their initializer does not run until the variable is first read. Marking such
//! a declaration `late` therefore buys nothing, and it misleads readers into
//! thinking deferred initialization is significant here. The rule only fires when
//! the declarator actually has an initializer, since `late` on an uninitialized
//! static or top-level variable does carry meaning. Drop the `late` keyword.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UnnecessaryLate;

impl Rule for UnnecessaryLate {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Variable(var) if var.is_late => {
                    flag_initialized(&var.declarators, ctx, &mut diags);
                }
                _ => {
                    if let Some(members) = crate::lint::declarations::members_of(decl) {
                        static_late_fields(members, ctx, &mut diags);
                    }
                }
            }
        }

        RulePreparation::Diagnostics(diags)
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
            diags.push(ctx.diagnostic(
                "unnecessary-late",
                "Unnecessary 'late' on an already-lazy static or top-level variable.".to_string(),
                decl.name.span,
            ));
        }
    }
}
