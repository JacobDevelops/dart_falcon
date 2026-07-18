//! Flags a class/mixin that overrides `==` without `hashCode` (or vice versa).
//! Ported from package:lints' `hash_and_equals`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct HashAndEquals;

impl Rule for HashAndEquals {
    fn name(&self) -> &'static str {
        "hash-and-equals"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let file = ctx.file_path.to_string_lossy().into_owned();
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let members = match decl {
                TopLevelDecl::Class(c) => &c.members,
                TopLevelDecl::Mixin(m) => &m.members,
                TopLevelDecl::MixinClass(mc) => &mc.members,
                _ => continue,
            };
            check_members(members, &file, &mut diags);
        }
        diags
    }
}

/// The span of the `==` operator, if the type declares one.
fn equals_span(members: &[ClassMember]) -> Option<&Span> {
    members.iter().find_map(|m| match m {
        ClassMember::Operator(op) if op.op == "==" => Some(&op.span),
        _ => None,
    })
}

/// The span of a `hashCode` override — as a getter, a field, or (defensively) a
/// method — if the type declares one.
fn hashcode_span(members: &[ClassMember]) -> Option<&Span> {
    members.iter().find_map(|m| match m {
        ClassMember::Getter(g) if g.name.name == "hashCode" => Some(&g.span),
        ClassMember::Method(mt) if mt.name.name == "hashCode" => Some(&mt.span),
        ClassMember::Field(f) if f.declarators.iter().any(|d| d.name.name == "hashCode") => {
            Some(&f.span)
        }
        _ => None,
    })
}

fn check_members(members: &[ClassMember], file: &str, diags: &mut Vec<Diagnostic>) {
    let eq = equals_span(members);
    let hash = hashcode_span(members);
    let (span, message) = match (eq, hash) {
        (Some(eq), None) => (eq, "Override 'hashCode' if you override the '==' operator."),
        (None, Some(hash)) => (
            hash,
            "Override the '==' operator if you override 'hashCode'.",
        ),
        _ => return,
    };
    diags.push(Diagnostic::new(
        "hash-and-equals",
        Severity::Warning,
        message,
        file.to_string(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
