//! Flags constructor parameters that are never used.
//!
//! A constructor parameter that is neither read by an initializer nor by the
//! body is dead weight that every caller still has to supply. Initializing
//! formals (`this.x`) and super parameters (`super.x`) are always used, and
//! parameters annotated `@deprecated`/`@Deprecated(...)` or named only with
//! underscores are skipped, as are redirecting factory and `external`
//! constructors. A constructor with neither initializers nor a body reports
//! every other parameter.
//!
//! ```dart
//! class C {
//!   final int x;
//!   C(this.x, String name); // flagged: `name`
//! }
//! ```
//!
//! Adopted from the official `avoid_unused_constructor_parameters` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_constructor_decl};

use crate::lint::lexical_usage::used_constructor_parameters;

#[derive(Default)]
pub struct AvoidUnusedConstructorParameters;

impl Rule for AvoidUnusedConstructorParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        if node.redirect.is_none() && !node.is_external {
            let used =
                used_constructor_parameters(&node.params, &node.initializers, node.body.as_ref());
            for param in &node.params.params {
                let name = &param.name.name;
                if param.initializing.is_some()
                    || is_deprecated(param)
                    || name.bytes().all(|byte| byte == b'_')
                    || used.contains(&param.name.span.start)
                {
                    continue;
                }
                self.diagnostics.push(self.ctx.diagnostic(
                    "avoid-unused-constructor-parameters",
                    format!("The parameter '{name}' is not used in the constructor."),
                    param.span,
                ));
            }
        }
        walk_constructor_decl(self, node);
    }
}

fn is_deprecated(param: &FormalParam) -> bool {
    param.annotations.iter().any(|annotation| {
        matches!(
            annotation.name.last().map(|name| name.name.as_str()),
            Some("deprecated" | "Deprecated")
        )
    })
}
