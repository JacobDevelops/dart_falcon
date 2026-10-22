//! Match callback field names to a configured regular expression.
//!
//! `name-pattern` must be a valid regex. Missing or empty patterns leave the rule inactive.
//! Leading privacy underscores are ignored. Resolved function fields, including
//! aliases and Function, must match; proven non-function fields must not match.
//! Unknown, dynamic and enclosing type-parameter types are excluded, as are
//! inferred, static and annotated fields. Classes with supertypes are skipped
//! to preserve inherited field/getter contracts.

use falcon_analyze::SemanticState;
use falcon_analyze::{
    AnalyzeContext, ResolvedTypeView, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferCorrectCallbackFieldName;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Regular expression matching callback field names; absent or empty patterns disable this rule.
    name_pattern: Option<crate::lint::options::Regex>,
}

crate::lint::options::impl_options!(Options);

impl Rule for PreferCorrectCallbackFieldName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(pattern) = crate::lint::options::get::<Options>(ctx, Self::NAME)
            .name_pattern
            .as_ref()
            .filter(|pattern| !pattern.as_str().is_empty())
        else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            state,
            pattern,
            diags: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    pattern: &'a crate::lint::options::Regex,
    diags: Vec<Diagnostic>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        match node {
            TopLevelDecl::Class(class) if class.extends().is_none() && class.with_clause().is_empty() && class.implements().is_empty() => {
                self.state.type_parameters.push(class.type_params(), &self.state.model);
                StructuralTraversal::Continue
            },
            _ => StructuralTraversal::SkipChildren,
        }
    }
    fn exit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::Class(class) = node
            && class.extends().is_none()
            && class.with_clause().is_empty()
            && class.implements().is_empty()
        {
            self.state.type_parameters.pop();
        }
    }
    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.is_static() || !node.annotations().is_empty() { return StructuralTraversal::Continue; }
        let Some(ty) = node.field_type() else { return StructuralTraversal::Continue; };
        let resolved = self.state.model.resolve_type_in(ty, &self.state.type_parameters);
        let function = match resolved.view() {
            ResolvedTypeView::Function { .. } => true,
            ResolvedTypeView::Interface { .. } => resolved.interface("dart:core", "Function"),
            ResolvedTypeView::Record { .. } | ResolvedTypeView::Void | ResolvedTypeView::Null | ResolvedTypeView::Never => false,
            _ => return StructuralTraversal::Continue,
        };
        for declaration in node.declarators() {
            if self.pattern.is_match(declaration.name().name().trim_start_matches('_')) != function {
                self.diags.push(self.ctx.diagnostic(if function {
                        format!("This callback field does not match `{}`; rename it to match the configured pattern.", self.pattern.as_str())
                    } else {
                        format!("This non-function field matches callback pattern `{}`; rename it so it does not match.", self.pattern.as_str())
                    }, declaration.name().span()));
            }
        }
        StructuralTraversal::Continue
    }
}
}
