//! Prefers `const` constructors in immutable classes.

use std::collections::{HashMap, HashSet};

use crate::lint::const_eval::{arguments_potentially_constant, potentially_constant_with_shadowed};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, Rule, SemanticModel, SignatureIndex, Truth, TypeIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferConstConstructorsInImmutables;

impl Rule for PreferConstConstructorsInImmutables {
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
            let Some(owner) = model.resolve_name(&class.name.name) else {
                continue;
            };
            let immutable = ctx.types.map_or(Truth::Unknown, |types| {
                types.is_immutable_type(&class.name.name)
            });
            let immutable = match immutable {
                Truth::Unknown => is_immutable(&class.annotations, program),
                known => known == Truth::Yes,
            };
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
                diags.push(ctx.diagnostic(
                    "prefer-const-constructors-in-immutables",
                    "Declare a const constructor in an immutable class.",
                    constructor.span,
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
            ClassMember::Field(field)
                if !field.is_static && (field.keyword != falcon_syntax::ast::VarKeyword::Var) =>
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
                    .map(|initializer| (declarator.name.name.as_str(), initializer))
            })
        })
        .collect::<HashMap<_, _>>();
    let allowed = HashSet::new();
    class.with_clause.is_empty()
        && class.members.iter().all(|member| match member {
            ClassMember::Field(field) if !field.is_static => {
                (field.keyword != falcon_syntax::ast::VarKeyword::Var)
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
            ) == Truth::Yes
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
            types.constructor_is_const(&class.name.name, target) == Truth::Yes
        });
    }
    let allowed = constructor
        .params
        .params
        .iter()
        .map(|parameter| parameter.name.name.clone())
        .collect::<HashSet<_>>();
    let mut shadowed = class_lexical_value_names(class);
    shadowed.extend(allowed.iter().cloned());
    let fields = class
        .members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Field(field)
                if !field.is_static && (field.keyword != falcon_syntax::ast::VarKeyword::Var) =>
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
        types.constructor_is_const(&super_name, super_call.flatten()) == Truth::Yes
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
