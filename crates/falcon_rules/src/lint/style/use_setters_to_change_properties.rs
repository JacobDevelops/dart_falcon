//! Flags one-parameter `void` methods that only assign that parameter to a
//! field.
//!
//! A method whose whole body is `field = value` conceptually changes a property;
//! a setter states that directly and lets callers use assignment syntax.
//!
//! ```dart
//! class Rectangle {
//!   num width = 0;
//!   void setWidth(num width) {   // flagged
//!     this.width = width;
//!   }
//!   set height(num value) => _height = value;  // preferred
//! }
//! ```
//!
//! The assignment target must resolve to a field or setter, including a
//! property on another typed receiver. Overriding methods are exempt;
//! extension methods never override, even with `@override`.
//! Extension receivers use their declared type-parameter bounds; null assertions
//! and null-aware assignments look up properties on the non-null receiver.
//! Unavailable external inheritance metadata conservatively exempts public
//! methods; private names cannot override across libraries.
//!
//! Adopted from the official `use_setters_to_change_properties` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, ResolvedType, Rule, RulePreparation,
    SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseSettersToChangeProperties;

impl Rule for UseSettersToChangeProperties {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            signatures,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            owner: None,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    signatures: &'a SignatureIndex,
    model: SemanticModel<'a>,
    owner: Option<(AstChildren<'a, ClassMember<'a>>, ResolvedType, bool, bool)>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        self.owner = None;
        let model = &self.model;
            let (owner_type, unresolved_super, extension) = match decl {
                TopLevelDecl::Extension(node) => {
                    // Extension members resolve `this` against the `on` type
                    // and never override.
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(node.type_params(), model);
                    (
                        model.resolve_type_in(node.on_type(), &parameters),
                        false,
                        true,
                    )
                }
                _ => {
                    let name = match decl {
                        TopLevelDecl::Class(node) => node.name().name(),
                        TopLevelDecl::MixinClass(node) => node.name().name(),
                        TopLevelDecl::Mixin(node) => node.name().name(),
                        TopLevelDecl::Enum(node) => node.name().name(),
                        TopLevelDecl::ExtensionType(node) => node.name().name(),
                        _ => return StructuralTraversal::Continue,
                    };
                    let Some(owner) = model.resolve_name(name) else {
                        return StructuralTraversal::Continue;
                    };
                    let unresolved_super = match decl {
                        TopLevelDecl::Class(class) => class.extends().is_some_and(|ty| {
                            matches!(model.resolve_type(ty), ResolvedType::Unknown)
                        }),
                        _ => false,
                    };
                    (
                        ResolvedType::interface_identity(owner),
                        unresolved_super,
                        false,
                    )
                }
            };
        self.owner = crate::lint::declarations::members_of(decl)
            .map(|members| (members, owner_type, unresolved_super, extension));
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, method: MethodDecl<'a>) -> StructuralTraversal {
        if let Some((members, owner_type, unresolved_super, extension)) = &self.owner
            && changes_property(
                method, members.clone(), owner_type, self.signatures, &self.model,
                *unresolved_super, *extension,
            )
        {
                    self.diags.push(self.ctx.diagnostic(
                        "The method is used to change a property.",
                        method.name().span(),
                    ));
                }
        StructuralTraversal::Continue
    }
}
}

fn changes_property<'a>(
    method: MethodDecl<'a>,
    members: AstChildren<'a, ClassMember<'a>>,
    owner_type: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    unresolved_super: bool,
    extension: bool,
) -> bool {
    let mut params = method.params().params();
    let (Some(param), None, Some(DartType::Void(_)), Some(body)) = (
        params.next(),
        params.next(),
        method.return_type(),
        method.body(),
    ) else {
        return false;
    };
    let mut owner_type = owner_type;
    while let ResolvedType::TypeParameter { bound, .. }
    | ResolvedType::NonNullTypeParameter { bound, .. } = owner_type
    {
        owner_type = bound;
    }
    let ResolvedType::Interface {
        identity: owner, ..
    } = owner_type
    else {
        return false;
    };
    let expr = match body {
        FunctionBody::Arrow(arrow) => arrow.expr(),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            match (statements.next(), statements.next()) {
                (Some(Stmt::Expr(statement)), None) => statement.expr(),
                _ => return false,
            }
        }
        FunctionBody::Native(_) => return false,
    };
    let Expr::Assign(assignment) = expr.unparenthesized() else {
        return false;
    };
    if assignment.op() != AssignOp::Eq
        || !matches!(assignment.value().unparenthesized(), Expr::Ident(identifier) if identifier.name() == param.name().name())
    {
        return false;
    }
    // A property of the type itself: declared by it or, for an extension,
    // by the extension or its `on` type.
    let own_property = |name: &str, receiver: &ResolvedType| {
        declares_property(members.clone(), name)
            || if extension {
                signatures
                    .resolved_writable_field(receiver, name, model)
                    .is_some()
            } else {
                inherits_setter(owner, name, signatures)
            }
    };
    let assigns_field = match assignment.target().unparenthesized() {
        Expr::Field(field)
            if !method.is_static()
                && (matches!(field.object().unparenthesized(), Expr::This(_))
                    || matches!(field.object().unparenthesized(), Expr::NullAssert(assertion)
                    if matches!(assertion.operand().unparenthesized(), Expr::This(_)))) =>
        {
            if field.is_null_safe()
                || matches!(field.object().unparenthesized(), Expr::NullAssert(_))
            {
                own_property(field.field().name(), &owner_type.non_null())
            } else {
                own_property(field.field().name(), owner_type)
            }
        }
        Expr::Field(field) => {
            let (object, asserted_non_null) = match field.object().unparenthesized() {
                Expr::NullAssert(assertion) => (assertion.operand().unparenthesized(), true),
                object => (object, false),
            };
            let receiver = if let Expr::Ident(identifier) = object {
                if identifier.name() == param.name().name() {
                    param.param_type().map(|ty| model.resolve_type(ty))
                } else {
                    signatures
                        .resolved_field(owner_type, identifier.name(), model)
                        .or_else(|| {
                            signatures.resolved_extension_getter(
                                owner_type,
                                identifier.name(),
                                model,
                            )
                        })
                        .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
                }
            } else {
                None
            };
            receiver.is_some_and(|receiver| {
                let receiver = if field.is_null_safe() || asserted_non_null {
                    receiver.non_null()
                } else {
                    receiver
                };
                signatures
                    .resolved_writable_field(&receiver, field.field().name(), model)
                    .is_some()
            })
        }
        // An unqualified name resolves lexically, so only the type's own
        // members are known to win over library-level declarations; inside
        // an extension the `on` type is only reached when nothing lexical is.
        Expr::Ident(identifier) if identifier.name() != param.name().name() => {
            declares_property(members.clone(), identifier.name())
                || if extension {
                    !method.is_static()
                        && model.resolve_value(identifier.name()).is_none()
                        && signatures
                            .resolved_writable_field(owner_type, identifier.name(), model)
                            .is_some()
                } else {
                    inherits_setter(owner, identifier.name(), signatures)
                }
        }
        _ => false,
    };
    assigns_field && (extension || !overrides_method(method, owner, signatures, unresolved_super))
}

/// A field or setter of this name declared by the type itself.
fn declares_property<'a>(mut members: AstChildren<'a, ClassMember<'a>>, name: &str) -> bool {
    members.any(|member| match member {
        ClassMember::Field(field) => field
            .declarators()
            .any(|declarator| declarator.name().name() == name),
        ClassMember::Setter(setter) => setter.name().name() == name,
        _ => false,
    })
}

fn inherits_setter(owner: &DeclarationIdentity, name: &str, signatures: &SignatureIndex) -> bool {
    signatures
        .inherited_member_facts(owner, name)
        .is_some_and(|facts| facts.iter().any(|fact| fact.kind.has_setter()))
}

/// Whether the method overrides an inherited method.
fn overrides_method(
    method: MethodDecl<'_>,
    owner: &DeclarationIdentity,
    signatures: &SignatureIndex,
    unresolved_super: bool,
) -> bool {
    if has_override(method) {
        return true;
    }
    signatures
        .inherited_member_facts(owner, method.name().name())
        .map_or(
            !unresolved_super && !method.name().name().starts_with('_'),
            |facts| facts.iter().any(|fact| fact.kind == MemberKind::Method),
        )
}

fn has_override(method: MethodDecl<'_>) -> bool {
    method.annotations().any(|annotation| {
        let mut names = annotation.name();
        matches!((names.next(), names.next()), (Some(name), None) if name.name() == "override")
    })
}
