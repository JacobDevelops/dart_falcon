use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferCorrectTypeName;

impl Rule for PreferCorrectTypeName {
    fn name(&self) -> &'static str {
        "prefer-correct-type-name"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => {
                    check_type_name(&c.name, &mut diags, ctx);
                }
                TopLevelDecl::Mixin(m) => {
                    check_type_name(&m.name, &mut diags, ctx);
                }
                TopLevelDecl::MixinClass(mc) => {
                    check_type_name(&mc.name, &mut diags, ctx);
                }
                TopLevelDecl::Enum(e) => {
                    check_type_name(&e.name, &mut diags, ctx);
                }
                TopLevelDecl::Extension(ext) => {
                    if let Some(name) = &ext.name {
                        check_type_name(name, &mut diags, ctx);
                    }
                }
                TopLevelDecl::ExtensionType(et) => {
                    check_type_name(&et.name, &mut diags, ctx);
                }
                TopLevelDecl::TypeAlias(ta) => {
                    check_type_name(&ta.name, &mut diags, ctx);
                }
                _ => {}
            }
        }
        diags
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
        diags.push(Diagnostic::new(
            "prefer-correct-type-name",
            Severity::Warning,
            format!(
                "Type name '{}' should be in UpperCamelCase and between 3 and 40 characters.",
                name
            ),
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: name_ident.span.start,
                end: name_ident.span.end,
            },
        ));
    }
}
