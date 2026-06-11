use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct MemberOrdering;

impl Rule for MemberOrdering {
    fn name(&self) -> &'static str {
        "member-ordering"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => check_members(&c.members, &mut diags, ctx),
                TopLevelDecl::Mixin(m) => check_members(&m.members, &mut diags, ctx),
                TopLevelDecl::MixinClass(mc) => check_members(&mc.members, &mut diags, ctx),
                _ => {}
            }
        }
        diags
    }
}

// Category ordering (lower = earlier in class):
// 0: static const field
// 1: static non-const field
// 2: instance field
// 3: constructor
// 4: static method/getter/setter/operator
// 5: instance method/getter/setter/operator
fn member_category(member: &ClassMember) -> u8 {
    match member {
        ClassMember::Field(f) if f.is_static && f.is_const => 0,
        ClassMember::Field(f) if f.is_static => 1,
        ClassMember::Field(_) => 2,
        ClassMember::Constructor(_) => 3,
        ClassMember::Method(m) if m.is_static => 4,
        ClassMember::Getter(g) if g.is_static => 4,
        ClassMember::Setter(s) if s.is_static => 4,
        ClassMember::Method(_)
        | ClassMember::Getter(_)
        | ClassMember::Setter(_)
        | ClassMember::Operator(_) => 5,
        ClassMember::Error(_) => u8::MAX,
    }
}

fn check_members(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let mut max_seen: u8 = 0;
    for member in members {
        let cat = member_category(member);
        if cat == u8::MAX {
            continue;
        }
        if cat < max_seen {
            let span = member.span();
            diags.push(Diagnostic::new(
                "member-ordering",
                Severity::Warning,
                "Class members are out of order: expected static const → static fields → instance fields → constructors → static methods → instance methods",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: span.start, end: span.end },
            ));
        } else {
            max_seen = cat;
        }
    }
}
