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
use falcon_syntax::cst;

use falcon_analyze::resolve::lexical_usage::{
    used_constructor_parameters, used_primary_constructor_parameters,
};

#[derive(Default)]
pub struct AvoidUnusedConstructorParameters;

impl Rule for AvoidUnusedConstructorParameters {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: falcon_analyze::SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    model: falcon_analyze::SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn report(
        &mut self,
        params: cst::FormalParamList<'_>,
        used: &std::collections::HashSet<usize>,
        primary: bool,
    ) {
        for param in params.params() {
            let name = param.name().name();
            if !is_candidate(param, primary, self.model) || used.contains(&param.name().span().start) {
                continue;
            }
            self.diagnostics.push(self.ctx.diagnostic(format!("The parameter `{name}` is not used in the constructor."), param.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_top_level_decl(&mut self, node: cst::TopLevelDecl<'_>) -> StructuralTraversal {
        let members = match node {
            cst::TopLevelDecl::Class(node) => Some(node.element_members()),
            cst::TopLevelDecl::MixinClass(node) => Some(node.element_members()),
            cst::TopLevelDecl::Enum(node) => Some(node.element_members()),
            _ => None,
        };
        // Primary parameters are represented separately from ordinary
        // constructors and their use includes eager instance field initializers.
        if let Some(members) = members
            && let Some(primary) = members.primary
            && primary.params().params().any(|param| is_candidate(param, true, self.model))
        {
            let used = used_primary_constructor_parameters(primary, members.declared.clone());
            self.report(primary.params(), &used, true);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: cst::ConstructorRef<'_>) -> StructuralTraversal {
        if !node.is_primary
            && node.redirect.is_none()
            && !node.is_external
            && node.params.params().any(|param| is_candidate(param, false, self.model))
        {
            let used = used_constructor_parameters(node.params, node.initializers.clone(), node.body);
            self.report(node.params, &used, false);
        }
        StructuralTraversal::Continue
    }
}
}

fn is_candidate(param: cst::FormalParam<'_>, primary: bool, model: falcon_analyze::SemanticModel<'_>) -> bool {
    param.initializing().is_none()
        && !(primary && (param.is_final() || param.is_var()))
        && !crate::lint::support::annotations::has_deprecated(param.annotations(), model)
        && !param.name().name().bytes().all(|byte| byte == b'_')
}

