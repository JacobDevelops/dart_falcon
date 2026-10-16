//! Disallow functions and methods that return a `Widget`.
//!
//! Flags a function, method, or getter whose return type is `Widget`,
//! `StatelessWidget`, or `StatefulWidget` (or a `Future`, `Stream`, or
//! `Completer` of one). Returning a widget from a helper instead of defining a
//! real widget class defeats Flutter's element tree: the returned subtree cannot
//! hold its own state, is rebuilt wholesale whenever the enclosing `build` runs,
//! and never appears as a distinct node in the widget inspector. Extract the
//! subtree into its own `StatelessWidget` or `StatefulWidget` so Flutter can
//! cache and rebuild it independently. The `build` method and `@override`
//! members are exempt, as is any return type naming a concrete widget subclass
//! such as `Container`.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidReturningWidgets;

impl Rule for AvoidReturningWidgets {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(f) => {
                    if let Some(ret_type) = &f.return_type
                        && contains_widget_type(ret_type)
                    {
                        diags.push(ctx.diagnostic("avoid-returning-widgets", "Functions should not return Widget directly; extract to a separate widget class", f.span));
                    }
                }
                _ => {
                    for member in crate::lint::declarations::members_of(decl).unwrap_or_default() {
                        check_class_member(member, &mut diags, ctx);
                    }
                }
            }
        }

        RulePreparation::Diagnostics(diags)
    }
}

fn is_widget_type(dart_type: &DartType) -> bool {
    match dart_type {
        DartType::Named(nt) => {
            if let Some(last) = nt.segments.last() {
                let name = &last.name;
                // Only flag Widget, StatelessWidget, and StatefulWidget
                // Don't flag specific widget subclasses like Card, Container, etc.
                name == "Widget" || name == "StatelessWidget" || name == "StatefulWidget"
            } else {
                false
            }
        }
        _ => false,
    }
}

fn contains_widget_type(dart_type: &DartType) -> bool {
    if is_widget_type(dart_type) {
        return true;
    }

    // For Future<Widget>, Stream<Widget>, etc., check the wrapped type
    match dart_type {
        DartType::Named(nt) => {
            if let Some(last) = nt.segments.last() {
                let name = &last.name;
                // Only check wrapped type for specific async/wrapper types
                if (name == "Future" || name == "Stream" || name == "Completer")
                    && !nt.type_args.is_empty()
                {
                    // Check if the type argument is a Widget type
                    if let Some(first_arg) = nt.type_args.first() {
                        return is_widget_type(first_arg);
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn has_override_annotation(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|ann| {
        if let Some(first) = ann.name.first() {
            first.name == "override"
        } else {
            false
        }
    })
}

fn check_class_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Method(m) => {
            if let Some(ret_type) = &m.return_type
                && contains_widget_type(ret_type)
                && m.name.name != "build"
                && !has_override_annotation(&m.annotations)
            {
                diags.push(ctx.diagnostic(
                    "avoid-returning-widgets",
                    "Methods should not return Widget directly; extract to a separate widget class",
                    m.name.span,
                ));
            }
        }
        ClassMember::Getter(g) => {
            if let Some(ret_type) = &g.return_type
                && contains_widget_type(ret_type)
                && g.name.name != "build"
                && !has_override_annotation(&g.annotations)
            {
                diags.push(ctx.diagnostic(
                    "avoid-returning-widgets",
                    "Getters should not return Widget directly; extract to a separate widget class",
                    g.span,
                ));
            }
        }
        _ => {}
    }
}
