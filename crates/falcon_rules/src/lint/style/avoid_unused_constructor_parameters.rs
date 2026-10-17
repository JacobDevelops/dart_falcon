//! Flags constructor parameters that are never used.
//!
//! A constructor parameter that is neither read by an initializer nor by the
//! body is dead weight that every caller still has to supply. Initializing
//! formals (`this.x`) and super parameters (`super.x`) are always used, and
//! parameters annotated `@deprecated`/`@Deprecated(...)` or named only with
//! underscores are skipped, as are redirecting factory and `external`
//! constructors. Primary header parameters are also read by eager instance
//! field initializers; declaring primary parameters implicitly initialize fields.
//! A constructor with neither initializers nor a body reports every other parameter.
//!
//! ```dart
//! class C {
//!   final int x;
//!   C(this.x, String name); // flagged: `name`
//! }
//! ```
//!
//! Adopted from the official `avoid_unused_constructor_parameters` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use falcon_analyze::resolve::lexical_usage::{
    used_constructor_parameters, used_primary_constructor_parameters,
};

#[derive(Default)]
pub struct AvoidUnusedConstructorParameters;

impl Rule for AvoidUnusedConstructorParameters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diagnostics: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn report(
        &mut self,
        params: &FormalParamList,
        used: &std::collections::HashSet<usize>,
        primary: bool,
    ) {
        for param in &params.params {
            let name = &param.name.name;
            if !is_candidate(param, primary) || used.contains(&param.name.span.start) {
                continue;
            }
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-unused-constructor-parameters",
                format!("The parameter '{name}' is not used in the constructor."),
                param.span,
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        let members = match node {
            TopLevelDecl::Class(node) => Some(node.element_members()),
            TopLevelDecl::MixinClass(node) => Some(node.element_members()),
            TopLevelDecl::Enum(node) => Some(node.element_members()),
            // The representation parameter always initializes its implicit field.
            _ => None,
        };
        if let Some(members) = members
            && let Some(primary) = members.primary
            && primary
                .params
                .params
                .iter()
                .any(|param| is_candidate(param, true))
        {
            let used = used_primary_constructor_parameters(primary, members.declared);
            self.report(&primary.params, &used, true);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        // Primary parameters need the enclosing field-initializer scope above.
        if !node.is_primary
            && node.redirect.is_none()
            && !node.is_external
            && node
                .params
                .params
                .iter()
                .any(|param| is_candidate(param, false))
        {
            let used = used_constructor_parameters(node.params, node.initializers, node.body);
            self.report(node.params, &used, false);
        }
        StructuralTraversal::Continue
    }
}
}

fn is_candidate(param: &FormalParam, primary: bool) -> bool {
    param.initializing.is_none()
        && !(primary && (param.is_final || param.is_var))
        && !is_deprecated(param)
        && !param.name.name.bytes().all(|byte| byte == b'_')
}

fn is_deprecated(param: &FormalParam) -> bool {
    param.annotations.iter().any(|annotation| {
        matches!(
            annotation.name.last().map(|name| name.name.as_str()),
            Some("deprecated" | "Deprecated")
        )
    })
}
