//! Prefer event-driven `Bloc` classes over `Cubit` subclasses.
//!
//! Reports classes whose superclass chain reaches the Bloc package's `Cubit`,
//! including indirect subclasses and unresolved-package import fallbacks, but
//! not event-driven `Bloc` classes or unrelated classes with the same name.

use crate::lint::bloc::BlocTypes;
use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Program, TopLevelDecl};

#[derive(Default)]
pub struct AvoidCubits;

impl Rule for AvoidCubits {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            types: BlocTypes::new(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    types: BlocTypes<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let (name, applicable) = match declaration {
            TopLevelDecl::Class(class) => (
                class.name(),
                self.types.is_cubit_class(class, &self.model, self.ctx.signatures()),
            ),
            TopLevelDecl::ClassTypeAlias(alias) => (
                alias.name(),
                self.types.is_api_syntax(alias.superclass(), &self.model, "Cubit")
                    || self.model.resolve_name(alias.name().name()).is_some_and(|identity| {
                        self.types.is_cubit_type(
                            &ResolvedType::interface_identity(identity),
                            &self.model,
                            self.ctx.signatures(),
                        )
                    }),
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        if applicable {
            self.diagnostics.push(self.ctx.diagnostic(
                "Prefer extending `Bloc` instead of `Cubit`.",
                name.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
