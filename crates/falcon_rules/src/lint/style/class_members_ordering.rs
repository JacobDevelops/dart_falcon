//! Flags class members declared out of a canonical order.
//!
//! A consistent member layout lets readers find fields, constructors, and
//! methods in the same place across every class. Without options the built-in
//! order applies: static constants, static fields, instance `final` fields,
//! instance mutable fields, constructors, public getters and setters, public
//! methods, then private members. Members are ranked by category and any pair
//! found out of sequence is reported.
//!
//! ## Options
//!
//! `order` (list of category tokens, default: the built-in order above) — the
//! required member sequence. Supported tokens are `public-fields`,
//! `private-fields`, `fields`, `static-fields`, `constructors`,
//! `named-constructors`, `static-methods`, `private-methods`, `public-methods`,
//! `methods`, `getters`, and `setters`. A member is ranked by the earliest
//! token it qualifies for; members matching no token are ignored.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::member_order::{category_rank, check_sequence, read_string_list};

#[derive(Default)]
pub struct ClassMembersOrdering;

const NAME: &str = "class-members-ordering";

impl Rule for ClassMembersOrdering {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        // A configured `order` option (a list of DCL category tokens) drives the
        // check; otherwise the built-in default order is used.
        let order = read_string_list(ctx, NAME, "order");
        for decl in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                continue;
            };
            match &order {
                Some(seq) => check_sequence(
                    members,
                    &mut diags,
                    ctx,
                    NAME,
                    |m| category_rank(m, seq),
                    ORDER_MESSAGE,
                ),
                None => check_members(members, &mut diags, ctx),
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

const ORDER_MESSAGE: &str = "Class member is out of the configured order";

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
            if f.is_static && f.keyword == falcon_syntax::ast::VarKeyword::Const {
                0
            } else if f.is_static && f.keyword == falcon_syntax::ast::VarKeyword::Final {
                1
            } else if f.is_static {
                2
            } else if f.keyword == falcon_syntax::ast::VarKeyword::Final {
                3
            } else {
                4
            }
        }
        ClassMember::Constructor(_) | ClassMember::PrimaryConstructorBody(_) => 5,
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

    for i in (0..members.len()).rev() {
        if i + 1 < members.len() {
            min_right[i] = std::cmp::min(categories[i + 1], min_right[i + 1]);
        } else {
            min_right[i] = u8::MAX;
        }
    }

    for i in 0..members.len() {
        if i > 0 {
            max_left[i] = std::cmp::max(max_left[i - 1], categories[i - 1]);
        }
    }

    for (i, member) in members.iter().enumerate() {
        let cat = categories[i];
        if cat == u8::MAX {
            continue;
        }
        let after_smaller = min_right[i] != u8::MAX && cat > min_right[i];
        let before_larger = i > 3 && cat < max_left[i];
        if after_smaller || before_larger {
            let span = member.span();
            diags.push(ctx.diagnostic(NAME, "Class members should be ordered: static const → static fields → instance final → instance var → constructors → public getters/setters → public methods → private members", *span));
        }
    }
}
