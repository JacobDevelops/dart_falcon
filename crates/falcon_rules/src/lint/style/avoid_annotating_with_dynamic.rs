//! Flags parameters explicitly annotated with `dynamic`.
//!
//! An unannotated parameter is already `dynamic` (or inferred from an
//! override), so writing `dynamic` adds noise. This covers regular, field
//! (`this.x`), and super (`super.x`) formals, function-typed formals whose
//! return type is `dynamic` (`dynamic f()`), setter parameters, and the
//! parameters of function types. `dynamic` must resolve through `dart:core`:
//! a file that imports `dart:core` only with a prefix sees `core.dynamic`,
//! not a bare `dynamic`.
//!
//! ```dart
//! void log(dynamic value) {} // flagged
//! void log(value) {}         // ok
//! ```
//!
//! Adopted from the official `avoid_annotating_with_dynamic` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_dart_type, walk_formal_param, walk_setter_decl};

use crate::lint::core_imports::CoreImports;

const RULE: &str = "avoid-annotating-with-dynamic";
const MESSAGE: &str = "Unnecessary 'dynamic' type annotation.";

#[derive(Default)]
pub struct AvoidAnnotatingWithDynamic;

impl Rule for AvoidAnnotatingWithDynamic {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            ctx,
            core: CoreImports::new(program),
            diags: Vec::new(),
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    core: CoreImports<'a>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_formal_param(&mut self, node: &FormalParam) {
        if node
            .param_type
            .as_ref()
            .is_some_and(|ty| self.core.is_dynamic(ty))
        {
            self.report(node.span);
        }
        walk_formal_param(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        if let Some(ty) = &node.param_type
            && self.core.is_dynamic(ty)
        {
            self.report(ty.span().merge(&node.param.span));
        }
        walk_setter_decl(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            for param in &function.params {
                let ty = &param.param_type;
                // Legacy typedefs (`typedef F(event);`) synthesize `dynamic` for
                // untyped parameters, spanning the whole parameter; a written
                // type always ends before the name.
                let synthesized = param
                    .name
                    .as_ref()
                    .is_some_and(|name| ty.span().end > name.span.start);
                if !synthesized && self.core.is_dynamic(ty) {
                    let span = param
                        .name
                        .as_ref()
                        .map_or(*ty.span(), |name| ty.span().merge(&name.span));
                    self.report(span);
                }
            }
        }
        walk_dart_type(self, node);
    }
}
