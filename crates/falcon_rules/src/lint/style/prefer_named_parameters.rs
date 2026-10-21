//! Prefer named parameters when a declaration exceeds `max-number` (default 1).
//!
//! Checks top-level functions, constructors, and methods on classes without
//! inheritance. Overrides, operators, setters, closures, function types, `main`,
//! external/annotated declarations, redirects and super-parameter constructors
//! are excluded because their signatures can be imposed by another declaration.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferNamedParameters;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum number of positional parameters allowed before named parameters are preferred.
    #[serde(deserialize_with = "crate::lint::options::integer")]
    max_number: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_number: 1 }
    }
}

crate::lint::options::impl_options!(Options);

impl Rule for PreferNamedParameters {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            maximum: crate::lint::options::get::<Options>(ctx, Self::NAME).max_number,
            methods_allowed: false,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    maximum: usize,
    methods_allowed: bool,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, params: FormalParamList<'_>) {
        if let Some(param) = params.positional().nth(self.maximum) {
            self.diags.push(self.ctx.diagnostic(format!("This declaration has more than {} positional parameters; convert the excess parameters to named parameters.", self.maximum), param.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if matches!(node, TopLevelDecl::ExtensionType(_)) {
            return StructuralTraversal::SkipChildren;
        }
        self.methods_allowed = matches!(node, TopLevelDecl::Class(class)
            if class.extends().is_none() && class.with_clause().is_empty() && class.implements().is_empty());
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if !node.is_external() && node.annotations().is_empty() && node.name().name() != "main"
            && let FunctionKind::Function(params) = node.kind() { self.check(params); }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        if self.methods_allowed && !node.is_external() && node.annotations().is_empty() { self.check(node.params()); }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        if !node.is_external && node.annotations.is_empty() && node.redirect.is_none()
            && !node.initializers.clone().any(|initializer| matches!(initializer, ConstructorInitializer::ThisCall(_)))
            && !node.params.params().any(|param| param.initializing() == Some(InitializingFormal::Super)) {
            self.check(node.params);
        }
        StructuralTraversal::Continue
    }
}
}
