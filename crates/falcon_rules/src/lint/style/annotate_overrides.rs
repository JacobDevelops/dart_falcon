//! Requires `@override` on members that override an inherited declaration.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberFacts, MemberKind, Rule, SemanticModel,
    SignatureIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AnnotateOverrides;

type Candidate<'a> = (&'a str, &'a Span, &'a [Annotation], MemberKind);

impl Rule for AnnotateOverrides {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::Mixin(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::MixinClass(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::Enum(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::ExtensionType(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::ClassTypeAlias(_)
                | TopLevelDecl::Extension(_)
                | TopLevelDecl::Function(_)
                | TopLevelDecl::Variable(_)
                | TopLevelDecl::TypeAlias(_)
                | TopLevelDecl::Error(_) => {}
            }
        }
        diags
    }
}

fn has_override(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|annotation| {
        annotation
            .name
            .last()
            .is_some_and(|name| name.name == "override")
    })
}

fn check_members(
    owner: Option<DeclarationIdentity>,
    members: &[ClassMember],
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = owner else {
        return;
    };
    for member in members {
        let candidates: Vec<Candidate<'_>> = match member {
            ClassMember::Field(field) if !field.is_static => field
                .declarators
                .iter()
                .map(|declarator| {
                    (
                        declarator.name.name.as_str(),
                        &declarator.name.span,
                        field.annotations.as_slice(),
                        MemberKind::field(field),
                    )
                })
                .collect(),
            ClassMember::Method(method) if !method.is_static => vec![(
                method.name.name.as_str(),
                &method.name.span,
                method.annotations.as_slice(),
                MemberKind::Method,
            )],
            ClassMember::Getter(getter) if !getter.is_static => vec![(
                getter.name.name.as_str(),
                &getter.name.span,
                getter.annotations.as_slice(),
                MemberKind::Getter,
            )],
            ClassMember::Setter(setter) if !setter.is_static => vec![(
                setter.name.name.as_str(),
                &setter.name.span,
                setter.annotations.as_slice(),
                MemberKind::Setter,
            )],
            ClassMember::Operator(operator) => vec![(
                operator.op.as_str(),
                &operator.span,
                operator.annotations.as_slice(),
                MemberKind::Operator,
            )],
            ClassMember::Field(_)
            | ClassMember::Constructor(_)
            | ClassMember::Method(_)
            | ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Error(_) => Vec::new(),
        };
        for (name, span, annotations, kind) in candidates {
            let overrides = signatures
                .inherited_member_facts(&owner, name)
                .is_some_and(|facts| facts.iter().any(|fact| compatible(kind, fact)));
            if !has_override(annotations) && overrides {
                diags.push(ctx.diagnostic(
                    "annotate-overrides",
                    "Annotate overridden members with '@override'.",
                    *span,
                ));
            }
        }
    }
}

fn compatible(declared: MemberKind, inherited: &MemberFacts) -> bool {
    match declared {
        MemberKind::Method | MemberKind::Operator => inherited.kind == declared,
        _ => {
            (declared.has_getter() && inherited.kind.has_getter())
                || (declared.has_setter() && inherited.kind.has_setter())
        }
    }
}
