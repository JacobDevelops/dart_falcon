//! Flags parameters explicitly annotated with `dynamic`.
//!
//! An unannotated parameter is already `dynamic` (or inferred from an
//! override), so writing `dynamic` adds noise. This covers regular, field
//! (`this.x`), and super (`super.x`) formals, setter parameters, and the
//! parameters of function types.
//!
//! ```dart
//! void log(dynamic value) {} // flagged
//! void log(value) {}         // ok
//! ```
//!
//! Adopted from the official `avoid_annotating_with_dynamic` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_dart_type, walk_formal_param, walk_setter_decl};

const RULE: &str = "avoid-annotating-with-dynamic";
const MESSAGE: &str = "Unnecessary 'dynamic' type annotation.";

#[derive(Default)]
pub struct AvoidAnnotatingWithDynamic;

impl Rule for AvoidAnnotatingWithDynamic {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_formal_param(&mut self, node: &FormalParam) {
        // `dynamic f()` is a function-typed formal; `dynamic` is its return type.
        if node.function_params.is_none()
            && matches!(node.param_type, Some(DartType::Dynamic { .. }))
        {
            self.report(node.span);
        }
        walk_formal_param(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        if let Some(ty @ DartType::Dynamic { .. }) = &node.param_type {
            self.report(ty.span().merge(&node.param.span));
        }
        walk_setter_decl(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            for param in &function.params {
                // Legacy typedefs synthesize `dynamic` for untyped parameters;
                // only a written `dynamic` keyword counts.
                if let DartType::Dynamic { span } = &param.param_type
                    && self
                        .ctx
                        .source
                        .get(span.start..span.end)
                        .is_some_and(|text| text.trim_end() == "dynamic")
                {
                    let span = param
                        .name
                        .as_ref()
                        .map_or(*span, |name| span.merge(&name.span));
                    self.report(span);
                }
            }
        }
        walk_dart_type(self, node);
    }
}
