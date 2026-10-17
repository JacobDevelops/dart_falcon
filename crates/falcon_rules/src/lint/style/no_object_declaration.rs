//! Flags class members declared with the type `Object` (or `Object?`).
//!
//! Typing a field or a method's return as `Object` discards nearly all static
//! information, forcing callers into casts or type checks and defeating the
//! type system's guarantees. A specific type documents the real contract, and
//! `dynamic` is the honest choice when a value genuinely may be anything. The
//! rule inspects only field types and the return types of methods, getters, and
//! operators; parameters, local variables, setters, and top-level declarations
//! are out of scope.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoObjectDeclaration;

impl Rule for NoObjectDeclaration {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

const MESSAGE: &str = "Avoid using the Object type. Use a specific type or dynamic instead.";

/// Returns true when the type is exactly `Object` (or `Object?`).
fn is_object_type(ty: DartType<'_>) -> bool {
    match ty {
        DartType::Named(named) => named
            .segments()
            .last()
            .map(|s| s.name() == "Object")
            .unwrap_or(false),
        _ => false,
    }
}

fn check_type(ty: Option<DartType<'_>>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(ty) = ty
        && is_object_type(ty)
    {
        diags.push(ctx.diagnostic("no-object-declaration", MESSAGE, ty.span()));
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

structural_visitor! {
impl<'a, 'ctx> StructuralRuleVisitor<'ctx> for Collector<'a, 'ctx> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'ctx>) -> StructuralTraversal {
        if matches!(
            decl,
            TopLevelDecl::Class(_)
                | TopLevelDecl::Mixin(_)
                | TopLevelDecl::MixinClass(_)
                | TopLevelDecl::Enum(_)
                | TopLevelDecl::Extension(_)
                | TopLevelDecl::ExtensionType(_)
        ) {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_member(&mut self, member: ClassMember<'ctx>) -> StructuralTraversal {
        let ty = match member {
            ClassMember::Field(f) => f.field_type(),
            ClassMember::Method(m) => m.return_type(),
            ClassMember::Getter(g) => g.return_type(),
            ClassMember::Operator(o) => o.return_type(),
            _ => None,
        };
        check_type(ty, &mut self.diags, self.ctx);
        StructuralTraversal::SkipChildren
    }
}
}
