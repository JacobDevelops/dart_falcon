//! Flags class members declared with the type `Object` (or `Object?`).
//!
//! Typing a field or a method's return as `Object` discards nearly all static
//! information, forcing callers into casts or type checks and defeating the
//! type system's guarantees. A specific type documents the real contract, and
//! `dynamic` is the honest choice when a value genuinely may be anything. The
//! rule inspects only field types and the return types of methods, getters, and
//! operators; parameters, local variables, setters, and top-level declarations
//! are out of scope.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_top_level_decl};

#[derive(Default)]
pub struct NoObjectDeclaration;

impl Rule for NoObjectDeclaration {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut visitor = Collector {
            diags: Vec::new(),
            ctx,
        };
        visitor.visit_program(program);
        visitor.diags
    }
}

const MESSAGE: &str = "Avoid using the Object type. Use a specific type or dynamic instead.";

/// Returns true when the type is exactly `Object` (or `Object?`).
fn is_object_type(ty: &DartType) -> bool {
    match ty {
        DartType::Named(named) => named
            .segments
            .last()
            .map(|s| s.name == "Object")
            .unwrap_or(false),
        _ => false,
    }
}

fn check_type(ty: Option<&DartType>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(ty) = ty
        && is_object_type(ty)
    {
        diags.push(ctx.diagnostic("no-object-declaration", MESSAGE, *(ty.span())));
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) {
        if matches!(
            decl,
            TopLevelDecl::Class(_)
                | TopLevelDecl::Mixin(_)
                | TopLevelDecl::MixinClass(_)
                | TopLevelDecl::Enum(_)
                | TopLevelDecl::Extension(_)
                | TopLevelDecl::ExtensionType(_)
        ) {
            walk_top_level_decl(self, decl);
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember) {
        let ty = match member {
            ClassMember::Field(f) => f.field_type.as_ref(),
            ClassMember::Method(m) => m.return_type.as_ref(),
            ClassMember::Getter(g) => g.return_type.as_ref(),
            ClassMember::Operator(o) => o.return_type.as_ref(),
            _ => None,
        };
        check_type(ty, &mut self.diags, self.ctx);
    }
}
