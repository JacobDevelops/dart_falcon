//! Reports confusingly similar declaration names at the same declaration level.
//!
//! Parameters and locals are compared across overlapping local scopes, class
//! members with members, and top-level declarations with top-level declarations.
//! Members and top-level names are not compared against parameters or locals.
//! Sibling blocks, functions, classes, loop scopes, and catch clauses remain
//! independent. Enclosing locals must precede nested declarations. If-case
//! bindings are confined to the condition and then branch, not the else branch.
//! References, identical names, function-type documentation parameters, and
//! conventional pairs differing only by a leading `_` or initial case are not
//! reported. Initializing and super formals refer to existing members.
//!
//! Similarity is case-sensitive normalized Levenshtein similarity over Unicode
//! characters: `1 - distance / max(lengths)`. A score at or above
//! `similarity-threshold` (default `0.8`) is reported on the later name, once,
//! using its closest counterpart. `show-similarity` includes the score in the
//! message. Exact `ignored-names` are excluded from both sides of comparisons.
//! Only declarations in the current file are considered; inherited and imported
//! names are not reconstructed by this structural rule.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

/// Reports confusingly similar declaration names at the same declaration level.
///
/// Uses case-sensitive normalized Levenshtein similarity, with an inclusive
/// default threshold of `0.8`. Parameters and local declarations are compared
/// across enclosing local scopes, but not against members or top-level names.
/// Enclosing locals must precede nested declarations. If-case bindings exclude
/// the else branch. Conventional pairs differing only by `_` or initial case are exempt.
/// Configure `ignored-names` to exclude either comparison side, or enable
/// `show-similarity` to include scores. Imported/inherited declarations and
/// function-type documentation parameters are not considered.
#[derive(Default)]
pub struct AvoidSimilarNames;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Minimum normalized edit similarity, inclusive, between zero and one.
    #[serde(deserialize_with = "deserialize_threshold")]
    #[schemars(range(min = 0.0, max = 1.0))]
    similarity_threshold: f64,
    /// Include the normalized similarity score in diagnostic messages.
    show_similarity: bool,
    /// Exact declaration names excluded from both sides of comparisons.
    ignored_names: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.8,
            show_similarity: false,
            ignored_names: Vec::new(),
        }
    }
}

fn deserialize_threshold<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    let value = <f64 as serde::Deserialize>::deserialize(deserializer)?;
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "similarity-threshold must be between zero and one",
        ))
    }
}

crate::lint::options::impl_options!(Options);

impl Rule for AvoidSimilarNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            options: crate::lint::options::get::<Options>(ctx, Self::NAME),
            names: Vec::new(),
        }))
    }
}

struct Name<'a> {
    identifier: Identifier<'a>,
    scope: SyntaxNode<'a>,
    length: usize,
    visibility_end: usize,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    options: &'a Options,
    names: Vec<Name<'a>>,
}

// Scope identity and declaration order come directly from CST ownership, not
// reference resolution. Closed child scopes must not leak into later locals.
fn is_scope(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Program
            | NodeKind::Block
            | NodeKind::ClassDecl
            | NodeKind::MixinDecl
            | NodeKind::EnumDecl
            | NodeKind::ExtensionDecl
            | NodeKind::ExtensionTypeDecl
            | NodeKind::MixinClassDecl
            | NodeKind::ClassTypeAliasDecl
            | NodeKind::FunctionDecl
            | NodeKind::MethodDecl
            | NodeKind::ConstructorDecl
            | NodeKind::PrimaryConstructor
            | NodeKind::PrimaryConstructorBody
            | NodeKind::GetterDecl
            | NodeKind::SetterDecl
            | NodeKind::OperatorDecl
            | NodeKind::LocalFuncDecl
            | NodeKind::FuncExpr
            | NodeKind::FunctionType
            | NodeKind::LegacyTypedef
            | NodeKind::TypeAliasDecl
            | NodeKind::FormalParam(_)
            | NodeKind::ForStmt
            | NodeKind::ForElement
            | NodeKind::MapForElement
            | NodeKind::IfStmt
            | NodeKind::IfElement
            | NodeKind::MapIfElement
            | NodeKind::WhileStmt
            | NodeKind::SwitchCase
            | NodeKind::SwitchExprArm
            | NodeKind::CatchClause
    )
}

fn visible_in(outer: &Name<'_>, inner: &Name<'_>) -> bool {
    inner
        .scope
        .ancestors()
        .any(|scope| scope.id() == outer.scope.id())
        && inner.identifier.span().start < outer.visibility_end
        && outer.identifier.span().start <= inner.identifier.span().start
}

fn overlaps(left: &Name<'_>, right: &Name<'_>) -> bool {
    left.scope.id() == right.scope.id() || visible_in(left, right) || visible_in(right, left)
}

fn distance_similarity(distance: usize, length: usize) -> f64 {
    // Identifier character counts and edit distances cannot exceed the u32
    // source-span bound.
    let distance = u32::try_from(distance).expect("edit distance fits the source-span bound");
    let length = u32::try_from(length).expect("identifier length fits the source-span bound");
    f64::from(length - distance) / f64::from(length)
}

fn maximum_distance(length: usize, threshold: f64) -> usize {
    let (mut low, mut high) = (0, length);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if distance_similarity(middle, length) >= threshold {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

fn bounded_distance(
    left: &[char],
    right: &[char],
    limit: usize,
    row: &mut Vec<usize>,
) -> Option<usize> {
    if left.len().abs_diff(right.len()) > limit {
        return None;
    }
    row.clear();
    row.extend((0..=right.len()).map(|distance| distance.min(limit + 1)));
    for (i, a) in left.iter().enumerate() {
        let start = (i + 1).saturating_sub(limit).max(1);
        let end = (i + 1 + limit).min(right.len());
        let mut diagonal = row[start - 1];
        row[start - 1] = if start == 1 { i + 1 } else { limit + 1 };
        let mut minimum = row[start - 1];
        for j in start..=end {
            let above = row[j];
            row[j] = (above + 1)
                .min(row[j - 1] + 1)
                .min(diagonal + usize::from(*a != right[j - 1]));
            diagonal = above;
            minimum = minimum.min(row[j]);
        }
        if end < right.len() {
            row[end + 1] = limit + 1;
        }
        if minimum > limit {
            return None;
        }
    }
    (row[right.len()] <= limit).then_some(row[right.len()])
}

fn member_scope(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Program
            | NodeKind::ClassDecl
            | NodeKind::MixinDecl
            | NodeKind::MixinClassDecl
            | NodeKind::ClassTypeAliasDecl
            | NodeKind::EnumDecl
            | NodeKind::ExtensionDecl
            | NodeKind::ExtensionTypeDecl
    )
}

fn comparison_group(mut scope: SyntaxNode<'_>) -> NodeId {
    if !member_scope(scope.kind()) {
        for ancestor in scope.ancestors().skip(1) {
            if member_scope(ancestor.kind()) {
                break;
            }
            scope = ancestor;
        }
    }
    scope.id()
}

fn conventional_pair(left: &str, right: &str) -> bool {
    let mut left = left.strip_prefix('_').unwrap_or(left).chars();
    let mut right = right.strip_prefix('_').unwrap_or(right).chars();
    match (left.next(), right.next()) {
        (Some(a), Some(b)) => a.to_lowercase().eq(b.to_lowercase()) && left.eq(right),
        _ => false,
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_name(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        // Setter bindings and type parameters use the non-reference callback.
        self.visit_identifier(node)
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        let Some(parent) = node.syntax().parent() else {
            return StructuralTraversal::Continue;
        };
        if node.is_missing() || node.name() == "_"
            || self.options.ignored_names.iter().any(|name| name == node.name()) {
            return StructuralTraversal::Continue;
        }
        if FormalParam::cast(parent).is_some_and(|parameter| parameter.initializing().is_some()) {
            return StructuralTraversal::Continue;
        }
        let declaration = match parent.kind() {
            NodeKind::CatchClause => true,
            NodeKind::LegacyTypedef => parent.parent().and_then(TypeAliasDecl::cast)
                .is_some_and(|alias| alias.name().span() == node.span()),
            NodeKind::ForInInit => ForInInit::cast(parent)
                .is_some_and(|init| init.declares_variable() && init.name().span() == node.span()),
            NodeKind::ClassDecl | NodeKind::MixinDecl | NodeKind::EnumDecl
            | NodeKind::ExtensionDecl | NodeKind::FunctionDecl | NodeKind::MethodDecl
            | NodeKind::GetterDecl | NodeKind::SetterDecl | NodeKind::VarDeclarator
            | NodeKind::FormalParam(_) | NodeKind::SetterParameter | NodeKind::TypeParam
            | NodeKind::TypeAliasDecl | NodeKind::LocalFuncDecl | NodeKind::VariablePattern
            | NodeKind::ClassTypeAliasDecl | NodeKind::MixinClassDecl
            | NodeKind::ExtensionTypeDecl | NodeKind::EnumVariant => parent.children()
                .find_map(Identifier::cast).is_some_and(|name| name.span() == node.span()),
            _ => false,
        };
        if !declaration {
            return StructuralTraversal::Continue;
        }
        // Names written in function types describe a signature, not bindings.
        let callable = parent.ancestors().skip(1).find(|ancestor| matches!(
            ancestor.kind(),
            NodeKind::FunctionType | NodeKind::LegacyTypedef | NodeKind::FormalParam(_)
                | NodeKind::SetterParameter | NodeKind::FunctionDecl | NodeKind::MethodDecl
                | NodeKind::ConstructorDecl | NodeKind::PrimaryConstructor
                | NodeKind::GetterDecl | NodeKind::SetterDecl | NodeKind::OperatorDecl
                | NodeKind::LocalFuncDecl | NodeKind::FuncExpr
        ));
        if callable.is_some_and(|ancestor| matches!(
            ancestor.kind(), NodeKind::FunctionType | NodeKind::LegacyTypedef
                | NodeKind::FormalParam(_) | NodeKind::SetterParameter
        )) {
            return StructuralTraversal::Continue;
        }
        if parent.kind() == NodeKind::VariablePattern
            && parent.ancestors().find(|ancestor| matches!(
                ancestor.kind(), NodeKind::PatternAssignStmt | NodeKind::PatternDeclaration
                    | NodeKind::CaseCondition | NodeKind::CasePatternLabel
                    | NodeKind::SwitchExprArm | NodeKind::PatternForInInit
            )).is_some_and(|ancestor| ancestor.kind() == NodeKind::PatternAssignStmt)
        {
            return StructuralTraversal::Continue;
        }
        let representation = callable
            .filter(|ancestor| ancestor.kind() == NodeKind::PrimaryConstructor)
            .and_then(SyntaxNode::parent)
            .filter(|ancestor| ancestor.kind() == NodeKind::ExtensionTypeDecl);
        let scope = if parent.kind() == NodeKind::CatchClause {
            Some(parent)
        } else if parent.kind() == NodeKind::LegacyTypedef {
            parent.parent().and_then(SyntaxNode::parent)
        } else {
            representation.or_else(|| parent.ancestors().skip(1).find(|ancestor| is_scope(ancestor.kind())))
        };
        if let Some(scope) = scope {
            let visibility_end = IfStmt::cast(scope).map(|statement| statement.then_branch().span().end)
                .or_else(|| IfElement::cast(scope).map(|element| element.then_elem().syntax().span().end))
                .or_else(|| MapIfElement::cast(scope).map(|element| element.then_entry().syntax().span().end))
                .unwrap_or(scope.span().end);
            self.names.push(Name {
                identifier: node, scope, length: node.name().chars().count(), visibility_end,
            });
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let capacity = self.names.iter().map(|name| name.length).max().unwrap_or(0) + 1;
        let mut row = Vec::with_capacity(capacity);
        let mut name_characters = Vec::with_capacity(capacity);
        let mut previous_characters = Vec::with_capacity(capacity);
        let mut groups: std::collections::HashMap<NodeId, Vec<&Name<'_>>> = std::collections::HashMap::new();
        for name in &self.names {
            name_characters.clear();
            name_characters.extend(name.identifier.name().chars());
            let previous_names = groups.entry(comparison_group(name.scope)).or_default();
            let mut closest = None;
            let mut best = self.options.similarity_threshold;
            for previous in previous_names.iter().rev().copied() {
                let length = previous.length.max(name.length);
                let minimum = previous.length.abs_diff(name.length).max(1);
                if closest.is_some() && distance_similarity(minimum, length) <= best {
                    continue;
                }
                let limit = maximum_distance(length, best);
                if previous.length.abs_diff(name.length) > limit
                    || conventional_pair(previous.identifier.name(), name.identifier.name())
                    || !overlaps(previous, name)
                {
                    continue;
                }
                previous_characters.clear();
                previous_characters.extend(previous.identifier.name().chars());
                let Some(distance) = bounded_distance(&previous_characters, &name_characters, limit, &mut row) else {
                    continue;
                };
                let score = distance_similarity(distance, length);
                if score > best || (closest.is_none() && score >= best) {
                    closest = Some(previous);
                    best = score;
                }
            }
            previous_names.push(name);
            if let Some(previous) = closest {
                let message = if self.options.show_similarity {
                    format!(
                        "The name `{}` is too similar to `{}` (similarity: {best:.2}).",
                        name.identifier.name(), previous.identifier.name(),
                    )
                } else {
                    format!(
                        "The name `{}` is too similar to `{}`.",
                        name.identifier.name(), previous.identifier.name(),
                    )
                };
                diagnostics.push(self.ctx.diagnostic(message, name.identifier.span()));
            }
        }
        diagnostics
    }
}
}

#[cfg(test)]
mod tests {
    use super::{
        Options, bounded_distance, conventional_pair, distance_similarity, maximum_distance,
    };
    use serde_json::json;

    #[test]
    fn options_preserve_documented_defaults() {
        let options: Options = serde_json::from_value(json!({})).unwrap();
        assert_eq!(options.similarity_threshold, 0.8);
        assert!(!options.show_similarity);
        assert_eq!(options.ignored_names, Vec::<String>::new());
    }

    #[test]
    fn similarity_threshold_is_bounded() {
        for value in [json!(-0.1), json!(1.1), json!("0.8"), json!(null)] {
            assert!(
                serde_json::from_value::<Options>(json!({"similarity-threshold": value})).is_err()
            );
        }
        for value in [0.0, 0.8, 1.0] {
            assert!(
                serde_json::from_value::<Options>(json!({"similarity-threshold": value})).is_ok()
            );
        }
    }

    #[test]
    fn inclusive_fraction_boundary_is_preserved() {
        assert_eq!(distance_similarity(11, 20), 0.45);
    }

    #[test]
    fn bounded_distance_preserves_all_in_threshold_pairs() {
        let words: Vec<Vec<char>> = (1_u32..32)
            .map(|bits| format!("{bits:b}").chars().collect())
            .collect();
        let mut row = Vec::new();
        for left in &words {
            for right in &words {
                let length = left.len().max(right.len());
                let mut full: Vec<_> = (0..=right.len()).collect();
                for (i, a) in left.iter().enumerate() {
                    let mut diagonal = full[0];
                    full[0] = i + 1;
                    for (j, b) in right.iter().enumerate() {
                        let above = full[j + 1];
                        full[j + 1] = (above + 1)
                            .min(full[j] + 1)
                            .min(diagonal + usize::from(a != b));
                        diagonal = above;
                    }
                }
                let distance = full[right.len()];
                for limit in 0..=length {
                    assert_eq!(
                        bounded_distance(left, right, limit, &mut row),
                        (distance <= limit).then_some(distance)
                    );
                }
                for threshold in [0.0, 0.45, 0.8, 0.95, 1.0] {
                    assert_eq!(
                        distance <= maximum_distance(length, threshold),
                        distance_similarity(distance, length) >= threshold
                    );
                }
            }
        }
    }

    #[test]
    fn conventional_pairs_do_not_hide_distinct_names() {
        assert!(conventional_pair("_count", "count"));
        assert!(conventional_pair("Customer", "customer"));
        assert!(!conventional_pair("customer", "customers"));
        assert!(!conventional_pair("customer", "custOmer"));
    }
}
