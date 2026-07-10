use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct ProperControllerDispose;

impl Rule for ProperControllerDispose {
    fn name(&self) -> &'static str {
        "proper_controller_dispose"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) if extends_state(&c.extends) => {
                    check_class(&c.members, &mut diags, ctx);
                }
                TopLevelDecl::MixinClass(mc) if extends_state(&mc.extends) => {
                    check_class(&mc.members, &mut diags, ctx);
                }
                _ => {}
            }
        }
        diags
    }
}

fn extends_state(extends: &Option<DartType>) -> bool {
    matches!(extends, Some(DartType::Named(nt))
        if nt.segments.last().is_some_and(|s| s.name.ends_with("State")))
}

fn check_class(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Controllers the class assigns to itself (never sourced from `widget.`).
    let mut assigned: HashSet<String> = HashSet::new();
    // Fields `.dispose()`-d inside the class's `dispose()` method.
    let mut disposed: HashSet<String> = HashSet::new();

    for member in members {
        if let Some(body) = member_body(member) {
            collect_assignments(body, &mut assigned);
        }
        if let ClassMember::Method(m) = member
            && m.name.name == "dispose"
            && let Some(body) = &m.body
        {
            collect_disposed(body, &mut disposed);
        }
    }

    for member in members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        for d in &field.declarators {
            let owned = match &d.initializer {
                Some(init) => constructs_controller(init) && !refs_widget(init),
                None => assigned.contains(&d.name.name),
            };
            if owned && !disposed.contains(&d.name.name) {
                diags.push(Diagnostic::new(
                    "proper_controller_dispose",
                    Severity::Warning,
                    "This controller is never disposed; dispose it in the State's dispose() method.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: d.name.span.start,
                        end: d.name.span.end,
                    },
                ));
            }
        }
    }
}

fn member_body(member: &ClassMember) -> Option<&FunctionBody> {
    match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    }
}

fn collect_assignments(body: &FunctionBody, out: &mut HashSet<String>) {
    struct Scan<'a>(&'a mut HashSet<String>);
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, node: &Expr) {
            if let Expr::Assign { target, value, .. } = node
                && let Some(name) = assign_target_name(target)
                && constructs_controller(value)
                && !refs_widget(value)
            {
                self.0.insert(name);
            }
            walk_expr(self, node);
        }
    }
    let mut scan = Scan(out);
    visit_body(&mut scan, body);
}

fn collect_disposed(body: &FunctionBody, out: &mut HashSet<String>) {
    struct Scan<'a>(&'a mut HashSet<String>);
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, node: &Expr) {
            match node {
                // `x.dispose()` / `x?.dispose()` / `this.x.dispose()`
                Expr::Call { callee, .. } => {
                    if let Expr::Field { object, field, .. } = callee.as_ref()
                        && field.name == "dispose"
                        && let Some(name) = receiver_name(object)
                    {
                        self.0.insert(name);
                    }
                }
                // Cascade `x..removeListener(..)..dispose()`
                Expr::Cascade {
                    object, sections, ..
                } => {
                    if sections
                        .iter()
                        .any(|s| matches!(&s.op, CascadeOp::Call(id, _, _) if id.name == "dispose"))
                        && let Some(name) = receiver_name(object)
                    {
                        self.0.insert(name);
                    }
                }
                _ => {}
            }
            walk_expr(self, node);
        }
    }
    let mut scan = Scan(out);
    visit_body(&mut scan, body);
}

fn visit_body<V: Visitor>(v: &mut V, body: &FunctionBody) {
    match body {
        FunctionBody::Block(b) => {
            for s in &b.stmts {
                v.visit_stmt(s);
            }
        }
        FunctionBody::Arrow(e, _) => v.visit_expr(e),
        FunctionBody::Native(_, _) => {}
    }
}

/// The name a controller is assigned to: `x = ...` or `this.x = ...`.
fn assign_target_name(target: &Expr) -> Option<String> {
    match target {
        Expr::Ident(id) => Some(id.name.clone()),
        Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::This { .. }) => {
            Some(field.name.clone())
        }
        _ => None,
    }
}

/// The field name a `.dispose()` is called on: `x.dispose()`, `x?.dispose()`,
/// or `this.x.dispose()`.
fn receiver_name(object: &Expr) -> Option<String> {
    match object {
        Expr::Ident(id) => Some(id.name.clone()),
        Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::This { .. }) => {
            Some(field.name.clone())
        }
        _ => None,
    }
}

/// True when `expr` constructs a value whose type name ends with `Controller`,
/// covering `TextEditingController()`, `AnimationController.unbounded()`, and the
/// explicit `new`/`const` forms.
fn constructs_controller(expr: &Expr) -> bool {
    match expr {
        Expr::New {
            dart_type: DartType::Named(nt),
            ..
        } => nt
            .segments
            .last()
            .is_some_and(|s| s.name.ends_with("Controller")),
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Ident(id) => id.name.ends_with("Controller"),
            Expr::Field { object, .. } => {
                matches!(object.as_ref(), Expr::Ident(id) if id.name.ends_with("Controller"))
            }
            _ => false,
        },
        _ => false,
    }
}

/// True when the expression references `widget` (e.g. `widget.controller`),
/// indicating the controller is owned by the parent widget, not this State.
fn refs_widget(expr: &Expr) -> bool {
    struct W(bool);
    impl Visitor for W {
        fn visit_expr(&mut self, node: &Expr) {
            if let Expr::Ident(id) = node
                && id.name == "widget"
            {
                self.0 = true;
            }
            walk_expr(self, node);
        }
    }
    let mut w = W(false);
    w.visit_expr(expr);
    w.0
}
