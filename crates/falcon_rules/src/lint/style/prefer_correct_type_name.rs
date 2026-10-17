//! Flags type declarations whose name is not well-formed UpperCamelCase.
//!
//! Consistent UpperCamelCase type names are a core Dart convention that keeps
//! types visually distinct from members and values. A name is accepted only when
//! it starts with an uppercase ASCII letter, contains no `$`, and — after a
//! single leading underscore is stripped — is between 3 and 40 characters long;
//! anything else is reported. The check covers classes, mixins, mixin classes,
//! enums, named extensions, extension types, and type aliases. It inspects the
//! declared name's spelling only, so it does not enforce interior capitalization
//! such as an acronym written in all caps.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferCorrectTypeName;

impl Rule for PreferCorrectTypeName {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) -> StructuralTraversal {
        match decl {
                TopLevelDecl::Class(c) => {
                    check_type_name(&c.name, &mut self.diags, self.ctx);
                }
                TopLevelDecl::Mixin(m) => {
                    check_type_name(&m.name, &mut self.diags, self.ctx);
                }
                TopLevelDecl::MixinClass(mc) => {
                    check_type_name(&mc.name, &mut self.diags, self.ctx);
                }
                TopLevelDecl::Enum(e) => {
                    check_type_name(&e.name, &mut self.diags, self.ctx);
                }
                TopLevelDecl::Extension(ext) => {
                    if let Some(name) = &ext.name {
                        check_type_name(name, &mut self.diags, self.ctx);
                    }
                }
                TopLevelDecl::ExtensionType(et) => {
                    check_type_name(&et.name, &mut self.diags, self.ctx);
                }
                TopLevelDecl::TypeAlias(ta) => {
                    check_type_name(&ta.name, &mut self.diags, self.ctx);
                }
                _ => {}
            }
        StructuralTraversal::SkipChildren
    }
}
}

fn check_type_name(name_ident: &Identifier, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let name = &name_ident.name;

    // Strip a single leading underscore
    let core = name.strip_prefix('_').unwrap_or(name);

    let char_count = core.chars().count();
    let is_valid = core.starts_with(|c: char| c.is_ascii_uppercase())
        && (3..=40).contains(&char_count)
        && !name.contains('$');

    if !is_valid {
        diags.push(ctx.diagnostic(
            "prefer-correct-type-name",
            format!(
                "Type name '{}' should be in UpperCamelCase and between 3 and 40 characters.",
                name
            ),
            name_ident.span,
        ));
    }
}
