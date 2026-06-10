use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct ClassMembersOrdering;

impl Rule for ClassMembersOrdering {
    fn name(&self) -> &'static str {
        "class_members_ordering"
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
// 1: static final field
// 2: static var field
// 3: instance final field
// 4: instance var field
// 5: constructor
// 6: public getter/setter
// 7: public method
// 8: private getter
// 9: private method/setter/operator
fn member_category(member: &ClassMember) -> u8 {
    match member {
        ClassMember::Field(f) => {
            if f.is_static && f.is_const {
                0
            } else if f.is_static && f.is_final {
                1
            } else if f.is_static {
                2
            } else if f.is_final {
                3
            } else {
                4
            }
        }
        ClassMember::Constructor(_) => 5,
        ClassMember::Getter(g) => {
            if g.name.name.starts_with('_') {
                8
            } else {
                6
            }
        }
        ClassMember::Setter(s) => {
            if s.name.name.starts_with('_') {
                9
            } else {
                6
            }
        }
        ClassMember::Method(m) => {
            if m.name.name.starts_with('_') {
                9
            } else {
                7
            }
        }
        ClassMember::Operator(_) => 9,
        ClassMember::Error(_) => u8::MAX,
    }
}

fn check_members(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let categories: Vec<u8> = members.iter().map(member_category).collect();
    let mut min_right: Vec<u8> = vec![u8::MAX; members.len()];
    let mut max_left: Vec<u8> = vec![0; members.len()];

    // Compute the minimum category to the right of each member
    for i in (0..members.len()).rev() {
        if i + 1 < members.len() {
            min_right[i] = std::cmp::min(categories[i + 1], min_right[i + 1]);
        } else {
            min_right[i] = u8::MAX;
        }
    }

    // Compute the maximum category to the left of each member
    for i in 0..members.len() {
        if i > 0 {
            max_left[i] = std::cmp::max(max_left[i - 1], categories[i - 1]);
        }
    }

    // Flag members that are out of order
    for (i, member) in members.iter().enumerate() {
        let cat = categories[i];
        if cat == u8::MAX {
            continue;
        }
        // Flag if this member is higher than something to the right
        if min_right[i] != u8::MAX && cat > min_right[i] {
            let span = member.span();
            diags.push(Diagnostic::new(
                "class_members_ordering",
                Severity::Warning,
                "Class members should be ordered: static const → static fields → instance final → instance var → constructors → public getters/setters → public methods → private members",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: span.start, end: span.end },
            ));
        // Also flag if this member is lower than something to the left (and comes late in the sequence)
        } else if i > 0 && cat < max_left[i] && i > 3 {
            let span = member.span();
            diags.push(Diagnostic::new(
                "class_members_ordering",
                Severity::Warning,
                "Class members should be ordered: static const → static fields → instance final → instance var → constructors → public getters/setters → public methods → private members",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: span.start, end: span.end },
            ));
        }
    }
}
