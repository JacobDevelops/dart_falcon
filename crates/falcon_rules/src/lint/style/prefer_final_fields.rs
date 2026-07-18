//! Flags a private field that is only ever initialized (at its declaration or
//! in constructors) and never reassigned, so it could be `final`
//! (`prefer-final-fields`, adopted from package:lints).
//!
//! Conservative. A whole-file scan collects every field name that is the target
//! of an assignment, compound assignment, or increment/decrement expression —
//! any such write disqualifies the name (matched by name across the file, so a
//! shared name in another class also protects it). A candidate is flagged only
//! when it is provably initialized through exactly one safe channel:
//!   * declaration initializer, and never touched by a constructor; or
//!   * no declaration initializer, but every non-redirecting generative
//!     constructor initializes it (via `: _x = ...` or a `this._x` formal).
//!
//! Fields marked `late`, `external`, or `abstract` are skipped as ambiguous.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};
use std::collections::HashSet;

pub struct PreferFinalFields;

impl Rule for PreferFinalFields {
    fn name(&self) -> &'static str {
        "prefer-final-fields"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut written = Written {
            names: HashSet::new(),
        };
        written.visit_program(program);

        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = members_of(decl) {
                check_class(members, &written.names, ctx, &mut diags);
            }
        }
        diags
    }
}

fn members_of(decl: &TopLevelDecl) -> Option<&[ClassMember]> {
    match decl {
        TopLevelDecl::Class(c) => Some(&c.members),
        TopLevelDecl::Mixin(m) => Some(&m.members),
        TopLevelDecl::MixinClass(mc) => Some(&mc.members),
        TopLevelDecl::Enum(e) => Some(&e.members),
        TopLevelDecl::ExtensionType(e) => Some(&e.members),
        _ => None,
    }
}

fn check_class(
    members: &[ClassMember],
    written: &HashSet<String>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    // Non-redirecting generative constructors: the ones a final field must be
    // initialized by if it has no declaration initializer.
    let generative: Vec<&ConstructorDecl> = members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Constructor(c) => Some(c),
            _ => None,
        })
        .filter(|c| !c.is_factory && !is_redirecting(c))
        .collect();

    for member in members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        if field.is_final
            || field.is_const
            || field.is_late
            || field.is_external
            || field.is_abstract
        {
            continue;
        }
        for d in &field.declarators {
            let name = d.name.name.as_str();
            if !name.starts_with('_') || written.contains(name) {
                continue;
            }
            let has_decl_init = d.initializer.is_some();
            let touched_by_ctor = generative.iter().any(|c| inits_field(c, name))
                || members.iter().any(|m| ctor_inits_any(m, name));

            let ok = if has_decl_init {
                !touched_by_ctor
            } else {
                !generative.is_empty() && generative.iter().all(|c| inits_field(c, name))
            };

            if ok {
                diags.push(Diagnostic::new(
                    "prefer-final-fields",
                    Severity::Warning,
                    format!("The private field '{name}' could be 'final'."),
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

fn is_redirecting(c: &ConstructorDecl) -> bool {
    c.initializers
        .iter()
        .any(|i| matches!(i, ConstructorInitializer::ThisCall { .. }))
}

fn ctor_inits_any(member: &ClassMember, name: &str) -> bool {
    matches!(member, ClassMember::Constructor(c) if inits_field(c, name))
}

fn inits_field(c: &ConstructorDecl, name: &str) -> bool {
    let via_list = c.initializers.iter().any(
        |i| matches!(i, ConstructorInitializer::FieldInit { field, .. } if field.name == name),
    );
    let via_formal = c
        .params
        .positional
        .iter()
        .chain(&c.params.optional_positional)
        .chain(&c.params.named)
        .any(|p| p.is_field && p.name.name == name);
    via_list || via_formal
}

// Collects every field/variable name that is written (assigned, compound
// assigned, or incremented) anywhere in the file.
struct Written {
    names: HashSet<String>,
}

impl Written {
    fn record_target(&mut self, target: &Expr) {
        match target {
            Expr::Ident(id) => {
                self.names.insert(id.name.clone());
            }
            Expr::Field { field, .. } => {
                self.names.insert(field.name.clone());
            }
            _ => {}
        }
    }
}

impl Visitor for Written {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Assign { target, .. } => self.record_target(target),
            Expr::PostfixIncDec { operand, .. } => self.record_target(operand),
            Expr::Unary {
                op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                operand,
                ..
            } => self.record_target(operand),
            Expr::Cascade { sections, .. } => {
                for s in sections {
                    for op in &s.ops {
                        if let CascadeOp::Assign(target, _, _) = op {
                            self.record_target(target);
                        }
                    }
                }
            }
            _ => {}
        }
        visitor::walk_expr(self, node);
    }
}
