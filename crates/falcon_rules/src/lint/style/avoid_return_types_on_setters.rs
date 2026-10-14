//! Flags setters declared with an explicit return type.
//!
//! A setter never returns a value, so annotating one with a return type — even
//! `void` — is redundant and can mislead readers into thinking the result is
//! usable. Drop the return type and write `set foo(int v)`. Both top-level
//! setters and setters declared in classes, mixins, enums, and extensions are
//! checked.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_top_level_decl};

#[derive(Default)]
pub struct AvoidReturnTypesOnSetters;

impl Rule for AvoidReturnTypesOnSetters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);

        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) {
        match decl {
            TopLevelDecl::Function(function) => self.visit_function_decl(function),
            TopLevelDecl::Class(_)
            | TopLevelDecl::Mixin(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Enum(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_) => walk_top_level_decl(self, decl),
            _ => {}
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember) {
        if let ClassMember::Setter(setter) = member {
            self.visit_setter_decl(setter);
        }
    }

    fn visit_expr(&mut self, _: &Expr) {}

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if matches!(node.kind, FunctionKind::Setter(_)) && node.return_type.is_some() {
            self.diags.push(self.ctx.diagnostic(
                "avoid-return-types-on-setters",
                "Avoid return types on setters.",
                node.name.span,
            ));
        }
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        if setter_has_return_type(node, self.ctx.source) {
            self.diags.push(self.ctx.diagnostic(
                "avoid-return-types-on-setters",
                "Avoid return types on setters.",
                node.name.span,
            ));
        }
    }
}

// Member setter return types are absent from the AST, so recover the modifier region.
fn setter_has_return_type(setter: &SetterDecl, source: &str) -> bool {
    let ann_end = setter
        .annotations
        .iter()
        .map(|a| a.span.end)
        .max()
        .unwrap_or(setter.span.start)
        .max(setter.span.start);
    let region = match source.get(ann_end..setter.name.span.start) {
        Some(r) => r,
        None => return false,
    };
    let stripped = strip_comments(region);
    stripped
        .split_whitespace()
        .any(|word| !matches!(word, "static" | "external" | "set"))
}

// Comments are not return types; keep this scanner aligned with type_init_formals::skip_trivia.
fn strip_comments(region: &str) -> String {
    let bytes = region.as_bytes();
    let mut out = String::with_capacity(region.len());
    let (mut i, mut kept_from) = (0, 0);
    while i < bytes.len() {
        let end = match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let mut j = i + 2;
                while j < bytes.len() && bytes[j] != b'\n' {
                    j += 1;
                }
                j
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let mut j = i + 2;
                while j < bytes.len() && !(bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/')) {
                    j += 1;
                }
                (j + 2).min(bytes.len())
            }
            _ => {
                i += 1;
                continue;
            }
        };
        out.push_str(&region[kept_from..i]);
        out.push(' ');
        i = end;
        kept_from = end;
    }
    out.push_str(&region[kept_from..]);
    out
}
