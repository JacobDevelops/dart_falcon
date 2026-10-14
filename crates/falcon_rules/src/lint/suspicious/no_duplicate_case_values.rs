//! Flags semantically equal constant values within one switch.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, Rule, SemanticModel, SignatureIndex,
    evaluate_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_class_decl, walk_expr, walk_function_decl, walk_stmt};

#[derive(Default)]
pub struct NoDuplicateCaseValues;

impl Rule for NoDuplicateCaseValues {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            model: &model,
            signatures,
            owner: None,
            fields: HashMap::new(),
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    owner: Option<DeclarationIdentity>,
    fields: HashMap<String, Expr>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_> {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        let saved_owner = self.owner.clone();
        let saved_fields = std::mem::take(&mut self.fields);
        let Some(owner) = self.model.resolve_name(&node.name.name) else {
            walk_class_decl(self, node);
            self.fields = saved_fields;
            return;
        };
        self.owner = Some(owner);
        self.fields = node
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field)
                    if field.keyword == falcon_syntax::ast::VarKeyword::Const =>
                {
                    Some(field)
                }
                _ => None,
            })
            .flat_map(|field| {
                field.declarators.iter().filter_map(|declarator| {
                    declarator
                        .initializer
                        .as_ref()
                        .map(|initializer| (declarator.name.name.clone(), initializer.clone()))
                })
            })
            .collect();
        walk_class_decl(self, node);
        self.owner = saved_owner;
        self.fields = saved_fields;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let saved = self.owner.clone();
        if let Some(owner) = self.model.resolve_value(&node.name.name) {
            self.owner = Some(owner);
        }
        walk_function_decl(self, node);
        self.owner = saved;
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Switch(switch) = node {
            let patterns = switch
                .cases
                .iter()
                .flat_map(|case| &case.cases)
                .filter_map(|case| match case {
                    SwitchCaseKind::Pattern(pattern, _) => Some(pattern.as_ref()),
                    SwitchCaseKind::Default => None,
                });
            self.check_patterns(patterns);
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Switch { arms, .. } = node {
            self.check_patterns(arms.iter().map(|arm| &arm.pattern));
        }
        walk_expr(self, node);
    }
}

impl Collector<'_> {
    fn check_patterns<'a>(&mut self, patterns: impl Iterator<Item = &'a Pattern>) {
        let field_refs = self
            .fields
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::<ConstantValue>::new();
        for pattern in patterns {
            for (value, span) in pattern_constants(
                pattern,
                self.owner.as_ref(),
                &field_refs,
                self.model,
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
    pattern: &Pattern,
    owner: Option<&DeclarationIdentity>,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Vec<(ConstantValue, Span)> {
    match pattern {
        Pattern::Literal(literal) => literal_expression(literal)
            .and_then(|expression| evaluate_constant(&expression, owner, fields, model, signatures))
            .map(|value| vec![(value, literal.span)])
            .unwrap_or_default(),
        Pattern::Const(constant) => {
            let expression = match &constant.value {
                ConstPatternValue::Expr(expression) => Some((**expression).clone()),
                ConstPatternValue::Name(name) => name_expression(name, &constant.span),
            };
            expression
                .and_then(|expression| {
                    evaluate_constant(&expression, owner, fields, model, signatures)
                })
                .map(|value| vec![(value, constant.span)])
                .unwrap_or_default()
        }
        Pattern::LogicalOr { left, right, .. } => {
            let mut values = pattern_constants(left, owner, fields, model, signatures);
            values.extend(pattern_constants(right, owner, fields, model, signatures));
            values
        }
        Pattern::ParenPattern { inner, .. } => {
            pattern_constants(inner, owner, fields, model, signatures)
        }
        _ => Vec::new(),
    }
}

fn literal_expression(literal: &LiteralPattern) -> Option<Expr> {
    let span = literal.span;
    Some(match &literal.value {
        LiteralPatternValue::Null => Expr::NullLit { span },
        LiteralPatternValue::Bool(value) => Expr::BoolLit {
            value: *value,
            span,
        },
        LiteralPatternValue::Int(value) => Expr::IntLit {
            value: value.clone(),
            span,
        },
        LiteralPatternValue::Double(value) => Expr::DoubleLit {
            value: value.clone(),
            span,
        },
        LiteralPatternValue::String(value) => Expr::StringLit(value.clone()),
        LiteralPatternValue::NegInt(value) => Expr::Unary {
            op: UnaryOp::Minus,
            operand: Box::new(Expr::IntLit {
                value: value.clone(),
                span,
            }),
            span,
        },
        LiteralPatternValue::NegDouble(value) => Expr::Unary {
            op: UnaryOp::Minus,
            operand: Box::new(Expr::DoubleLit {
                value: value.clone(),
                span,
            }),
            span,
        },
    })
}

fn name_expression(names: &[Identifier], span: &Span) -> Option<Expr> {
    let (first, rest) = names.split_first()?;
    let mut expression = Expr::Ident(first.clone());
    for name in rest {
        expression = Expr::Field {
            object: Box::new(expression),
            field: name.clone(),
            is_null_safe: false,
            span: *span,
        };
    }
    Some(expression)
}
