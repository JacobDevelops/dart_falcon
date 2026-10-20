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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidReturningThis;

impl Rule for AvoidReturningThis {
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
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.owner = match node {
            TopLevelDecl::Class(node) => Some(node.name().name()),
            TopLevelDecl::MixinClass(node) => Some(node.name().name()),
            TopLevelDecl::Enum(node) => Some(node.name().name()),
            TopLevelDecl::Mixin(node) => Some(node.name().name()),
            _ => None,
        }
        .and_then(|name| self.model.resolve_name(name).map(|owner| (name, owner)));
        if self.owner.is_some() {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let Some((name, owner)) = &self.owner else {
            return StructuralTraversal::SkipChildren;
        };
        let (member_name, return_type, body, annotations, getter) = match member {
            ClassMember::Method(method) => (
                method.name(), method.return_type(), method.body(), method.annotations(), false,
            ),
            ClassMember::Getter(getter) => (
                getter.name(), getter.return_type(), getter.body(), getter.annotations(), true,
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
            .inherited_member_facts(owner, member_name.name())
            .unwrap_or_default()
            .iter()
            .any(|fact| {
                if getter { fact.kind.has_getter() } else { fact.kind == MemberKind::Method }
            });
        if overrides {
            return StructuralTraversal::SkipChildren;
        }
        match body {
            FunctionBody::Arrow(body) => {
                if matches!(body.expr(), Expr::This(_)) {
                    self.report(member_name.span());
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

    fn exit_class_member(&mut self, _member: ClassMember<'a>) {
        if let Some(returned) = self.returned.take()
            && returned.all_this
            && let Some(span) = returned.first
        {
            self.report(span);
        }
    }

    fn visit_expr(&mut self, _node: Expr<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if matches!(node, Stmt::LocalFunc(_)) {
            StructuralTraversal::SkipChildren
        } else {
            StructuralTraversal::Continue
        }
    }

    fn visit_return_stmt(&mut self, node: ReturnStmt<'a>) -> StructuralTraversal {
        if let Some(returned) = &mut self.returned
            && let Some(value) = node.value()
        {
            returned.first.get_or_insert(value.span());
            returned.all_this &= matches!(value, Expr::This(_));
        }
        StructuralTraversal::SkipChildren
    }
}
}

/// Whether `ty` names the enclosing declaration (type arguments and
/// nullability do not change the referenced element).
fn names_type(ty: DartType<'_>, name: &str) -> bool {
    matches!(ty, DartType::Named(named) if {
        let mut segments = named.segments();
        segments.next().is_some_and(|segment| segment.name() == name) && segments.next().is_none()
    })
}

fn has_override<'a>(mut annotations: AstChildren<'a, Annotation<'a>>) -> bool {
    annotations.any(|annotation| {
        let mut names = annotation.name();
        names.next().is_some_and(|name| name.name() == "override") && names.next().is_none()
    })
}
