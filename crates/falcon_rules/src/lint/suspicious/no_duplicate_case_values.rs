//! Flags semantically equal constant values within one switch.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, Rule, SemanticModel, SignatureIndex,
    evaluate_constant,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_class_decl, walk_expr, walk_function_decl, walk_stmt};

pub struct NoDuplicateCaseValues;

impl Rule for NoDuplicateCaseValues {
    fn name(&self) -> &'static str {
        "no-duplicate-case-values"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            model: &model,
            signatures,
            owner: DeclarationIdentity::Project {
                library: usize::MAX,
                name: "<file>".to_string(),
            },
            fields: HashMap::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    owner: DeclarationIdentity,
    fields: HashMap<String, Expr>,
    file: String,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_> {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        let saved_owner = self.owner.clone();
        let saved_fields = std::mem::take(&mut self.fields);
        let Some(owner) = self
            .model
            .resolve_name(std::slice::from_ref(&node.name.name))
        else {
            walk_class_decl(self, node);
            self.fields = saved_fields;
            return;
        };
        self.owner = owner;
        self.fields = node
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field) if field.is_const => Some(field),
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
        if let Some(owner) = self
            .model
            .resolve_value(std::slice::from_ref(&node.name.name))
        {
            self.owner = owner;
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
                &self.owner,
                &field_refs,
                self.model,
                self.signatures,
            ) {
                if !seen.insert(value) {
                    self.diags.push(Diagnostic::new(
                        "no-duplicate-case-values",
                        Severity::Warning,
                        "Duplicate case value in switch statement.",
                        self.file.clone(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }
        }
    }
}

fn pattern_constants(
    pattern: &Pattern,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Vec<(ConstantValue, Span)> {
    match pattern {
        Pattern::Literal(literal) => literal_expression(literal)
            .and_then(|expression| evaluate_constant(&expression, owner, fields, model, signatures))
            .map(|value| vec![(value, literal.span.clone())])
            .unwrap_or_default(),
        Pattern::Const(constant) => {
            let expression = constant
                .expr
                .as_deref()
                .cloned()
                .or_else(|| name_expression(&constant.name, &constant.span));
            expression
                .and_then(|expression| {
                    evaluate_constant(&expression, owner, fields, model, signatures)
                })
                .map(|value| vec![(value, constant.span.clone())])
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
    let span = literal.span.clone();
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
                span: span.clone(),
            }),
            span,
        },
        LiteralPatternValue::NegDouble(value) => Expr::Unary {
            op: UnaryOp::Minus,
            operand: Box::new(Expr::DoubleLit {
                value: value.clone(),
                span: span.clone(),
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
            span: span.clone(),
        };
    }
    Some(expression)
}
