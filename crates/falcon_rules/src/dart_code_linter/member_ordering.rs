use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

use crate::member_order::{category_rank, check_sequence, read_string_list};

pub struct MemberOrdering;

const NAME: &str = "member-ordering";

impl Rule for MemberOrdering {
    fn name(&self) -> &'static str {
        NAME
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let order = read_string_list(ctx, NAME, "order");
        let widgets_order = read_string_list(ctx, NAME, "widgets_order");

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
            NAME,
            |m| widget_rank(m, seq),
            widgets_message(),
        );
    } else if let Some(seq) = order {
        check_sequence(
            members,
            diags,
            ctx,
            NAME,
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
