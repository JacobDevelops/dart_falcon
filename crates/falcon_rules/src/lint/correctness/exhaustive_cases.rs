//! Require switch statements over enum-like classes to cover every constant value.
//!
//! The official visitor removes canonical constant values, not field names:
//! top-level/static aliases, parenthesized constant patterns, and dot shorthands
//! cover the same value. Canonicalization retains collection runtime types and
//! applies constructor argument contexts (for example, `1` becomes `1.0` for a
//! `double` parameter); it must not globally equate integer and double values.

use std::collections::{HashMap, HashSet};

use crate::lint::support::enum_like::{exact_identity, is_enum_like};
use falcon_analyze::{
    AnalyzeContext, ConstantContext, ConstantValue, DeclarationFacts, DeclarationIdentity,
    ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticModel, SignatureIndex,
    evaluate_constant, evaluate_constant_name,
};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_pattern};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ExhaustiveCases;

impl Rule for ExhaustiveCases {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check_switch<'a>(&mut self, node: SwitchStmt<'a>, state: &SemanticState<'a>) {
        if node.cases().any(|group| {
            group
                .cases()
                .any(|case| matches!(case, SwitchCaseKind::Default(_)))
        }) {
            return;
        }
        let subject_type = state.infer(node.subject());
        let ResolvedTypeView::Interface { identity, .. } = subject_type.view() else {
            return;
        };
        if state.signatures.type_kind(identity) == Some(falcon_analyze::TypeKind::Enum) {
            return;
        }
        let Some(facts) = state.signatures.declaration(identity) else {
            return;
        };
        if !is_enum_like(facts, identity) {
            return;
        }
        let values = constant_values(facts, identity, &state.model, &state.signatures);
        if values.len() < 2 {
            return;
        }
        let covered = covered_constants(node, state, facts, identity, &values);
        let mut groups = preferred_constants(facts, &values);
        // The analyzer reports from `switch` through the closing `)`.
        let end = self
            .ctx
            .tree()
            .tokens_in(
                self.ctx.source(),
                Span::new(node.subject().span().end, node.span().end),
            )
            .find(|token| !token.is_trivia())
            .filter(|token| token.kind() == falcon_syntax::TokenKind::RParen)
            .map_or(node.subject().span().end, |token| {
                token.offset() + token.len()
            });
        for constant in &facts.static_consts {
            let Some(value) = values.get(&constant.name) else {
                continue;
            };
            let Some(preferred) = groups.remove(value) else {
                continue;
            };
            if covered.contains(value) {
                continue;
            }
            let preferred = &facts.static_consts[preferred];
            self.diagnostics.push(self.ctx.diagnostic(
                format!("Add a case for `{}.{}`.", identity.name(), preferred.name),
                Span::new(node.span().start, end),
            ));
        }
    }
}

fn covered_constants<'a>(
    node: SwitchStmt<'a>,
    state: &SemanticState<'a>,
    facts: &DeclarationFacts,
    identity: &DeclarationIdentity,
    values: &HashMap<String, ConstantValue>,
) -> HashSet<ConstantValue> {
    let locals = state.visible_local_constants();
    let enclosing = match state.enclosing_type.as_ref().map(ResolvedType::view) {
        Some(ResolvedTypeView::Interface { identity, .. }) => Some(identity),
        _ => None,
    };
    let mut covered = HashSet::new();
    for group in node.cases() {
        for case in group.cases() {
            let mut cover = |constant: CaseConstant<'_>| {
                let value = match constant {
                    CaseConstant::Expr(expr) => evaluate_constant(
                        expr,
                        ConstantContext {
                            owner: enclosing,
                            locals: Some(&locals),
                            ..ConstantContext::new(&state.model, &state.signatures)
                        },
                    ),
                    CaseConstant::Name(names) => evaluate_constant_name(
                        names,
                        ConstantContext {
                            owner: enclosing,
                            locals: Some(&locals),
                            ..ConstantContext::new(&state.model, &state.signatures)
                        },
                    ),
                    // `.name` reads a static member of the switched type.
                    CaseConstant::Shorthand(name) => {
                        values.get(name.name()).cloned().or_else(|| {
                            facts
                                .static_consts
                                .iter()
                                .find(|constant| constant.name == name.name())
                                .and_then(|constant| {
                                    evaluate_constant(
                                        constant.initializer.expr(),
                                        ConstantContext {
                                            owner: Some(identity),
                                            ..ConstantContext::new(&state.model, &state.signatures)
                                        },
                                    )
                                })
                        })
                    }
                };
                if let Some(value) = value {
                    covered.insert(value);
                }
            };
            match case {
                SwitchCaseKind::Pattern(pattern) => {
                    visit_case_constant(pattern.pattern(), &mut cover);
                }
                SwitchCaseKind::Expr(expression) => match expression.expr().unparenthesized() {
                    Expr::DotShorthand(shorthand) => {
                        cover(CaseConstant::Shorthand(shorthand.name()));
                    }
                    Expr::Ident(_) | Expr::Field(_) => cover(CaseConstant::Expr(expression.expr())),
                    _ => {}
                },
                SwitchCaseKind::Default(_) => {}
            }
        }
    }
    covered
}

fn preferred_constants<'a>(
    facts: &DeclarationFacts,
    values: &'a HashMap<String, ConstantValue>,
) -> HashMap<&'a ConstantValue, usize> {
    let mut groups: HashMap<&ConstantValue, usize> = HashMap::with_capacity(values.len());
    for (index, constant) in facts.static_consts.iter().enumerate() {
        if let Some(value) = values.get(&constant.name) {
            groups
                .entry(value)
                .and_modify(|preferred| {
                    if facts.static_consts[*preferred].is_deprecated && !constant.is_deprecated {
                        *preferred = index;
                    }
                })
                .or_insert(index);
        }
    }
    groups
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Switch(switch) = node {
            self.check_switch(switch, state);
        }
    }
}

fn constant_values(
    facts: &DeclarationFacts,
    identity: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> HashMap<String, ConstantValue> {
    let fields = facts
        .static_consts
        .iter()
        .map(|constant| (constant.name.as_str(), constant.initializer.expr()))
        .collect::<HashMap<_, _>>();
    facts
        .static_consts
        .iter()
        .filter(|constant| exact_identity(&constant.ty, identity))
        .filter_map(|constant| {
            evaluate_constant(
                constant.initializer.expr(),
                ConstantContext {
                    owner: Some(identity),
                    fields: Some(&fields),
                    ..ConstantContext::new(model, signatures)
                },
            )
            .map(|value| (constant.name.clone(), value))
        })
        .collect()
}

/// The case's constant reference, as the linter accepts it: an identifier,
/// property access, or dot shorthand inside (parenthesized) constant patterns.
enum CaseConstant<'a> {
    Name(PatternNames<'a>),
    Expr(Expr<'a>),
    Shorthand(Identifier<'a>),
}

fn visit_case_constant<'a>(pattern: Pattern<'a>, visit: impl FnMut(CaseConstant<'a>)) {
    struct ConstantVisitor<F>(F);
    impl<'a, F: FnMut(CaseConstant<'a>)> Visitor<'a> for ConstantVisitor<F> {
        fn visit_pattern(&mut self, pattern: Pattern<'a>) {
            if matches!(pattern, Pattern::ParenPattern(_)) {
                // Only parentheses are transparent; shared traversal must not
                // turn nested logical, object, or collection patterns into cases.
                walk_pattern(self, pattern);
            } else if let Some(constant) = direct_case_constant(pattern) {
                (self.0)(constant);
            }
        }
    }
    ConstantVisitor(visit).visit_pattern(pattern);
}

fn direct_case_constant(pattern: Pattern<'_>) -> Option<CaseConstant<'_>> {
    match pattern {
        Pattern::Const(constant) => match constant.value() {
            ConstPatternValue::Name(names) if names.clone().next().is_some() => {
                Some(CaseConstant::Name(names))
            }
            ConstPatternValue::Name(_) => None,
            ConstPatternValue::Expr(expr) => match expr.unparenthesized() {
                Expr::DotShorthand(shorthand) => Some(CaseConstant::Shorthand(shorthand.name())),
                Expr::Ident(_) | Expr::Field(_) => Some(CaseConstant::Expr(expr)),
                _ => None,
            },
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use falcon_analyze::RuleAnalysis;
    use std::path::Path;
    use std::sync::Arc;

    use falcon_analyze::{ParsedFile, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn single_file_fallback_preserves_owner_constant_dependencies() {
        let source = include_str!("../../../tests/corpus/exhaustive-cases/owner-dependencies.dart");
        let source = source.replace(
            "switch (token)",
            "switch ( /* before */ (token) /* after */ )",
        );
        let source = source.as_str();
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let path = Path::new("owner-dependencies.dart");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let identities = crate::lint::test_support::identities(&[(path, &parsed)]);
        let types = TypeIndex::from_resolved_files(
            [(
                path,
                falcon_analyze::LibrarySource {
                    program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types);
        let diagnostics = ExhaustiveCases.analyze(ExhaustiveCases::NAME, program, &ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "Add a case for `Token.second`.");
        let span = &diagnostics[0].span;
        assert_eq!(
            &source[span.start..span.end],
            "switch ( /* before */ (token) /* after */ )"
        );
    }

    #[test]
    fn missing_cases_follow_declaration_order_with_preferred_aliases() {
        let source =
            include_str!("../../../tests/corpus/exhaustive-cases/review_declaration_order.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let path = Path::new("review_declaration_order.dart");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let identities = crate::lint::test_support::identities(&[(path, &parsed)]);
        let types = TypeIndex::from_program(program);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types);
        let diagnostics = ExhaustiveCases.analyze(ExhaustiveCases::NAME, program, &ctx);
        let messages: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert_eq!(
            messages,
            [
                "Add a case for `Token.second`.",
                "Add a case for `Token.third`.",
                "Add a case for `Token.fourth`.",
            ]
        );
    }
}
