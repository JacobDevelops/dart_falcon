//! Shared helpers for the two member-ordering rules — `member-ordering` (DCL)
//! and `class-members-ordering` (pyramid). Both accept an `order` option listing
//! DCL category tokens; these helpers model that option, map a member to the
//! category tokens it qualifies for, rank it against a configured order, and
//! flag any member that appears earlier than one that should precede it.

use falcon_analyze::AnalyzeContext;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(
    Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MemberCategory {
    PublicFields,
    PrivateFields,
    Fields,
    StaticFields,
    Constructors,
    NamedConstructors,
    StaticMethods,
    PrivateMethods,
    PublicMethods,
    Methods,
    Getters,
    Setters,
}

#[derive(
    Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WidgetCategory {
    Constructor,
    InitState,
    DidChangeDependencies,
    DidUpdateWidget,
    Dispose,
    Build,
    OverriddenMethods,
}

pub(crate) fn is_override<'a>(mut annotations: AstChildren<'a, Annotation<'a>>) -> bool {
    annotations.any(|annotation| {
        annotation
            .name()
            .next_back()
            .is_some_and(|id| id.name() == "override")
    })
}

fn member_name(member: ClassMember<'_>) -> Option<&str> {
    match member {
        ClassMember::Field(f) => f.declarators().next().map(|d| d.name().name()),
        ClassMember::Method(m) => Some(m.name().name()),
        ClassMember::Getter(g) => Some(g.name().name()),
        ClassMember::Setter(s) => Some(s.name().name()),
        ClassMember::Constructor(c) => c.constructor_name().map(|n| n.name()),
        _ => None,
    }
}

fn is_private(member: ClassMember<'_>) -> bool {
    member_name(member).is_some_and(|n| n.starts_with('_'))
}

/// All DCL category tokens a member qualifies for (most specific first). The
/// member's rank is the earliest of these that appears in the user's `order`.
pub(crate) fn member_tokens(member: ClassMember<'_>) -> Vec<MemberCategory> {
    use MemberCategory::*;
    match member {
        ClassMember::Field(f) => {
            let mut t = Vec::new();
            if f.is_static() {
                t.push(StaticFields);
            }
            if is_private(member) {
                t.push(PrivateFields);
            } else {
                t.push(PublicFields);
            }
            t.push(Fields);
            t
        }
        ClassMember::Constructor(c) => {
            let mut t = Vec::new();
            if c.constructor_name().is_some() {
                t.push(NamedConstructors);
            }
            t.push(Constructors);
            t
        }
        ClassMember::Method(m) => {
            let mut t = Vec::new();
            if m.is_static() {
                t.push(StaticMethods);
            }
            if is_private(member) {
                t.push(PrivateMethods);
            } else {
                t.push(PublicMethods);
            }
            t.push(Methods);
            t
        }
        ClassMember::Getter(_) => vec![Getters, Methods],
        ClassMember::Setter(_) => vec![Setters, Methods],
        ClassMember::Operator(_) => vec![Methods],
        ClassMember::PrimaryConstructorBody(_) => vec![Constructors],
        ClassMember::Error(_) => Vec::new(),
    }
}

/// Rank a member by the earliest of its category tokens that appears in `order`.
pub(crate) fn category_rank(member: ClassMember<'_>, order: &[MemberCategory]) -> Option<usize> {
    let tokens = member_tokens(member);
    order.iter().position(|cat| tokens.iter().any(|t| t == cat))
}

/// Generic ordering check: each member is assigned a rank; a member whose rank
/// is lower than the highest rank seen so far is out of order. Members with no
/// rank (not covered by the configured categories) are skipped.
pub fn check_sequence<'a>(
    members: impl IntoIterator<Item = ClassMember<'a>>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    rank_of: impl Fn(ClassMember<'a>) -> Option<usize>,
    message: &str,
) {
    let mut max_seen: Option<usize> = None;
    for member in members {
        let Some(rank) = rank_of(member) else {
            continue;
        };
        match max_seen {
            Some(prev) if rank < prev => {
                let span = member.span();
                diags.push(ctx.diagnostic(message, span));
            }
            _ => max_seen = Some(rank),
        }
    }
}
