//! Flags a generative constructor that is legally eligible to be `const`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::const_eval::{arguments_potentially_constant, potentially_constant};

#[derive(Default)]
pub struct PreferDeclaringConstConstructor;

impl Rule for PreferDeclaringConstConstructor {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Class(class) = declaration {
            check_class(class, &self.model, self.signatures, self.ctx, &mut self.diags);
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn check_class(
    class: ClassDecl<'_>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = model.resolve_name(class.name().name()) else {
        return;
    };
    if !class.with_clause().is_empty() {
        return;
    }
    if class.members().any(|member| {
        matches!(member, ClassMember::Field(field)
            if !field.is_static() && (field.keyword() != VarKeyword::Final || field.is_late()))
    }) {
        return;
    }
    let constant_fields = class
        .members()
        .filter_map(|member| match member {
            ClassMember::Field(field) if field.keyword() == VarKeyword::Const => Some(field),
            _ => None,
        })
        .flat_map(|field| {
            field.declarators().filter_map(|declarator| {
                declarator
                    .initializer()
                    .map(|value| (declarator.name().name(), value))
            })
        })
        .collect::<HashMap<_, _>>();
    if class.members().any(|member| match member {
        ClassMember::Field(field) if !field.is_static() => field.declarators().any(|declarator| {
            declarator.initializer().is_some_and(|initializer| {
                !potentially_constant(
                    initializer,
                    &HashSet::new(),
                    &owner,
                    &constant_fields,
                    model,
                    signatures,
                )
            })
        }),
        _ => false,
    }) {
        return;
    }

    let constructors = class
        .members()
        .filter_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut const_names = constructors
        .iter()
        .filter(|constructor| constructor.is_const())
        .map(|constructor| constructor_name(*constructor).to_string())
        .collect::<HashSet<_>>();
    let mut candidates = HashSet::new();
    loop {
        let mut changed = false;
        for &constructor in &constructors {
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
            diags.push(ctx.diagnostic("Constructor could be declared as `const`.", constructor.span()));
        }
    }
}

fn constructor_candidate<'a>(
    constructor: ConstructorDecl<'a>,
    class: ClassDecl<'a>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, Expr<'a>>,
    const_names: &HashSet<String>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    if constructor.is_const()
        || constructor.is_factory()
        || constructor.is_external()
        || constructor.redirect().is_some()
        || constructor.body().is_some()
    {
        return false;
    }
    let parameters = constructor
        .params()
        .params()
        .map(|parameter| parameter.name().name())
        .collect::<HashSet<_>>();
    let mut has_super_call = false;
    let mut has_this_call = false;
    for initializer in constructor.initializers() {
        match initializer {
            ConstructorInitializer::SuperCall(call) => {
                has_super_call = true;
                if !super_constructor_const(class, call.call_name(), model, signatures)
                    || !arguments_potentially_constant(
                        call.args(),
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
            ConstructorInitializer::ThisCall(call) => {
                has_this_call = true;
                let target = call.call_name().map_or("new", |name| name.name());
                if !const_names.contains(target)
                    || !arguments_potentially_constant(
                        call.args(),
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
            ConstructorInitializer::FieldInit(field) => {
                if !potentially_constant(
                    field.value(),
                    &parameters,
                    owner,
                    fields,
                    model,
                    signatures,
                ) {
                    return false;
                }
            }
            ConstructorInitializer::Assert(assertion) => {
                if !potentially_constant(
                    assertion.condition(),
                    &parameters,
                    owner,
                    fields,
                    model,
                    signatures,
                ) || assertion.message().is_some_and(|message| {
                    !potentially_constant(message, &parameters, owner, fields, model, signatures)
                }) {
                    return false;
                }
            }
        }
    }
    has_this_call || has_super_call || implicit_super_const(class, model, signatures)
}

fn implicit_super_const(
    class: ClassDecl<'_>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some(supertype) = class.extends() else {
        return true;
    };
    let ResolvedType::Interface { identity, .. } = model.resolve_type(supertype) else {
        return false;
    };
    constructor_const(&identity, "new", signatures)
}

fn super_constructor_const(
    class: ClassDecl<'_>,
    name: Option<Identifier<'_>>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some(supertype) = class.extends() else {
        return name.is_none();
    };
    let ResolvedType::Interface { identity, .. } = model.resolve_type(supertype) else {
        return false;
    };
    constructor_const(
        &identity,
        name.map_or("new", |identifier| identifier.name()),
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

fn constructor_name(constructor: ConstructorDecl<'_>) -> &str {
    constructor
        .constructor_name()
        .map_or("new", |name| name.name())
}
