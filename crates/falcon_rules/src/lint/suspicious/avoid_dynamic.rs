//! Flags explicit `dynamic` type annotations.
//!
//! `dynamic` opts an expression out of static type checking, so member accesses
//! and calls on it are resolved at runtime and any mistake surfaces as a crash
//! rather than a compile error. Reaching for it is usually a shortcut where a
//! concrete type, a generic, or `Object?` plus explicit checks would be safer.
//! Prefer a precise type. `dynamic` used directly as a `Map` type argument — the
//! documented JSON escape hatch, `Map<String, dynamic>` — is exempt, but a
//! `dynamic` nested more deeply (such as `Map<String, List<dynamic>>`) is still
//! flagged.
//! All explicit type positions are checked, including record fields, type
//! aliases, generic bounds, patterns, and types inside initializers.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_dart_type, walk_expr, walk_map_element};

#[derive(Default)]
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut visitor = DynamicVisitor {
            diagnostics: Vec::new(),
            ctx,
        };
        visitor.visit_program(program);
        visitor.diagnostics
    }
}

struct DynamicVisitor<'a, 'ctx> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for DynamicVisitor<'_, '_> {
    fn visit_dart_type(&mut self, node: &DartType) {
        match node {
            DartType::Dynamic { span } => self.diagnostics.push(self.ctx.diagnostic(
                "avoid-dynamic",
                "Avoid using the 'dynamic' type",
                *span,
            )),
            DartType::Named(named)
                if named
                    .segments
                    .last()
                    .is_some_and(|segment| segment.name == "Map") =>
            {
                for argument in &named.type_args {
                    if !matches!(argument, DartType::Dynamic { .. }) {
                        self.visit_dart_type(argument);
                    }
                }
            }
            _ => walk_dart_type(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Map {
            type_args,
            elements,
            ..
        } = node
        {
            for argument in type_args {
                if !matches!(argument, DartType::Dynamic { .. }) {
                    self.visit_dart_type(argument);
                }
            }
            for element in elements {
                walk_map_element(self, element);
            }
        } else {
            walk_expr(self, node);
        }
    }
}
