//! Require switch statements over enum-like classes to cover every constant value.
//!
//! The official visitor removes canonical constant values, not field names:
//! top-level/static aliases, parenthesized constant patterns, and dot shorthands
//! cover the same value. Canonicalization retains collection runtime types and
//! applies constructor argument contexts (for example, `1` becomes `1.0` for a
//! `double` parameter); it must not globally equate integer and double values.

use std::collections::{HashMap, HashSet};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationFacts, DeclarationIdentity, ResolvedType, Rule,
    RulePreparation, SemanticModel, SignatureIndex, evaluate_constant,
    evaluate_constant_in_member_context, evaluate_constant_name_with_local_values,
    evaluate_constant_with_local_values,
};
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
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        RulePreparation::Semantic(Box::new(collector))
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
        let ResolvedType::Interface { identity, .. } = state.infer(node.subject()) else {
            return;
        };
        if state.signatures.type_kind(&identity) == Some(falcon_analyze::resolve::TypeKind::Enum) {
            return;
        }
        let Some(facts) = state.signatures.declaration(&identity) else {
            return;
        };
        if !is_enum_like(facts, &identity) {
            return;
        }
        let values = constant_values(facts, &identity, &state.model, &state.signatures);
        if values.len() < 2 {
            return;
        }
        let locals = state.visible_local_constants();
        let enclosing = match &state.enclosing_type {
            Some(ResolvedType::Interface { identity, .. }) => Some(identity),
            _ => None,
        };
        let mut covered = HashSet::new();
        for group in node.cases() {
            for case in group.cases() {
                let mut cover = |constant: CaseConstant<'_>| {
                    let value = match constant {
                        CaseConstant::Expr(expr) => evaluate_constant_with_local_values(
                            expr,
                            enclosing,
                            &locals,
                            &state.model,
                            &state.signatures,
                        ),
                        CaseConstant::Name(names) => evaluate_constant_name_with_local_values(
                            names,
                            enclosing,
                            &locals,
                            &state.model,
                            &state.signatures,
                        ),
                        // `.name` reads a static member of the switched type.
                        CaseConstant::Shorthand(name) => {
                            values.get(name.name()).cloned().or_else(|| {
                                facts
                                    .static_consts
                                    .iter()
                                    .find(|constant| constant.name == name.name())
                                    .and_then(|constant| {
                                        evaluate_constant_in_member_context(
                                            constant.initializer.expr(),
                                            &identity,
                                            &state.model,
                                            &state.signatures,
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
                        visit_case_constant(pattern.pattern(), &mut cover)
                    }
                    SwitchCaseKind::Expr(expression) => match expression.expr().unparenthesized() {
                        Expr::DotShorthand(shorthand) => {
                            cover(CaseConstant::Shorthand(shorthand.name()))
                        }
                        Expr::Ident(_) | Expr::Field(_) => {
                            cover(CaseConstant::Expr(expression.expr()))
                        }
                        _ => {}
                    },
                    SwitchCaseKind::Default(_) => {}
                }
            }
        }
        let mut groups: HashMap<&ConstantValue, usize> = HashMap::with_capacity(values.len());
        for (index, constant) in facts.static_consts.iter().enumerate() {
            if let Some(value) = values.get(&constant.name) {
                groups
                    .entry(value)
                    .and_modify(|preferred| {
                        if facts.static_consts[*preferred].is_deprecated && !constant.is_deprecated
                        {
                            *preferred = index;
                        }
                    })
                    .or_insert(index);
            }
        }
        // The analyzer reports from `switch` through the closing `)`.
        let end = self
            .ctx
            .tree()
            .tokens_in(
                self.ctx.source,
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
                "exhaustive-cases",
                format!("Add a case for '{}.{}'.", identity.name(), preferred.name),
                Span::new(node.span().start, end),
            ));
        }
    }
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

pub(crate) fn is_enum_like(facts: &DeclarationFacts, identity: &DeclarationIdentity) -> bool {
    !facts.is_abstract
        && !facts.has_subclass_in_library
        && !facts.constructors.is_empty()
        && facts
            .constructors
            .iter()
            .all(|constructor| constructor.is_private && !constructor.is_factory)
        && facts
            .static_consts
            .iter()
            .filter(|constant| exact_identity(&constant.ty, identity))
            .count()
            >= 2
}

fn exact_identity(ty: &ResolvedType, expected: &DeclarationIdentity) -> bool {
    matches!(ty, ResolvedType::Interface { identity, arguments, nullable: false, .. }
        if identity == expected && arguments.is_empty())
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
                Some(identity),
                &fields,
                model,
                signatures,
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
    use std::path::Path;
    use std::sync::Arc;

    use falcon_analyze::{IdentityIndex, IdentitySource, ParsedFile, RuleAnalysis, TypeIndex};
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
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
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
        let diagnostics = ExhaustiveCases.analyze(program, &ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "Add a case for 'Token.second'.");
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
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(program);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types);
        let diagnostics = ExhaustiveCases.analyze(program, &ctx);
        let messages: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert_eq!(
            messages,
            [
                "Add a case for 'Token.second'.",
                "Add a case for 'Token.third'.",
                "Add a case for 'Token.fourth'.",
            ]
        );
    }
}
