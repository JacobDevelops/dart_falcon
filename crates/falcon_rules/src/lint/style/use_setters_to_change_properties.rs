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
    AnalyzeContext, DeclarationIdentity, MemberKind, ResolvedType, Rule, SemanticModel,
    SignatureIndex, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseSettersToChangeProperties;

impl Rule for UseSettersToChangeProperties {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (owner_type, members, unresolved_super, extension) = match decl {
                TopLevelDecl::Extension(node) => {
                    // Extension members resolve `this` against the `on` type
                    // and never override.
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(&node.type_params, &model);
                    (
                        model.resolve_type_in(&node.on_type, &parameters),
                        &node.members,
                        false,
                        true,
                    )
                }
                _ => {
                    let (name, members) = match decl {
                        TopLevelDecl::Class(node) => (&node.name.name, &node.members),
                        TopLevelDecl::MixinClass(node) => (&node.name.name, &node.members),
                        TopLevelDecl::Mixin(node) => (&node.name.name, &node.members),
                        TopLevelDecl::Enum(node) => (&node.name.name, &node.members),
                        TopLevelDecl::ExtensionType(node) => (&node.name.name, &node.members),
                        _ => continue,
                    };
                    let Some(owner) = model.resolve_name(name.as_str()) else {
                        continue;
                    };
                    let unresolved_super = match decl {
                        TopLevelDecl::Class(class) => class.extends.as_ref().is_some_and(|ty| {
                            matches!(model.resolve_type(ty), ResolvedType::Unknown)
                        }),
                        _ => false,
                    };
                    (
                        ResolvedType::interface_identity(owner),
                        members,
                        unresolved_super,
                        false,
                    )
                }
            };
            for member in members {
                let ClassMember::Method(method) = member else {
                    continue;
                };
                if changes_property(
                    method,
                    members,
                    &owner_type,
                    signatures,
                    &model,
                    unresolved_super,
                    extension,
                ) {
                    diags.push(ctx.diagnostic(
                        "use-setters-to-change-properties",
                        "The method is used to change a property.",
                        method.name.span,
                    ));
                }
            }
        }
        diags
    }
}

fn changes_property(
    method: &MethodDecl,
    members: &[ClassMember],
    owner_type: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    unresolved_super: bool,
    extension: bool,
) -> bool {
    let ([param], Some(DartType::Void { .. }), Some(body)) = (
        method.params.params.as_slice(),
        &method.return_type,
        &method.body,
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
        FunctionBody::Arrow(expr, _) => &**expr,
        FunctionBody::Block(block) => match block.stmts.as_slice() {
            [Stmt::Expr(statement)] => &statement.expr,
            _ => return false,
        },
        FunctionBody::Native(..) => return false,
    };
    let Expr::Assign {
        target,
        op: AssignOp::Eq,
        value,
        ..
    } = expr.unparenthesized()
    else {
        return false;
    };
    if !matches!(value.unparenthesized(), Expr::Ident(identifier) if identifier.name == param.name.name)
    {
        return false;
    }
    // A property of the type itself: declared by it or, for an extension,
    // by the extension or its `on` type.
    let own_property = |name: &str, receiver: &ResolvedType| {
        declares_property(members, name)
            || if extension {
                signatures
                    .resolved_writable_field(receiver, name, model)
                    .is_some()
            } else {
                inherits_setter(owner, name, signatures)
            }
    };
    let assigns_field = match target.unparenthesized() {
        Expr::Field {
            object,
            field,
            is_null_safe,
            ..
        } if !method.is_static
            && (matches!((**object).unparenthesized(), Expr::This { .. })
                || matches!(object.unparenthesized(), Expr::NullAssert { operand, .. }
                    if matches!(operand.unparenthesized(), Expr::This { .. }))) =>
        {
            if *is_null_safe || matches!(object.unparenthesized(), Expr::NullAssert { .. }) {
                own_property(&field.name, &owner_type.non_null())
            } else {
                own_property(&field.name, owner_type)
            }
        }
        Expr::Field {
            object,
            field,
            is_null_safe,
            ..
        } => {
            let (object, asserted_non_null) = match object.unparenthesized() {
                Expr::NullAssert { operand, .. } => (operand.unparenthesized(), true),
                object => (object, false),
            };
            let receiver = if let Expr::Ident(identifier) = object {
                if identifier.name == param.name.name {
                    param.param_type.as_ref().map(|ty| model.resolve_type(ty))
                } else {
                    signatures
                        .resolved_field(owner_type, &identifier.name, model)
                        .or_else(|| {
                            signatures.resolved_extension_getter(
                                owner_type,
                                &identifier.name,
                                model,
                            )
                        })
                        .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
                }
            } else {
                None
            };
            receiver.is_some_and(|receiver| {
                let receiver = if *is_null_safe || asserted_non_null {
                    receiver.non_null()
                } else {
                    receiver
                };
                signatures
                    .resolved_writable_field(&receiver, &field.name, model)
                    .is_some()
            })
        }
        // An unqualified name resolves lexically, so only the type's own
        // members are known to win over library-level declarations; inside
        // an extension the `on` type is only reached when nothing lexical is.
        Expr::Ident(identifier) if identifier.name != param.name.name => {
            declares_property(members, &identifier.name)
                || if extension {
                    !method.is_static
                        && model.resolve_value(identifier.name.as_str()).is_none()
                        && signatures
                            .resolved_writable_field(owner_type, &identifier.name, model)
                            .is_some()
                } else {
                    inherits_setter(owner, &identifier.name, signatures)
                }
        }
        _ => false,
    };
    assigns_field && (extension || !overrides_method(method, owner, signatures, unresolved_super))
}

/// A field or setter of this name declared by the type itself.
fn declares_property(members: &[ClassMember], name: &str) -> bool {
    members.iter().any(|member| match member {
        ClassMember::Field(field) => field
            .declarators
            .iter()
            .any(|declarator| declarator.name.name == name),
        ClassMember::Setter(setter) => setter.name.name == name,
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
    method: &MethodDecl,
    owner: &DeclarationIdentity,
    signatures: &SignatureIndex,
    unresolved_super: bool,
) -> bool {
    if has_override(method) {
        return true;
    }
    signatures
        .inherited_member_facts(owner, &method.name.name)
        .map_or(
            !unresolved_super && !method.name.name.starts_with('_'),
            |facts| facts.iter().any(|fact| fact.kind == MemberKind::Method),
        )
}

fn has_override(method: &MethodDecl) -> bool {
    method
        .annotations
        .iter()
        .any(|annotation| matches!(annotation.name.as_slice(), [name] if name.name == "override"))
}
