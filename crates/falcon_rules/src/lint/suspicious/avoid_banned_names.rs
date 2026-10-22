//! Ban declaration names, exempting references, initializing/super formals, and
//! `@override`-annotated members and their parameters.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;




#[derive(Default)]
pub struct AvoidBannedNames;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Banned declaration-name globs and the diagnostic message to report on a match.
    rules: Vec<Entry>,
}

crate::lint::options::impl_options!(Options);

use crate::lint::support::banned_entries::{Entry, report};

impl Rule for AvoidBannedNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = &crate::lint::options::get::<Options>(ctx, Self::NAME).rules;
        if entries.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            entries,
            model: falcon_analyze::SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: &'a [Entry],
    model: falcon_analyze::SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        if let Some(name) = node.constructor_name
            && !crate::lint::support::annotations::has_override(node.annotations, self.model)
        {
            report(self.ctx, self.entries, name.name(), name.span(), &mut self.diagnostics);
        }
        StructuralTraversal::Continue
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        let Some(parent) = node.syntax().parent() else {
            return StructuralTraversal::Continue;
        };
        if FormalParam::cast(parent).is_some_and(|parameter| parameter.initializing().is_some()) {
            return StructuralTraversal::Continue;
        }
        let additional_name = match parent.kind() {
            NodeKind::CatchClause | NodeKind::ImportDirective => true,
            NodeKind::ForInInit => ForInInit::cast(parent)
                .is_some_and(|init| init.declares_variable() && init.name().span() == node.span()),
            _ => false,
        };
        if !additional_name && (!matches!(parent.kind(),
            NodeKind::ClassDecl | NodeKind::MixinDecl | NodeKind::EnumDecl
            | NodeKind::ExtensionDecl | NodeKind::FunctionDecl | NodeKind::MethodDecl
            | NodeKind::GetterDecl | NodeKind::SetterDecl | NodeKind::VarDeclarator
            | NodeKind::FormalParam(_) | NodeKind::TypeParam | NodeKind::TypeAliasDecl
            | NodeKind::LocalFuncDecl | NodeKind::VariablePattern
            | NodeKind::ClassTypeAliasDecl | NodeKind::MixinClassDecl
            | NodeKind::ExtensionTypeDecl | NodeKind::EnumVariant
        ) || parent.children().find_map(Identifier::cast).is_none_or(|name| name.span() != node.span())) {
            return StructuralTraversal::Continue;
        }
        let declaration = if matches!(parent.kind(), NodeKind::FormalParam(_)) {
            parent.ancestors().skip(1).find(|ancestor| matches!(ancestor.kind(),
                NodeKind::MethodDecl | NodeKind::SetterDecl | NodeKind::FunctionDecl
                | NodeKind::ConstructorDecl | NodeKind::LocalFuncDecl | NodeKind::FuncExpr
            )).unwrap_or(parent)
        } else if parent.kind() == NodeKind::VarDeclarator {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        if crate::lint::support::annotations::has_override(
            declaration.children().filter_map(Annotation::cast), self.model,
        ) {
            return StructuralTraversal::Continue;
        }
        report(self.ctx, self.entries, node.name(), node.span(), &mut self.diagnostics);
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

#[cfg(test)]
mod tests {
    use super::Options;
    use serde_json::json;

    #[test]
    fn invalid_entries_are_rejected() {
        for entry in [
            json!({"deny": ["*"]}),
            json!({"deny": [], "message": "blocked"}),
            json!({"deny": [""], "message": "blocked"}),
            json!({"deny": ["["], "message": "blocked"}),
            json!({"deny": ["*", false], "message": "blocked"}),
            json!({"deny": ["*"], "message": " "}),
            json!({"deny": ["*"], "message": "blocked", "unknown": true}),
        ] {
            assert!(serde_json::from_value::<Options>(json!({"rules": [entry]})).is_err());
        }
    }

}
