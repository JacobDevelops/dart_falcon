//! Flags static methods that create and return an instance of their own class.
//!
//! A static method whose job is to build an instance of the enclosing type is a
//! named constructor in disguise. Named (and factory) constructors make the
//! intent explicit, show up as constructors in tooling, and work with
//! constructor tear-offs.
//!
//! ```dart
//! class Point {
//!   final num x, y;
//!   Point(this.x, this.y);
//!   static Point polar(num theta, num radius) =>   // flagged
//!       Point(radius * cos(theta), radius * sin(theta));
//!   Point.fromPolar(num theta, num radius)         // preferred
//!       : x = radius * cos(theta), y = radius * sin(theta);
//! }
//! ```
//!
//! Only non-generic static methods and getters of non-generic classes and
//! extension types are checked. As in the analyzer, the last visited constructor
//! call determines whether the body creates the declared enclosing return type.
//!
//! Adopted from the official `prefer_constructors_over_static_methods` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    StructuralNode, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferConstructorsOverStaticMethods;

impl Rule for PreferConstructorsOverStaticMethods {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            owner: None,
            constructors: Vec::new(),
            active: None,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    owner: Option<DeclarationIdentity>,
    constructors: Vec<&'a str>,
    active: Option<(Span, bool)>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let (name, type_params, members, representation) = match decl {
            TopLevelDecl::Class(node) => (node.name(), node.type_params(), node.members(), None),
            TopLevelDecl::MixinClass(node) => {
                (node.name(), node.type_params(), node.members(), None)
            }
            TopLevelDecl::ExtensionType(node) => (
                node.name(),
                node.type_params(),
                node.members(),
                Some(
                    node.primary_constructor()
                        .constructor_name()
                        .map_or("new", |name| &self.ctx.source[name.span().start..name.span().end]),
                ),
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        if !type_params.is_empty() {
            return StructuralTraversal::SkipChildren;
        }
        let Some(owner) = self.model.resolve_name(name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        self.owner = Some(owner);
        self.constructors.clear();
        self.constructors.extend(
            members
                .filter_map(|member| match member {
                    ClassMember::Constructor(constructor) => Some(
                        constructor
                            .constructor_name()
                            .map_or("new", |name| &self.ctx.source[name.span().start..name.span().end]),
                    ),
                    _ => None,
                })
                .chain(representation),
        );
        if self.constructors.is_empty() {
            self.constructors.push("new");
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let (name, return_type, body, is_static, generic) = match member {
            ClassMember::Method(method) => (
                method.name(),
                method.return_type(),
                method.body(),
                method.is_static(),
                !method.type_params().is_empty(),
            ),
            ClassMember::Getter(getter) => (
                getter.name(),
                getter.return_type(),
                getter.body(),
                getter.is_static(),
                false,
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        let (Some(return_type), Some(_body)) = (return_type, body) else {
            return StructuralTraversal::SkipChildren;
        };
        if !is_static || generic {
            return StructuralTraversal::SkipChildren;
        }
        let returns_owner = matches!(
            self.model.resolve_type(return_type),
            ResolvedType::Interface { identity, nullable: false, .. }
                if self.owner.as_ref() == Some(&identity)
        );
        if !returns_owner {
            return StructuralTraversal::SkipChildren;
        }
        self.active = Some((name.span(), false));
        StructuralTraversal::Continue
    }

    fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
        match parent {
            StructuralNode::MethodDecl(_) | StructuralNode::GetterDecl(_) => {
                matches!(child, StructuralNode::FunctionBody(_))
            }
            _ => true,
        }
    }

    fn exit_class_member(&mut self, _member: ClassMember<'a>) {
        if let Some((span, found)) = self.active.take()
            && found
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-constructors-over-static-methods",
                "Static method should be a constructor.",
                span,
            ));
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        let Some((_, found)) = self.active.as_mut() else {
            return StructuralTraversal::SkipChildren;
        };
        let owner = self.owner.as_ref().expect("eligible enclosing declaration");
        if let Some((identity, constructor, _)) =
            crate::lint::flutter::constructor_invocation(node, &self.model)
            && (matches!(node, Expr::New(_))
                || identity != *owner
                || self.constructors.contains(&constructor))
        {
            *found = identity == *owner;
            if *found {
                return StructuralTraversal::SkipChildren;
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
