//! Flags setters declared with an explicit return type.
//!
//! A setter never returns a value, so annotating one with a return type — even
//! `void` — is redundant and can mislead readers into thinking the result is
//! usable. Drop the return type and write `set foo(int v)`. Both top-level
//! setters and setters declared in classes, mixins, enums, and extensions are
//! checked.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

pub struct AvoidReturnTypesOnSetters;

impl Rule for AvoidReturnTypesOnSetters {
    fn name(&self) -> &'static str {
        "avoid-return-types-on-setters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                // Top-level setters keep their return type in the AST directly.
                TopLevelDecl::Function(func) if func.is_setter && func.return_type.is_some() => {
                    push(&mut diags, ctx, &func.name.span);
                }
                TopLevelDecl::Class(c) => check_members(&c.members, ctx, &mut diags),
                TopLevelDecl::Mixin(m) => check_members(&m.members, ctx, &mut diags),
                TopLevelDecl::MixinClass(mc) => check_members(&mc.members, ctx, &mut diags),
                TopLevelDecl::Enum(e) => check_members(&e.members, ctx, &mut diags),
                TopLevelDecl::Extension(e) => check_members(&e.members, ctx, &mut diags),
                TopLevelDecl::ExtensionType(e) => check_members(&e.members, ctx, &mut diags),
                _ => {}
            }
        }

        diags
    }
}

fn check_members(members: &[ClassMember], ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    for member in members {
        if let ClassMember::Setter(setter) = member
            && setter_has_return_type(setter, ctx.source)
        {
            push(diags, ctx, &setter.name.span);
        }
    }
}

// A class setter's return type is parsed and dropped, so recover it from the
// source slice preceding the `set` keyword. Everything between the last
// annotation and the setter name is `[modifiers] [return type] set`; if a token
// other than `static`/`external`/`set` survives, a return type was written.
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
    let remaining: Vec<&str> = stripped
        .split_whitespace()
        .filter(|w| !matches!(*w, "static" | "external" | "set"))
        .collect();
    !remaining.is_empty()
}

// Comments in the modifier region are not return types. Mirrors the comment arms
// of `skip_trivia` in `type_init_formals.rs` (kept local rather than shared).
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

fn push(diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, span: &Span) {
    diags.push(Diagnostic::new(
        "avoid-return-types-on-setters",
        Severity::Warning,
        "Avoid return types on setters.".to_string(),
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
