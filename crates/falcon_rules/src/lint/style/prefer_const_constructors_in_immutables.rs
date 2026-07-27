//! Prefers `const` constructors in immutable classes.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, Rule, SemanticModel, SignatureIndex, TypeIndex,
    evaluate_constant,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferConstConstructorsInImmutables;

impl Rule for PreferConstConstructorsInImmutables {
    fn name(&self) -> &'static str {
        "prefer-const-constructors-in-immutables"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let TopLevelDecl::Class(class) = decl else {
                continue;
            };
            let Some(owner) = model.resolve_name(std::slice::from_ref(&class.name.name)) else {
                continue;
            };
            let immutable = ctx
                .types
                .and_then(|types| types.is_immutable_type(&class.name.name))
                .unwrap_or_else(|| is_immutable(&class.annotations, program));
            if !immutable || !class_fields_allow_const(class, &owner, &model, signatures) {
                continue;
            }
            for member in &class.members {
                let ClassMember::Constructor(constructor) = member else {
                    continue;
                };
                if constructor.is_const
                    || constructor.is_external
                    || !constructor_can_be_const(
                        class,
                        constructor,
                        ctx.types,
                        &owner,
                        &model,
                        signatures,
                    )
                {
                    continue;
                }
                diags.push(Diagnostic::new(
                    "prefer-const-constructors-in-immutables",
                    Severity::Warning,
                    "Declare a const constructor in an immutable class.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: constructor.span.start,
                        end: constructor.span.end,
                    },
                ));
            }
        }
        diags
    }
}

fn is_immutable(annotations: &[Annotation], program: &Program) -> bool {
    annotations.iter().any(|annotation| {
        let Some(last) = annotation.name.last() else {
            return false;
        };
        if last.name != "immutable" {
            return false;
        }
        let prefix = (annotation.name.len() > 1).then(|| annotation.name[0].name.as_str());
        program.imports.iter().any(|import| {
            matches!(
                import.uri.value.as_str(),
                "package:meta/meta.dart"
                    | "package:flutter/foundation.dart"
                    | "package:flutter/widgets.dart"
                    | "package:flutter/material.dart"
            ) && import.as_name.as_ref().map(|name| name.name.as_str()) == prefix
        })
    })
}

fn class_lexical_value_names(class: &ClassDecl) -> HashSet<String> {
    let mut names = class
        .type_params
        .iter()
        .map(|parameter| parameter.name.name.clone())
        .collect::<HashSet<_>>();
    for member in &class.members {
        match member {
            ClassMember::Field(field) => {
                names.extend(
                    field
                        .declarators
                        .iter()
                        .map(|declarator| declarator.name.name.clone()),
                );
            }
            ClassMember::Method(method) => {
                names.insert(method.name.name.clone());
            }
            ClassMember::Getter(getter) => {
                names.insert(getter.name.name.clone());
            }
            ClassMember::Setter(setter) => {
                names.insert(setter.name.name.clone());
            }
            ClassMember::Constructor(_) | ClassMember::Operator(_) | ClassMember::Error(_) => {}
        }
    }
    names
}

fn class_fields_allow_const(
    class: &ClassDecl,
    owner: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let shadowed = class_lexical_value_names(class);
    let fields = class
        .members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Field(field) if !field.is_static && (field.is_final || field.is_const) => {
                Some(field)
            }
            _ => None,
        })
        .flat_map(|field| {
            field.declarators.iter().filter_map(|declarator| {
                declarator
                    .initializer
                    .as_ref()
                    .map(|initializer| (declarator.name.name.as_str(), initializer))
            })
        })
        .collect::<HashMap<_, _>>();
    let allowed = HashSet::new();
    class.with_clause.is_empty()
        && class.members.iter().all(|member| match member {
            ClassMember::Field(field) if !field.is_static => {
                (field.is_final || field.is_const)
                    && field.declarators.iter().all(|declarator| {
                        declarator.initializer.as_ref().is_none_or(|initializer| {
                            potentially_constant_with_shadowed(
                                initializer,
                                &allowed,
                                &shadowed,
                                owner,
                                &fields,
                                model,
                                signatures,
                            )
                        })
                    })
            }
            _ => true,
        })
}

fn constructor_can_be_const(
    class: &ClassDecl,
    constructor: &ConstructorDecl,
    types: Option<&TypeIndex>,
    owner: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    if constructor.is_factory {
        let Some(redirect) = &constructor.redirect else {
            return false;
        };
        let Some(type_name) = type_name(&redirect.type_) else {
            return false;
        };
        return types.is_some_and(|types| {
            types.constructor_is_const(
                &type_name,
                redirect.constructor_name.as_ref().map(|n| n.name.as_str()),
            ) == Some(true)
        });
    }
    if !empty_body(constructor.body.as_ref()) {
        return false;
    }
    if let Some(target) =
        constructor
            .initializers
            .iter()
            .find_map(|initializer| match initializer {
                ConstructorInitializer::ThisCall { call_name, .. } => {
                    Some(call_name.as_ref().map(|name| name.name.as_str()))
                }
                _ => None,
            })
    {
        return types.is_some_and(|types| {
            types.constructor_is_const(&class.name.name, target) == Some(true)
        });
    }
    let allowed = constructor
        .params
        .positional
        .iter()
        .chain(&constructor.params.optional_positional)
        .chain(&constructor.params.named)
        .map(|parameter| parameter.name.name.clone())
        .collect::<HashSet<_>>();
    let mut shadowed = class_lexical_value_names(class);
    shadowed.extend(allowed.iter().cloned());
    let fields = class
        .members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Field(field) if !field.is_static && (field.is_final || field.is_const) => {
                Some(field)
            }
            _ => None,
        })
        .flat_map(|field| {
            field.declarators.iter().filter_map(|declarator| {
                declarator
                    .initializer
                    .as_ref()
                    .map(|initializer| (declarator.name.name.as_str(), initializer))
            })
        })
        .collect::<HashMap<_, _>>();
    if !constructor.initializers.iter().all(|initializer| {
        initializer_allows_const(
            initializer,
            &allowed,
            &shadowed,
            owner,
            &fields,
            model,
            signatures,
        )
    }) {
        return false;
    }
    let super_call = constructor
        .initializers
        .iter()
        .find_map(|initializer| match initializer {
            ConstructorInitializer::SuperCall { call_name, .. } => {
                Some(call_name.as_ref().map(|name| name.name.as_str()))
            }
            _ => None,
        });
    let super_name = class
        .extends
        .as_ref()
        .and_then(type_name)
        .unwrap_or_else(|| "Object".to_string());
    types.is_none_or(|types| {
        types.constructor_is_const(&super_name, super_call.flatten()) == Some(true)
    })
}

fn initializer_allows_const(
    initializer: &ConstructorInitializer,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    match initializer {
        ConstructorInitializer::FieldInit { value, .. } => potentially_constant_with_shadowed(
            value, allowed, shadowed, owner, fields, model, signatures,
        ),
        ConstructorInitializer::Assert {
            condition, message, ..
        } => {
            potentially_constant_with_shadowed(
                condition, allowed, shadowed, owner, fields, model, signatures,
            ) && message.as_ref().is_none_or(|message| {
                potentially_constant_with_shadowed(
                    message, allowed, shadowed, owner, fields, model, signatures,
                )
            })
        }
        ConstructorInitializer::SuperCall { args, .. } => arguments_potentially_constant(
            args, allowed, shadowed, owner, fields, model, signatures,
        ),
        ConstructorInitializer::ThisCall { .. } => true,
    }
}

fn arguments_potentially_constant(
    args: &ArgList,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    args.positional
        .iter()
        .chain(args.named.iter().map(|argument| &argument.value))
        .all(|argument| {
            potentially_constant_with_shadowed(
                argument, allowed, shadowed, owner, fields, model, signatures,
            )
        })
}

pub(crate) fn potentially_constant(
    expr: &Expr,
    allowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    potentially_constant_with_shadowed(expr, allowed, allowed, owner, fields, model, signatures)
}

fn potentially_constant_with_shadowed(
    expr: &Expr,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let mut checker = PotentiallyConstant {
        allowed,
        shadowed,
        owner,
        fields,
        model,
        signatures,
        valid: true,
    };
    checker.visit_expr(expr);
    checker.valid
}

struct PotentiallyConstant<'a, 'model> {
    allowed: &'a HashSet<String>,
    shadowed: &'a HashSet<String>,
    owner: &'a DeclarationIdentity,
    fields: &'a HashMap<&'a str, &'a Expr>,
    model: &'a SemanticModel<'model>,
    signatures: &'a SignatureIndex,
    valid: bool,
}

impl Visitor for PotentiallyConstant<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if !self.valid {
            return;
        }
        if !matches!(node, Expr::Call { .. } | Expr::New { .. })
            && evaluate_constant(node, self.owner, self.fields, self.model, self.signatures)
                .is_some()
        {
            return;
        }
        match node {
            Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
            | Expr::SymbolLit { .. } => {}
            Expr::Ident(identifier) => {
                self.valid = self.allowed.contains(&identifier.name)
                    || evaluate_constant(
                        node,
                        self.owner,
                        self.fields,
                        self.model,
                        self.signatures,
                    )
                    .is_some();
            }
            Expr::Call { callee, args, .. } => {
                let shadowed = expression_segments(callee)
                    .and_then(|segments| segments.into_iter().next())
                    .is_some_and(|name| self.shadowed.contains(&name));
                self.valid = !shadowed && const_constructor(callee, self.model, self.signatures);
                if self.valid {
                    self.visit_arguments(args);
                }
            }
            Expr::New {
                is_const: true,
                args,
                ..
            } => self.visit_arguments(args),
            Expr::StringLit(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Conditional { .. }
            | Expr::As { .. }
            | Expr::Field { .. }
            | Expr::NullAssert { .. }
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. } => walk_expr(self, node),
            _ => self.valid = false,
        }
    }
}

impl PotentiallyConstant<'_, '_> {
    fn visit_arguments(&mut self, args: &ArgList) {
        for argument in args
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value))
        {
            self.visit_expr(argument);
        }
    }
}

fn const_constructor(
    callee: &Expr,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some(mut segments) = expression_segments(callee) else {
        return false;
    };
    let (identity, constructor) = if let Some(identity) = model.resolve_name(&segments) {
        (identity, "new".to_string())
    } else {
        let Some(constructor) = segments.pop() else {
            return false;
        };
        let Some(identity) = model.resolve_name(&segments) else {
            return false;
        };
        (identity, constructor)
    };
    signatures.declaration(&identity).is_some_and(|facts| {
        facts
            .constructors
            .iter()
            .any(|candidate| candidate.name == constructor && candidate.is_const)
    })
}

fn expression_segments(expr: &Expr) -> Option<Vec<String>> {
    let mut current = expr;
    let mut segments = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.clone());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.clone());
                current = object;
            }
            _ => return None,
        }
    }
}

fn type_name(ty: &DartType) -> Option<String> {
    match ty {
        DartType::Named(named) => named.segments.last().map(|name| name.name.clone()),
        _ => None,
    }
}

fn empty_body(body: Option<&FunctionBody>) -> bool {
    match body {
        None => true,
        Some(FunctionBody::Block(block)) => block.stmts.is_empty(),
        _ => false,
    }
}
