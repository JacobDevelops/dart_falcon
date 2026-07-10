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
        let order = read_string_list(ctx, "order");
        let widgets_order = read_string_list(ctx, "widgets_order");

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => check_class(
                    &c.members,
                    c.extends.as_ref(),
                    &order,
                    &widgets_order,
                    &mut diags,
                    ctx,
                ),
                TopLevelDecl::Mixin(m) => {
                    check_class(&m.members, None, &order, &widgets_order, &mut diags, ctx)
                }
                TopLevelDecl::MixinClass(mc) => check_class(
                    &mc.members,
                    mc.extends.as_ref(),
                    &order,
                    &widgets_order,
                    &mut diags,
                    ctx,
                ),
                _ => {}
            }
        }
        diags
    }
}

/// Read a `member-ordering` option as a list of lowercase category tokens.
fn read_string_list(ctx: &AnalyzeContext, key: &str) -> Option<Vec<String>> {
    crate::meta::meta_for("member-ordering")
        .and_then(|m| ctx.config.rule_options(m.group, "member-ordering"))
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
}

fn check_class(
    members: &[ClassMember],
    extends: Option<&DartType>,
    order: &Option<Vec<String>>,
    widgets_order: &Option<Vec<String>>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    // A Flutter State subclass with a configured `widgets_order` uses the
    // lifecycle sequence; otherwise `order` (if any) drives the check; otherwise
    // the built-in default order.
    if let Some(seq) = widgets_order
        && extends_state(extends)
    {
        check_sequence(
            members,
            diags,
            ctx,
            |m| widget_rank(m, seq),
            widgets_message(),
        );
    } else if let Some(seq) = order {
        check_sequence(
            members,
            diags,
            ctx,
            |m| category_rank(m, seq),
            order_message(),
        );
    } else {
        check_default(members, diags, ctx);
    }
}

/// True when the class extends `State` / `State<T>` (Flutter widget state).
fn extends_state(extends: Option<&DartType>) -> bool {
    matches!(extends, Some(DartType::Named(nt)) if nt.segments.last().is_some_and(|id| id.name == "State"))
}

/// Generic ordering check: each member is assigned a rank; a member whose rank
/// is lower than the highest rank seen so far is out of order. Members with no
/// rank (not covered by the configured categories) are skipped.
fn check_sequence(
    members: &[ClassMember],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    rank_of: impl Fn(&ClassMember) -> Option<usize>,
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
                diags.push(Diagnostic::new(
                    "member-ordering",
                    Severity::Warning,
                    message,
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
            _ => max_seen = Some(rank),
        }
    }
}

fn member_name(member: &ClassMember) -> Option<&str> {
    match member {
        ClassMember::Field(f) => f.declarators.first().map(|d| d.name.name.as_str()),
        ClassMember::Method(m) => Some(m.name.name.as_str()),
        ClassMember::Getter(g) => Some(g.name.name.as_str()),
        ClassMember::Setter(s) => Some(s.name.name.as_str()),
        ClassMember::Constructor(c) => c.constructor_name.as_ref().map(|n| n.name.as_str()),
        _ => None,
    }
}

fn is_private(member: &ClassMember) -> bool {
    member_name(member).is_some_and(|n| n.starts_with('_'))
}

/// All DCL category tokens a member qualifies for (most specific first). The
/// member's rank is the earliest of these that appears in the user's `order`.
fn member_tokens(member: &ClassMember) -> Vec<&'static str> {
    match member {
        ClassMember::Field(f) => {
            let mut t = Vec::new();
            if f.is_static {
                t.push("static-fields");
            }
            if is_private(member) {
                t.push("private-fields");
            } else {
                t.push("public-fields");
            }
            t.push("fields");
            t
        }
        ClassMember::Constructor(c) => {
            let mut t = Vec::new();
            if c.constructor_name.is_some() {
                t.push("named-constructors");
            }
            t.push("constructors");
            t
        }
        ClassMember::Method(m) => {
            let mut t = Vec::new();
            if m.is_static {
                t.push("static-methods");
            }
            if is_private(member) {
                t.push("private-methods");
            } else {
                t.push("public-methods");
            }
            t.push("methods");
            t
        }
        ClassMember::Getter(_) => vec!["getters", "methods"],
        ClassMember::Setter(_) => vec!["setters", "methods"],
        ClassMember::Operator(_) => vec!["methods"],
        ClassMember::Error(_) => Vec::new(),
    }
}

fn category_rank(member: &ClassMember, order: &[String]) -> Option<usize> {
    let tokens = member_tokens(member);
    order.iter().position(|cat| tokens.iter().any(|t| t == cat))
}

/// Lifecycle token for a State member, for `widgets_order`. `overridden-methods`
/// is a best-effort catch-all for any other `@override` method.
fn widget_token(member: &ClassMember) -> Option<&'static str> {
    match member {
        ClassMember::Constructor(_) => Some("constructor"),
        ClassMember::Method(m) => match m.name.name.as_str() {
            "initState" => Some("init-state"),
            "didChangeDependencies" => Some("did-change-dependencies"),
            "didUpdateWidget" => Some("did-update-widget"),
            "dispose" => Some("dispose"),
            "build" => Some("build"),
            _ if is_override(&m.annotations) => Some("overridden-methods"),
            _ => None,
        },
        _ => None,
    }
}

fn widget_rank(member: &ClassMember, seq: &[String]) -> Option<usize> {
    let token = widget_token(member)?;
    seq.iter().position(|t| t == token)
}

fn is_override(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|a| a.name.last().is_some_and(|id| id.name == "override"))
}

fn order_message() -> &'static str {
    "Class member is out of the configured order"
}

fn widgets_message() -> &'static str {
    "State lifecycle method is out of the configured widgets_order"
}

// ── Default ordering (no `order` option configured) ──────────────────────────
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

fn check_default(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
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
