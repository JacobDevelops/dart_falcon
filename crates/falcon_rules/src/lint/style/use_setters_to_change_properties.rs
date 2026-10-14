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
//! The assignment target must be a field of the enclosing type: `this.name`
//! (declared or inherited) or an unqualified name declared by the type itself.
//! Methods that override an inherited method are exempt; when inheritance
//! cannot be resolved the rule stays silent.
//!
//! Adopted from the official `use_setters_to_change_properties` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, SemanticModel, SignatureIndex,
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
            for member in members {
                let ClassMember::Method(method) = member else {
                    continue;
                };
                if changes_property(method, members, &owner, signatures) {
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
    owner: &DeclarationIdentity,
    signatures: &SignatureIndex,
) -> bool {
    let ([param], Some(DartType::Void { .. }), Some(body)) = (
        method.params.params.as_slice(),
        &method.return_type,
        &method.body,
    ) else {
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
    } = expr
    else {
        return false;
    };
    if !matches!(&**value, Expr::Ident(identifier) if identifier.name == param.name.name) {
        return false;
    }
    let assigns_field = match &**target {
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } if matches!(**object, Expr::This { .. }) && !method.is_static => {
            declares_property(members, &field.name)
                || inherits_setter(owner, &field.name, signatures)
        }
        // An unqualified name resolves lexically, so only the type's own
        // members are known to win over library-level declarations.
        Expr::Ident(identifier) if identifier.name != param.name.name => {
            declares_property(members, &identifier.name)
        }
        _ => false,
    };
    assigns_field && !overrides_method(method, owner, signatures)
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

/// Whether the method overrides an inherited method. Unresolvable
/// inheritance counts as overriding so the rule stays silent.
fn overrides_method(
    method: &MethodDecl,
    owner: &DeclarationIdentity,
    signatures: &SignatureIndex,
) -> bool {
    if method
        .annotations
        .iter()
        .any(|annotation| matches!(annotation.name.as_slice(), [name] if name.name == "override"))
    {
        return true;
    }
    signatures
        .inherited_member_facts(owner, &method.name.name)
        .is_none_or(|facts| facts.iter().any(|fact| fact.kind == MemberKind::Method))
}
