use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidReturningWidgets;

impl Rule for AvoidReturningWidgets {
    fn name(&self) -> &'static str {
        "avoid-returning-widgets"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(f) => {
                    if let Some(ret_type) = &f.return_type
                        && contains_widget_type(ret_type)
                    {
                        diags.push(Diagnostic::new(
                                "avoid-returning-widgets",
                                Severity::Warning,
                                "Functions should not return Widget directly; extract to a separate widget class",
                                ctx.file_path.to_string_lossy().into_owned(),
                                DiagSpan {
                                    start: f.span.start,
                                    end: f.span.end,
                                },
                            ));
                    }
                }
                TopLevelDecl::Class(c) => {
                    for member in &c.members {
                        check_class_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Mixin(m) => {
                    for member in &m.members {
                        check_class_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::MixinClass(mc) => {
                    for member in &mc.members {
                        check_class_member(member, &mut diags, ctx);
                    }
                }
                _ => {}
            }
        }

        diags
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
                diags.push(Diagnostic::new(
                    "avoid-returning-widgets",
                    Severity::Warning,
                    "Methods should not return Widget directly; extract to a separate widget class",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: m.name.span.start,
                        end: m.name.span.end,
                    },
                ));
            }
        }
        ClassMember::Getter(g) => {
            if let Some(ret_type) = &g.return_type
                && contains_widget_type(ret_type)
                && g.name.name != "build"
                && !has_override_annotation(&g.annotations)
            {
                diags.push(Diagnostic::new(
                    "avoid-returning-widgets",
                    Severity::Warning,
                    "Getters should not return Widget directly; extract to a separate widget class",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: g.span.start,
                        end: g.span.end,
                    },
                ));
            }
        }
        _ => {}
    }
}
