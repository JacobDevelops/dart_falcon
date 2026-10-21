//! Flags casting the parameter of `operator ==` instead of testing its type.
//!
//! `==` must accept any object and answer `false` for unrelated types. Casting
//! the parameter with `as` throws a `CastError` for those instead, so comparing
//! with an object of another type crashes. Test the type with `is` (or a
//! pattern) and return `false` when it does not match.
//!
//! ```dart
//! class Field {
//!   @override
//!   bool operator ==(Object other) {
//!     Field otherField = other as Field;  // flagged
//!     return value == otherField.value;
//!   }
//! }
//! ```
//!
//! Adopted from the official `test_types_in_equals` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::is_parenthesized;

#[derive(Default)]
pub struct TestTypesInEquals;

impl Rule for TestTypesInEquals {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(ParameterCasts {
            ctx,
            diagnostics: Vec::new(),
            declarations: program.declarations(),
            members: None,
            type_name: "",
            name: None,
        }))
    }
}

/// Collects bare `<parameter> as T` casts, not `(parameter) as T`.
struct ParameterCasts<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    declarations: AstChildren<'a, TopLevelDecl<'a>>,
    members: Option<AstChildren<'a, ClassMember<'a>>>,
    type_name: &'a str,
    name: Option<&'a str>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for ParameterCasts<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) -> StructuralTraversal {
        // Top-level callbacks follow the program's declaration order.
        let Some(node) = self.declarations.next() else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(members) = crate::lint::declarations::members_of(node) else {
            return StructuralTraversal::SkipChildren;
        };
        self.members = Some(members);
        self.type_name = match node {
            TopLevelDecl::Class(node) => node.name().name(),
            TopLevelDecl::MixinClass(node) => node.name().name(),
            TopLevelDecl::Enum(node) => node.name().name(),
            TopLevelDecl::Mixin(node) => node.name().name(),
            TopLevelDecl::Extension(node) => {
                let span = node.on_type().span();
                self.ctx.source().get(span.start..span.end).unwrap_or("unknown")
            }
            _ => "unknown",
        };
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, _node: ClassMember<'a>) -> StructuralTraversal {
        if let Some(ClassMember::Operator(op)) = self.members.as_mut().and_then(Iterator::next)
            && op.op() == "=="
            && op.body().is_some()
            && op.params().params().len() == 1
            && let Some(param) = op.params().params().first()
        {
            self.name = Some(param.name().name());
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn exit_class_member(&mut self, _node: ClassMember<'a>) {
        self.name = None;
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        if matches!(parent, StructuralNode::OperatorDecl(_)) {
            return matches!(child, StructuralNode::FunctionBody(_));
        }
        true
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Some(name) = self.name
            && let Expr::As(cast) = node
            && matches!(cast.expr(), Expr::Ident(identifier) if identifier.name() == name)
            && !is_parenthesized(cast.expr())
        {
            self.diagnostics.push(self.ctx.diagnostic(
                format!("Missing type test for '{}' in '=='.", self.type_name),
                cast.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
