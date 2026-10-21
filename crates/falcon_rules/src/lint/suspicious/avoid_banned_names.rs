//! Ban declaration names, exempting references, initializing/super formals, and
//! `@override`-annotated members and their parameters.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use glob::{MatchOptions, Pattern};
use serde::{Deserialize, Deserializer};

#[derive(Default)]
pub struct AvoidBannedNames;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Banned declaration-name globs and the diagnostic message to report on a match.
    rules: Vec<Entry>,
}

crate::lint::options::impl_options!(Options);

#[derive(Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub(crate) struct Glob(Pattern);

/// A validated glob pattern used to match a banned name or API.
impl TryFrom<String> for Glob {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err("glob must not be empty".into());
        }
        Pattern::new(&value).map(Self).map_err(|error| error.to_string())
    }
}

impl From<Glob> for String {
    fn from(value: Glob) -> Self {
        value.0.as_str().to_owned()
    }
}

impl Glob {
    pub(crate) fn matches_with(&self, value: &str, options: MatchOptions) -> bool {
        self.0.matches_with(value, options)
    }
}

pub(crate) fn nonempty_patterns<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Glob>, D::Error> {
    let patterns = Vec::<Glob>::deserialize(deserializer)?;
    if patterns.is_empty() {
        return Err(serde::de::Error::custom("glob list must not be empty"));
    }
    Ok(patterns)
}

pub(crate) fn nonblank_message<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let message = String::deserialize(deserializer)?;
    if message.trim().is_empty() {
        return Err(serde::de::Error::custom("message must not be blank"));
    }
    Ok(message)
}

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Entry {
    /// Non-empty glob patterns (JSON key `deny`) that trigger this entry.
    #[serde(rename = "deny", deserialize_with = "nonempty_patterns")]
    #[schemars(length(min = 1))]
    patterns: Vec<Glob>,
    /// Diagnostic text emitted when any pattern in this entry matches.
    #[serde(deserialize_with = "nonblank_message")]
    message: String,
}
pub(super) fn report(
    ctx: &AnalyzeContext<'_>,
    entries: &[Entry],
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const MATCH: MatchOptions = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    for entry in entries {
        if entry
            .patterns
            .iter()
            .any(|pattern| pattern.matches_with(value, MATCH))
        {
            diagnostics.push(ctx.diagnostic(&entry.message, span));
        }
    }
}

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
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            entries,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: &'a [Entry],
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        if let Some(name) = node.constructor_name
            && !node.annotations.into_iter().any(|annotation| {
                annotation.name().any(|name| name.name() == "override")
            })
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
        if declaration.children().filter_map(Annotation::cast)
            .any(|annotation| annotation.name().any(|name| name.name() == "override"))
        {
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

    #[test]
    fn entries_preserve_patterns_and_messages() {
        let options: Options = serde_json::from_value(json!({
            "rules": [{"deny": ["Legacy*"], "message": "Use the supported API."}]
        })).unwrap();
        assert_eq!(options.rules[0].patterns[0].0.as_str(), "Legacy*");
        assert_eq!(options.rules[0].message, "Use the supported API.");
        assert!(Options::default().rules.is_empty());
    }
}
