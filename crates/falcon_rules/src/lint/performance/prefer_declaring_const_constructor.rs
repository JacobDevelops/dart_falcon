//! Flags a generative constructor that is legally eligible to be `const`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::const_eval::{arguments_potentially_constant, potentially_constant};

#[derive(Default)]
pub struct PreferDeclaringConstConstructor;

impl Rule for PreferDeclaringConstConstructor {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            check_class(class, &model, signatures, ctx, &mut diags);
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn check_class(
    class: &ClassDecl,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = model.resolve_name(&class.name.name) else {
        return;
    };
    if !class.with_clause.is_empty() {
        return;
    }
    if class.members.iter().any(|member| {
        matches!(member, ClassMember::Field(field)
            if !field.is_static && (field.keyword != VarKeyword::Final || field.is_late))
    }) {
        return;
    }
    let constant_fields = class
        .members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Field(field) if field.keyword == falcon_syntax::ast::VarKeyword::Const => {
                Some(field)
            }
            _ => None,
        })
        .flat_map(|field| {
            field.declarators.iter().filter_map(|declarator| {
                declarator
                    .initializer
                    .as_ref()
                    .map(|value| (declarator.name.name.as_str(), value))
            })
        })
        .collect::<HashMap<_, _>>();
    if class.members.iter().any(|member| match member {
        ClassMember::Field(field) if !field.is_static => {
            field.declarators.iter().any(|declarator| {
                declarator.initializer.as_ref().is_some_and(|initializer| {
                    !potentially_constant(
                        initializer,
                        &HashSet::new(),
                        &owner,
                        &constant_fields,
                        model,
                        signatures,
                    )
                })
            })
        }
        _ => false,
    }) {
        return;
    }

    let constructors = class
        .members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut const_names = constructors
        .iter()
        .filter(|constructor| constructor.is_const)
        .map(|constructor| constructor_name(constructor).to_string())
        .collect::<HashSet<_>>();
    let mut candidates = HashSet::new();
    loop {
        let mut changed = false;
        for constructor in &constructors {
            let name = constructor_name(constructor);
            if const_names.contains(name)
                || !constructor_candidate(
                    constructor,
                    class,
                    &owner,
                    &constant_fields,
                    &const_names,
                    model,
                    signatures,
                )
            {
                continue;
            }
            const_names.insert(name.to_string());
            candidates.insert(name.to_string());
            changed = true;
        }
        if !changed {
            break;
        }
    }

    for constructor in constructors {
        if candidates.contains(constructor_name(constructor)) {
            diags.push(ctx.diagnostic(
                "prefer-declaring-const-constructor",
                "Constructor could be declared as const.",
                constructor.span,
            ));
        }
    }
}

fn constructor_candidate(
    constructor: &ConstructorDecl,
    class: &ClassDecl,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    const_names: &HashSet<String>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    if constructor.is_const
        || constructor.is_factory
        || constructor.is_external
        || constructor.redirect.is_some()
        || constructor.body.is_some()
    {
        return false;
    }
    let parameters = constructor
        .params
        .params
        .iter()
        .map(|parameter| parameter.name.name.clone())
        .collect::<HashSet<_>>();
    let mut has_super_call = false;
    let mut has_this_call = false;
    for initializer in &constructor.initializers {
        match initializer {
            ConstructorInitializer::SuperCall {
                call_name, args, ..
            } => {
                has_super_call = true;
                if !super_constructor_const(class, call_name.as_ref(), model, signatures)
                    || !arguments_potentially_constant(
                        args,
                        &parameters,
                        &parameters,
                        owner,
                        fields,
                        model,
                        signatures,
                    )
                {
                    return false;
                }
            }
            ConstructorInitializer::ThisCall {
                call_name, args, ..
            } => {
                has_this_call = true;
                let target = call_name.as_ref().map_or("new", |name| name.name.as_str());
                if !const_names.contains(target)
                    || !arguments_potentially_constant(
                        args,
                        &parameters,
                        &parameters,
                        owner,
                        fields,
                        model,
                        signatures,
                    )
                {
                    return false;
                }
            }
            ConstructorInitializer::FieldInit { value, .. } => {
                if !potentially_constant(value, &parameters, owner, fields, model, signatures) {
                    return false;
                }
            }
            ConstructorInitializer::Assert {
                condition, message, ..
            } => {
                if !potentially_constant(condition, &parameters, owner, fields, model, signatures)
                    || message.as_ref().is_some_and(|message| {
                        !potentially_constant(
                            message,
                            &parameters,
                            owner,
                            fields,
                            model,
                            signatures,
                        )
                    })
                {
                    return false;
                }
            }
        }
    }
    has_this_call || has_super_call || implicit_super_const(class, model, signatures)
}

fn implicit_super_const(
    class: &ClassDecl,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some(supertype) = &class.extends else {
        return true;
    };
    let ResolvedType::Interface { identity, .. } = model.resolve_type(supertype) else {
        return false;
    };
    constructor_const(&identity, "new", signatures)
}

fn super_constructor_const(
    class: &ClassDecl,
    name: Option<&Identifier>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some(supertype) = &class.extends else {
        return name.is_none();
    };
    let ResolvedType::Interface { identity, .. } = model.resolve_type(supertype) else {
        return false;
    };
    constructor_const(
        &identity,
        name.map_or("new", |identifier| identifier.name.as_str()),
        signatures,
    )
}

fn constructor_const(
    identity: &DeclarationIdentity,
    name: &str,
    signatures: &SignatureIndex,
) -> bool {
    (*identity == DeclarationIdentity::OBJECT && name == "new")
        || signatures.is_const_constructor(identity, name)
}

fn constructor_name(constructor: &ConstructorDecl) -> &str {
    constructor
        .constructor_name
        .as_ref()
        .map_or("new", |name| name.name.as_str())
}
