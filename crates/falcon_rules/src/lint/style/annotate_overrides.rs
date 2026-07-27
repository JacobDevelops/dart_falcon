//! Requires `@override` on members that override an inherited declaration.

use falcon_analyze::{AnalyzeContext, MemberFacts, Rule, SemanticMemberKind, SignatureIndex};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AnnotateOverrides;

type Candidate<'a> = (
    &'a str,
    &'a Span,
    &'a [Annotation],
    SemanticMemberKind,
    bool,
    bool,
);

impl Rule for AnnotateOverrides {
    fn name(&self) -> &'static str {
        "annotate-overrides"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(_), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::Mixin(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::MixinClass(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::Enum(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::ExtensionType(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
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
    type_name: &str,
    members: &[ClassMember],
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = ctx.identities.and_then(|identities| {
        identities.resolve_declaration(ctx.file_path, &[type_name.to_string()])
    }) else {
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
                        SemanticMemberKind::Field,
                        true,
                        !field.is_final && !field.is_const,
                    )
                })
                .collect(),
            ClassMember::Method(method) if !method.is_static => vec![(
                method.name.name.as_str(),
                &method.name.span,
                method.annotations.as_slice(),
                SemanticMemberKind::Method,
                false,
                false,
            )],
            ClassMember::Getter(getter) if !getter.is_static => vec![(
                getter.name.name.as_str(),
                &getter.name.span,
                getter.annotations.as_slice(),
                SemanticMemberKind::Getter,
                true,
                false,
            )],
            ClassMember::Setter(setter) if !setter.is_static => vec![(
                setter.name.name.as_str(),
                &setter.name.span,
                setter.annotations.as_slice(),
                SemanticMemberKind::Setter,
                false,
                true,
            )],
            ClassMember::Operator(operator) => vec![(
                operator.op.as_str(),
                &operator.span,
                operator.annotations.as_slice(),
                SemanticMemberKind::Operator,
                false,
                false,
            )],
            ClassMember::Field(_)
            | ClassMember::Constructor(_)
            | ClassMember::Method(_)
            | ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Error(_) => Vec::new(),
        };
        for (name, span, annotations, kind, has_getter, has_setter) in candidates {
            let overrides = signatures
                .inherited_member_facts(&owner, name)
                .is_some_and(|facts| {
                    facts
                        .iter()
                        .any(|fact| compatible(kind, has_getter, has_setter, fact))
                });
            if !has_override(annotations) && overrides {
                diags.push(Diagnostic::new(
                    "annotate-overrides",
                    Severity::Warning,
                    "Annotate overridden members with '@override'.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
        }
    }
}

fn compatible(
    declared: SemanticMemberKind,
    has_getter: bool,
    has_setter: bool,
    inherited: &MemberFacts,
) -> bool {
    match declared {
        SemanticMemberKind::Method => inherited.kind == SemanticMemberKind::Method,
        SemanticMemberKind::Getter => inherited.has_getter,
        SemanticMemberKind::Setter => inherited.has_setter,
        SemanticMemberKind::Operator => inherited.kind == SemanticMemberKind::Operator,
        SemanticMemberKind::Field => {
            (has_getter && inherited.has_getter) || (has_setter && inherited.has_setter)
        }
    }
}
