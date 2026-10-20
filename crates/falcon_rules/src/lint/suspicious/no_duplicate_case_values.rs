//! Flags semantically equal constant values within one switch.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, evaluate_constant,
    evaluate_pattern_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoDuplicateCaseValues;

impl Rule for NoDuplicateCaseValues {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let collector = Collector {
            model,
            signatures,
            owner: None,
            fields: HashMap::new(),
            ctx,
            diags: Vec::new(),
            class_context: Vec::new(),
            function_context: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    owner: Option<DeclarationIdentity>,
    fields: HashMap<&'a str, Expr<'a>>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    class_context: Vec<(Option<DeclarationIdentity>, HashMap<&'a str, Expr<'a>>)>,
    function_context: Vec<Option<DeclarationIdentity>>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_class_decl(&mut self, node: ClassDecl<'a>) -> StructuralTraversal {
        self.class_context.push((self.owner.clone(), std::mem::take(&mut self.fields)));
        let Some(owner) = self.model.resolve_name(node.name().name()) else {
            return StructuralTraversal::Continue;
        };
        self.owner = Some(owner);
        self.fields = node
            .members()
            .filter_map(|member| match member {
                ClassMember::Field(field)
                    if field.keyword() == VarKeyword::Const =>
                {
                    Some(field)
                }
                _ => None,
            })
            .flat_map(|field| {
                field.declarators().filter_map(|declarator| {
                    declarator
                        .initializer()
                        .map(|initializer| (declarator.name().name(), initializer))
                })
            })
            .collect();
        StructuralTraversal::Continue
    }

    fn exit_class_decl(&mut self, _node: ClassDecl<'a>) {
        let (owner, fields) = self.class_context.pop().unwrap();
        self.owner = owner;
        self.fields = fields;
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.function_context.push(self.owner.clone());
        if let Some(owner) = self.model.resolve_value(node.name().name()) {
            self.owner = Some(owner);
        }
        StructuralTraversal::Continue
    }

    fn exit_function_decl(&mut self, _node: FunctionDecl<'a>) {
        self.owner = self.function_context.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Switch(switch) = node {
            let patterns = switch
                .cases()
                .flat_map(|case| case.cases())
                .filter_map(|case| match case {
                    SwitchCaseKind::Pattern(pattern) => Some(pattern.pattern()),
                    SwitchCaseKind::Expr(_) | SwitchCaseKind::Default(_) => None,
                });
            self.check_patterns(patterns);
            self.check_expressions(switch.cases().flat_map(|case| case.cases()).filter_map(
                |case| match case {
                    SwitchCaseKind::Expr(expression) => Some(expression.expr()),
                    _ => None,
                },
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Switch(switch) = node {
            self.check_patterns(switch.arms().map(|arm| arm.pattern()));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

impl<'a> Collector<'a> {
    fn check_expressions(&mut self, expressions: impl Iterator<Item = Expr<'a>>) {
        let mut expressions = expressions.peekable();
        if expressions.peek().is_none() {
            return;
        }
        let mut seen = HashSet::new();
        for expression in expressions {
            if let Some(value) = evaluate_constant(
                expression,
                self.owner.as_ref(),
                &self.fields,
                &self.model,
                self.signatures,
            ) && !seen.insert(value)
            {
                self.diags.push(self.ctx.diagnostic(
                    "no-duplicate-case-values",
                    "Duplicate case value in switch statement.",
                    expression.span(),
                ));
            }
        }
    }

    fn check_patterns(&mut self, patterns: impl Iterator<Item = Pattern<'a>>) {
        let mut seen = HashSet::<ConstantValue>::new();
        for pattern in patterns {
            for (value, span) in pattern_constants(
                pattern,
                self.owner.as_ref(),
                &self.fields,
                &self.model,
                self.signatures,
            ) {
                if !seen.insert(value) {
                    self.diags.push(self.ctx.diagnostic(
                        "no-duplicate-case-values",
                        "Duplicate case value in switch statement.",
                        span,
                    ));
                }
            }
        }
    }
}

fn pattern_constants(
    pattern: Pattern<'_>,
    owner: Option<&DeclarationIdentity>,
    fields: &HashMap<&str, Expr<'_>>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Vec<(ConstantValue, Span)> {
    match pattern {
        Pattern::Literal(_) | Pattern::Const(_) => {
            evaluate_pattern_constant(pattern, owner, fields, model, signatures)
                .map(|value| vec![(value, pattern.span())])
                .unwrap_or_default()
        }
        Pattern::LogicalOr(logical) => {
            let mut values = pattern_constants(logical.left(), owner, fields, model, signatures);
            values.extend(pattern_constants(
                logical.right(),
                owner,
                fields,
                model,
                signatures,
            ));
            values
        }
        Pattern::ParenPattern(paren) => {
            pattern_constants(paren.inner(), owner, fields, model, signatures)
        }
        _ => Vec::new(),
    }
}
