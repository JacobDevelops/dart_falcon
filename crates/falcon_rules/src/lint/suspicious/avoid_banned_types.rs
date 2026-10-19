//! Ban resolved explicit type annotations, including nested generic arguments.
//! Aliases are expanded; import prefixes and nullability do not change matching.
//! Unknown types are not reported. Match against the declaration's simple name.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_banned_names::{Entry, entries, report};

#[derive(Default)]
pub struct AvoidBannedTypes;

impl Rule for AvoidBannedTypes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = entries(ctx, "avoid-banned-types");
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        if entries.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            entries,
            model: SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types),
            diagnostics: Vec::new(),
            ancestors: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: Vec<Entry<'a>>,
    model: SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
    ancestors: Vec<SyntaxNode<'a>>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        self.ancestors.clear();
        let mut parent = node.syntax().parent();
        while let Some(ancestor) = parent {
            if let Some(construction) = NewExpr::cast(ancestor)
                && construction.dart_type().span() == node.span()
            {
                return StructuralTraversal::Continue;
            }
            self.ancestors.push(ancestor);
            parent = ancestor.parent();
        }
        let mut parameters = TypeParameterScope::default();
        for ancestor in self.ancestors.iter().rev().copied() {
            let params = ancestor.children().filter_map(TypeParam::cast);
            if params.clone().next().is_some() {
                parameters.push(params, &self.model);
            }
        }
        let resolved = self.model.resolve_type_in(node, &parameters);
        let name = match &resolved {
            ResolvedType::Interface { identity, .. } => identity.name(),
            ResolvedType::Dynamic => "dynamic",
            ResolvedType::Void => "void",
            _ => return StructuralTraversal::Continue,
        };
        report(self.ctx, &self.entries, "avoid-banned-types", name, node.span(), &mut self.diagnostics);
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
