//! Flags export directives whose visible names are already covered by another export.
//! URI values and conditional branches must match; show/hide chains are compared
//! as sets, so disjoint or partially overlapping visibility remains untouched.

use std::borrow::Cow;
use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst;

#[derive(Default)]
pub struct AvoidDuplicateExports;

impl Rule for AvoidDuplicateExports {
    fn prepare<'a>(
        &self,
        program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let exports: Vec<_> = program
            .exports()
            .map(|node| {
                (
                    node,
                    visibility(node.combinators()),
                    normalized_uri(node.uri().value()),
                )
            })
            .collect();
        let mut diags = Vec::new();
        for (index, (node, visible, uri)) in exports.iter().enumerate() {
            if exports
                .iter()
                .enumerate()
                .any(|(other_index, (other, coverage, other_uri))| {
                    index != other_index
                        && uri == other_uri
                        && node.configurable_uris().count() == other.configurable_uris().count()
                        && node
                            .configurable_uris()
                            .zip(other.configurable_uris())
                            .all(|(left, right)| same_syntax(left.syntax(), right.syntax()))
                        && node.annotations().count() == other.annotations().count()
                        && node
                            .annotations()
                            .zip(other.annotations())
                            .all(|(left, right)| same_syntax(left.syntax(), right.syntax()))
                        && visible.is_subset(coverage)
                        && (other_index < index || !coverage.is_subset(visible))
                })
            {
                diags.push(ctx.diagnostic("This export adds no names beyond another export of the same library — remove the redundant directive", node.span()));
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

pub(crate) enum Visibility<'a> {
    Only(HashSet<&'a str>),
    Except(HashSet<&'a str>),
}

impl Visibility<'_> {
    fn is_subset(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Only(left), Self::Only(right)) => left.is_subset(right),
            (Self::Only(left), Self::Except(right)) => left.is_disjoint(right),
            (Self::Except(left), Self::Except(right)) => right.is_subset(left),
            (Self::Except(_), Self::Only(_)) => false,
        }
    }
}

fn visibility<'a>(combinators: cst::AstChildren<'a, cst::ImportCombinator<'a>>) -> Visibility<'a> {
    let mut result = Visibility::Except(HashSet::new());
    for combinator in combinators {
        match combinator {
            cst::ImportCombinator::Show(show) => {
                let names: HashSet<_> = show.names().map(|name| name.name()).collect();
                result = Visibility::Only(match result {
                    Visibility::Only(previous) => previous.intersection(&names).copied().collect(),
                    Visibility::Except(hidden) => names.difference(&hidden).copied().collect(),
                });
            }
            cst::ImportCombinator::Hide(hide) => {
                for name in hide.names() {
                    match &mut result {
                        Visibility::Only(names) => {
                            names.remove(name.name());
                        }
                        Visibility::Except(names) => {
                            names.insert(name.name());
                        }
                    }
                }
            }
        }
    }
    result
}

/// Token comparison preserves boundaries and literal contents without allocating.
pub(crate) fn same_syntax(left: cst::SyntaxNode<'_>, right: cst::SyntaxNode<'_>) -> bool {
    left.significant_tokens()
        .map(|token| (token.kind(), token.text()))
        .eq(right
            .significant_tokens()
            .map(|token| (token.kind(), token.text())))
}

// Ordinary relative URI paths have lexical identity independent of `.`/`..`.
// Leave schemes, escaping and query/fragment syntax untouched rather than
// applying filesystem normalization to their different URI semantics.
fn normalized_uri(uri: Cow<'_, str>) -> Cow<'_, str> {
    if !uri.starts_with("//")
        && !uri.contains([':', '%', '?', '#'])
        && uri.split('/').any(|segment| matches!(segment, "." | ".."))
    {
        let absolute = uri.starts_with('/');
        let mut segments = Vec::with_capacity(uri.bytes().filter(|byte| *byte == b'/').count() + 1);
        for segment in uri.split('/') {
            match segment {
                "" | "." => {}
                ".." if segments.last().is_some_and(|previous| *previous != "..") => {
                    segments.pop();
                }
                ".." if absolute => {}
                other => segments.push(other),
            }
        }
        let mut normalized = String::with_capacity(uri.len());
        if absolute {
            normalized.push('/');
        }
        for (index, segment) in segments.into_iter().enumerate() {
            if index > 0 {
                normalized.push('/');
            }
            normalized.push_str(segment);
        }
        return Cow::Owned(normalized);
    }
    uri
}
