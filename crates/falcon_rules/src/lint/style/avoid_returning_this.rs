//! Flags methods that return `this` only to enable a fluent interface.
//!
//! Dart's cascade operator (`..`) chains calls on the same receiver without any
//! cooperation from the API, so a method returning its own receiver adds
//! nothing but a misleading return type. Return `void` and let callers cascade.
//!
//! ```dart
//! class Builder {
//!   Builder add(int x) {        // flagged at `return this`
//!     _items.add(x);
//!     return this;
//!   }
//!   Builder clear() => this;    // flagged at `clear`
//! }
//! ```
//!
//! Only methods whose declared return type is the enclosing class, enum, or
//! mixin and whose every `return` yields an unparenthesized `this` are reported.
//! Methods that override an inherited member are exempt.
//!
//! Adopted from the official `avoid_returning_this` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidReturningThis;

impl Rule for AvoidReturningThis {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            signatures,
            declarations: program.declarations.iter(),
            owner: None,
            returned: None,
            diags: Vec::new(),
        }))
    }
}

struct Returned {
    first: Option<Span>,
    all_this: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    declarations: std::slice::Iter<'a, TopLevelDecl>,
    owner: Option<(&'a str, DeclarationIdentity)>,
    returned: Option<Returned>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(
            "avoid-returning-this",
            "Don't return 'this' from a method.",
            span,
        ));
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        self.owner = self.declarations.next()
            .inspect(|declaration| debug_assert!(std::ptr::eq(*declaration, node)))
            .and_then(|declaration| match declaration {
                TopLevelDecl::Class(node) => Some(node.name.name.as_str()),
                TopLevelDecl::MixinClass(node) => Some(node.name.name.as_str()),
                TopLevelDecl::Enum(node) => Some(node.name.name.as_str()),
                TopLevelDecl::Mixin(node) => Some(node.name.name.as_str()),
                _ => None,
            })
            .and_then(|name| self.model.resolve_name(name).map(|owner| (name, owner)));
        if self.owner.is_some() {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember) -> StructuralTraversal {
        let Some((name, owner)) = &self.owner else {
            return StructuralTraversal::SkipChildren;
        };
        let (member_name, return_type, body, annotations, getter) = match member {
            ClassMember::Method(method) => (
                &method.name, &method.return_type, &method.body, &method.annotations, false,
            ),
            ClassMember::Getter(getter) => (
                &getter.name, &getter.return_type, &getter.body, &getter.annotations, true,
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        let (Some(return_type), Some(body)) = (return_type, body) else {
            return StructuralTraversal::SkipChildren;
        };
        if !names_type(return_type, name) || has_override(annotations) {
            return StructuralTraversal::SkipChildren;
        }
        let overrides = self.signatures
            .inherited_member_facts(owner, &member_name.name)
            .unwrap_or_default()
            .iter()
            .any(|fact| {
                if getter { fact.kind.has_getter() } else { fact.kind == MemberKind::Method }
            });
        if overrides {
            return StructuralTraversal::SkipChildren;
        }
        match body {
            FunctionBody::Arrow(expr, _) => {
                if matches!(expr.as_ref(), Expr::This { .. }) {
                    self.report(member_name.span);
                }
                StructuralTraversal::SkipChildren
            }
            FunctionBody::Block(_) => {
                self.returned = Some(Returned { first: None, all_this: true });
                StructuralTraversal::Continue
            }
            FunctionBody::Native(..) => StructuralTraversal::SkipChildren,
        }
    }

    fn exit_class_member(&mut self, _member: &ClassMember) {
        if let Some(returned) = self.returned.take()
            && returned.all_this
            && let Some(span) = returned.first
        {
            self.report(span);
        }
    }

    fn visit_expr(&mut self, _node: &Expr) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if matches!(node, Stmt::LocalFunc(_)) {
            StructuralTraversal::SkipChildren
        } else {
            StructuralTraversal::Continue
        }
    }

    fn visit_return_stmt(&mut self, node: &ReturnStmt) -> StructuralTraversal {
        if let Some(returned) = &mut self.returned
            && let Some(value) = &node.value
        {
            returned.first.get_or_insert(*value.span());
            returned.all_this &= matches!(value, Expr::This { .. });
        }
        StructuralTraversal::SkipChildren
    }
}
}

/// Whether `ty` names the enclosing declaration (type arguments and
/// nullability do not change the referenced element).
fn names_type(ty: &DartType, name: &str) -> bool {
    matches!(ty, DartType::Named(named)
        if matches!(named.segments.as_slice(), [segment] if segment.name == name))
}

fn has_override(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|annotation| matches!(annotation.name.as_slice(), [name] if name.name == "override"))
}
