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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::options;
use crate::member_order::{MemberCategory, category_rank, check_sequence};

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Optional member category sequence; unset uses the built-in ordering.
    order: Option<Vec<MemberCategory>>,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct ClassMembersOrdering;


impl Rule for ClassMembersOrdering {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            order: options::get::<Options>(ctx, Self::NAME).order.as_deref(),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    order: Option<&'a [MemberCategory]>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                return StructuralTraversal::SkipChildren;
            };
            match self.order {
                Some(seq) => check_sequence(
                    members,
                    &mut self.diags,
                    self.ctx,
                    |m| category_rank(m, seq),
                    ORDER_MESSAGE,
                ),
                None => check_members(members, &mut self.diags, self.ctx),
            }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

const ORDER_MESSAGE: &str = "Class member is out of the configured order.";

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
fn member_category(member: ClassMember<'_>) -> u8 {
    match member {
        ClassMember::Field(f) => {
            if f.is_static() && f.keyword() == VarKeyword::Const {
                0
            } else if f.is_static() && f.keyword() == VarKeyword::Final {
                1
            } else if f.is_static() {
                2
            } else if f.keyword() == VarKeyword::Final {
                3
            } else {
                4
            }
        }
        ClassMember::Constructor(_) | ClassMember::PrimaryConstructorBody(_) => 5,
        ClassMember::Getter(g) => {
            if g.name().name().starts_with('_') {
                8
            } else {
                6
            }
        }
        ClassMember::Setter(s) => {
            if s.name().name().starts_with('_') {
                9
            } else {
                6
            }
        }
        ClassMember::Method(m) => {
            if m.name().name().starts_with('_') {
                9
            } else {
                7
            }
        }
        ClassMember::Operator(_) => 9,
        ClassMember::Error(_) => u8::MAX,
    }
}

fn check_members(
    members: AstChildren<'_, ClassMember<'_>>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let categories: Vec<u8> = members.clone().map(member_category).collect();
    let count = categories.len();
    let mut min_right: Vec<u8> = vec![u8::MAX; count];
    let mut max_left: Vec<u8> = vec![0; count];

    for i in (0..count).rev() {
        if i + 1 < count {
            min_right[i] = std::cmp::min(categories[i + 1], min_right[i + 1]);
        } else {
            min_right[i] = u8::MAX;
        }
    }

    for i in 0..count {
        if i > 0 {
            max_left[i] = std::cmp::max(max_left[i - 1], categories[i - 1]);
        }
    }

    for (i, member) in members.enumerate() {
        let cat = categories[i];
        if cat == u8::MAX {
            continue;
        }
        let after_smaller = min_right[i] != u8::MAX && cat > min_right[i];
        let before_larger = i > 3 && cat < max_left[i];
        if after_smaller || before_larger {
            let span = member.span();
            diags.push(ctx.diagnostic("Order class members as follows: static constants, static fields, final instance fields, mutable instance fields, constructors, public getters and setters, public methods, then private members.", span));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{AnalyzeContext, ParsedFile, RuleAnalysis};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::{path::Path, sync::Arc};

    #[test]
    fn class_members_ordering_respects_order_option() {
        // `Foo` is in the *custom* order (methods -> constructors -> fields) and out
        // of the *default* order; `Bar` is the reverse. This lets one source assert
        // that the `order` option (parity with `member-ordering`) drives the check.
        let src = r"
    class Foo {
      void run() {}
      Foo();
      int value = 0;
    }
    class Bar {
      Bar();
      void run() {}
    }
    ";
        let falcon_dart_parser::ParseResult { tree, .. } = parse(src);
        let parsed = Arc::new(ParsedFile {
            source: src.into(),
            tree,
        });
        let program = parsed.program();
        let rule = ClassMembersOrdering;
    
        // Default config → built-in pyramid order (fields → constructors → methods).
        // `Foo.run` (a method before a field/constructor) is flagged; `Bar` is fine.
        let default_cfg = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("t.dart"), &default_cfg, &parsed);
        let default_diags = rule.analyze(ClassMembersOrdering::NAME, program, &ctx);
        assert!(
            !default_diags.is_empty(),
            "default order should flag Foo (method before field)"
        );
        assert!(
            default_diags
                .iter()
                .all(|d| !d.message.contains("configured order")),
            "default path must use the built-in message, got {default_diags:?}"
        );
    
        // Custom `order` → `Foo` is now correctly ordered (0 diagnostics for it) and
        // `Bar` (constructor before method) is the out-of-order one.
        let configured: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": {
                "rules": {
                    "style": {
                        "class-members-ordering": {
                            "level": "warn",
                            "options": {
                                "order": ["public-methods", "constructors", "public-fields", "private-methods"]
                            }
                        }
                    }
                }
            }
        }))
        .expect("valid config");
        let ctx = AnalyzeContext::new(Path::new("t.dart"), &configured, &parsed);
        let diags = rule.analyze(ClassMembersOrdering::NAME, program, &ctx);
        assert_eq!(
            diags.len(),
            1,
            "custom order should flag exactly Bar.run, got {diags:?}"
        );
        assert!(
            diags[0].message.contains("configured order"),
            "order path should use the configured-order message: {}",
            diags[0].message
        );
    }
}
