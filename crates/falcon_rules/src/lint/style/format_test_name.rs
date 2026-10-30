//! Enforce configured regular expressions for test descriptions.
//!
//! `test-name-pattern` and `test-widgets-name-pattern` independently constrain
//! descriptions passed to the test framework's `test` and `testWidgets` functions.
//! Both default to absent, so the rule reports nothing until configured. Groups,
//! unrelated same-named functions, and descriptions that cannot be evaluated as
//! constant strings are ignored. Patterns use regular-expression search semantics;
//! add `^` and `$` to require a full-string match.
//! Diagnostics include the pattern that the description failed to match.
//!
//! See <https://dcm.dev/docs/rules/common/format-test-name/>.

use crate::lint::{options, support::test_assertions};
use falcon_analyze::{
    AnalyzeContext, ConstantContext, ConstantValue, Rule, RulePreparation, SemanticRuleVisitor,
    SemanticState, evaluate_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Regular expression required for `test` descriptions; absent means unrestricted.
    test_name_pattern: Option<options::Regex>,
    /// Regular expression required for `testWidgets` descriptions; absent means unrestricted.
    test_widgets_name_pattern: Option<options::Regex>,
}

options::impl_options!(Options);

/// Requires `test` and `testWidgets` descriptions to match their configured regex patterns.
///
/// Both patterns are optional and unrestricted by default. Constant-string descriptions
/// are checked only for resolved test-framework registrations; groups are excluded.
#[derive(Default)]
pub struct FormatTestName;

impl Rule for FormatTestName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let options = options::get::<Options>(ctx, Self::NAME);
        if (options.test_name_pattern.is_none() && options.test_widgets_name_pattern.is_none())
            || !test_assertions::enabled(program, ctx)
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            options,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    options: &'a Options,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Some((name, package)) = test_assertions::package_call(call, state) else {
            return;
        };
        if !test_assertions::is_framework(package) {
            return;
        }
        let pattern = match name {
            "test" => self.options.test_name_pattern.as_ref(),
            "testWidgets" => self.options.test_widgets_name_pattern.as_ref(),
            _ => None,
        };
        let (Some(pattern), Some(description)) = (pattern, call.args().positional().next()) else {
            return;
        };
        let Some(ConstantValue::String(value)) = evaluate_constant(
            description,
            ConstantContext {
                owner: state.constant_owner.as_ref(),
                locals: Some(&state.visible_local_constants()),
                ..ConstantContext::new(&state.model, &state.signatures)
            },
        ) else {
            return;
        };
        if !pattern.is_match(&value) {
            self.diagnostics.push(self.ctx.diagnostic(
                format!(
                    "The `{name}` description must match `{}`.",
                    pattern.as_str()
                ),
                description.span(),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::Options;

    #[test]
    fn patterns_are_optional_and_independent() {
        let defaults: Options = serde_json::from_str("{}").unwrap();
        assert!(defaults.test_name_pattern.is_none());
        assert!(defaults.test_widgets_name_pattern.is_none());

        let tests: Options = serde_json::from_str(r#"{"test-name-pattern":"^should "}"#).unwrap();
        assert!(tests.test_name_pattern.unwrap().is_match("should work"));
        assert!(tests.test_widgets_name_pattern.is_none());

        let widgets: Options =
            serde_json::from_str(r#"{"test-widgets-name-pattern":"^renders "}"#).unwrap();
        assert!(widgets.test_name_pattern.is_none());
        assert!(
            widgets
                .test_widgets_name_pattern
                .unwrap()
                .is_match("renders text")
        );
    }

    #[test]
    fn invalid_patterns_and_unknown_options_are_rejected() {
        assert!(serde_json::from_str::<Options>(r#"{"test-name-pattern":"["}"#).is_err());
        assert!(serde_json::from_str::<Options>(r#"{"test-widgets-name-pattern":"["}"#).is_err());
        assert!(serde_json::from_str::<Options>(r#"{"group-name-pattern":".*"}"#).is_err());
    }
}
