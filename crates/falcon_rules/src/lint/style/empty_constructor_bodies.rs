//! Flags constructors with an empty block body `{}`, ported from package:lints
//! `empty_constructor_bodies`. An empty body should be written as `;` instead.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct EmptyConstructorBodies;

impl Rule for EmptyConstructorBodies {
    fn name(&self) -> &'static str {
        "empty-constructor-bodies"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let members = match decl {
                TopLevelDecl::Class(c) => &c.members,
                TopLevelDecl::Mixin(m) => &m.members,
                TopLevelDecl::MixinClass(mc) => &mc.members,
                TopLevelDecl::Enum(e) => &e.members,
                TopLevelDecl::Extension(e) => &e.members,
                TopLevelDecl::ExtensionType(e) => &e.members,
                _ => continue,
            };
            for member in members {
                if let ClassMember::Constructor(c) = member {
                    check_constructor(c, &mut diags, ctx);
                }
            }
        }
        diags
    }
}

fn check_constructor(ctor: &ConstructorDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let FunctionBody::Block(block) = (match &ctor.body {
        Some(b) => b,
        None => return,
    }) else {
        return;
    };
    if !block.stmts.is_empty() {
        return;
    }
    diags.push(Diagnostic::new(
        "empty-constructor-bodies",
        Severity::Warning,
        "Empty constructor body — use `;` instead of `{}`",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: block.span.start,
            end: block.span.start + 1,
        },
    ));
}
