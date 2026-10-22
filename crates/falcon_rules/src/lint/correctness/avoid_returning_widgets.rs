//! Disallow functions and methods that return a `Widget`.
//!
//! Flags a function, method, or getter whose return type is `Widget`,
//! `StatelessWidget`, or `StatefulWidget` (or a `Future`, `Stream`, or
//! `Completer` of one). Returning a widget from a helper instead of defining a
//! real widget class defeats Flutter's element tree: the returned subtree cannot
//! hold its own state, is rebuilt wholesale whenever the enclosing `build` runs,
//! and never appears as a distinct node in the widget inspector. Extract the
//! subtree into its own `StatelessWidget` or `StatefulWidget` so Flutter can
//! cache and rebuild it independently. The `build` method and `@override`
//! members are exempt, as is any return type naming a concrete widget subclass
//! such as `Container`.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidReturningWidgets;

impl Rule for AvoidReturningWidgets {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            parameters: TypeParameterScope::default(),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    parameters: TypeParameterScope,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if !decl.type_params().is_empty() {
            self.parameters.push(decl.type_params(), &self.model);
        }
        if let TopLevelDecl::Function(f) = decl {
            if let Some(ret_type) = f.return_type()
                && contains_widget_type(self.model.resolve_type_in(ret_type, &self.parameters))
            {
                self.diags.push(self.ctx.diagnostic("Functions should not return `Widget` directly; extract to a separate widget class.", f.span()));
            }
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) {
        if !decl.type_params().is_empty() {
            self.parameters.pop();
        }
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        check_class_member(member, &mut self.diags, self.ctx, &self.model, &mut self.parameters);
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_widget_type(dart_type: &ResolvedType) -> bool {
    matches!(dart_type.view(), ResolvedTypeView::Interface { identity, .. }
        if ["Widget", "StatelessWidget", "StatefulWidget"]
            .iter()
            .any(|name| identity.is_package("flutter", name)))
}

fn contains_widget_type(dart_type: ResolvedType) -> bool {
    is_widget_type(&dart_type)
        || (["Future", "Stream", "Completer"]
            .iter()
            .any(|name| dart_type.interface("dart:async", name))
            && dart_type
                .type_argument(0)
                .is_some_and(|type_argument| is_widget_type(&type_argument)))
}


fn check_class_member(
    member: ClassMember<'_>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    model: &SemanticModel<'_>,
    parameters: &mut TypeParameterScope,
) {
    match member {
        ClassMember::Method(m) => {
            if !m.type_params().is_empty() {
                parameters.push(m.type_params(), model);
            }
            if let Some(ret_type) = m.return_type()
                && contains_widget_type(model.resolve_type_in(ret_type, parameters))
                && m.name().name() != "build"
                && !crate::lint::support::annotations::has_override(m.annotations(), *model)
            {
                diags.push(ctx.diagnostic("Methods should not return `Widget` directly; extract to a separate widget class.", m.name().span()));
            }
            if !m.type_params().is_empty() {
                parameters.pop();
            }
        }
        ClassMember::Getter(g) => {
            if let Some(ret_type) = g.return_type()
                && contains_widget_type(model.resolve_type_in(ret_type, parameters))
                && g.name().name() != "build"
                && !crate::lint::support::annotations::has_override(g.annotations(), *model)
            {
                diags.push(ctx.diagnostic("Getters should not return `Widget` directly; extract to a separate widget class.", g.span()));
            }
        }
        _ => {}
    }
}
