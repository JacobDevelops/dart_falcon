//! Flags type names that are not UpperCamelCase.
//!
//! Classes, mixins, enums, extension types, and type aliases are all types and
//! read most predictably when named in UpperCamelCase, matching the rest of the
//! Dart ecosystem. Leading underscores are ignored, then each word must begin
//! with an uppercase letter or `$`; a name that is entirely underscores is
//! treated as a wildcard and never flagged.

use crate::lint::naming::is_upper_camel_case;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct CamelCaseTypes;

impl Rule for CamelCaseTypes {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => check(&c.name, &mut diags, ctx),
                TopLevelDecl::MixinClass(c) => check(&c.name, &mut diags, ctx),
                TopLevelDecl::Mixin(m) => check(&m.name, &mut diags, ctx),
                TopLevelDecl::Enum(e) => check(&e.name, &mut diags, ctx),
                TopLevelDecl::ExtensionType(x) => check(&x.name, &mut diags, ctx),
                TopLevelDecl::TypeAlias(t) => check(&t.name, &mut diags, ctx),
                _ => {}
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

const MESSAGE: &str = "Name types using UpperCamelCase.";

fn check(name: &Identifier, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !is_upper_camel_case(&name.name) {
        diags.push(ctx.diagnostic("camel-case-types", MESSAGE, name.span));
    }
}
