//! Match callback field names to a configured regular expression.
//!
//! `name_pattern` must be a nonempty valid regex; otherwise the rule is inactive.
//! Leading privacy underscores are ignored. Resolved function fields, including
//! aliases and Function, must match; proven non-function fields must not match.
//! Unknown, dynamic and enclosing type-parameter types are excluded, as are
//! inferred, static and annotated fields. Classes with supertypes are skipped
//! to preserve inherited field/getter contracts.

use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use regex::Regex;

const RULE: &str = "prefer-correct-callback-field-name";
#[derive(Default)]
pub struct PreferCorrectCallbackFieldName;

impl Rule for PreferCorrectCallbackFieldName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(pattern) = crate::lint::options::for_rule(ctx, RULE)
            .and_then(|o| o.get("name_pattern"))
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .and_then(|s| Regex::new(s).ok())
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
    pattern: Regex,
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
        let function = match &resolved {
            ResolvedType::Function { .. } => true,
            ResolvedType::Interface { .. } => resolved.interface("dart:core", "Function"),
            ResolvedType::Record { .. } | ResolvedType::Void | ResolvedType::Null | ResolvedType::Never => false,
            _ => return StructuralTraversal::Continue,
        };
        for declaration in node.declarators() {
            if self.pattern.is_match(declaration.name().name().trim_start_matches('_')) != function {
                self.diags.push(self.ctx.diagnostic(if function {
                        format!("This callback field does not match '{}'; rename it to match the configured pattern.", self.pattern)
                    } else {
                        format!("This non-function field matches callback pattern '{}'; rename it so it does not match.", self.pattern)
                    }, declaration.name().span()));
            }
        }
        StructuralTraversal::Continue
    }
}
}
