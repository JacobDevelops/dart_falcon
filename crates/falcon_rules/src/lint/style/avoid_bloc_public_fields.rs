//! Keep package-confirmed Bloc instance fields private; inherited API overrides are allowed.
use crate::lint::bloc::BlocTypes;
use falcon_analyze::{
    AnalyzeContext, MemberKind, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidBlocPublicFields;
impl Rule for AvoidBlocPublicFields {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, false)
    }
}

pub(super) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    methods: bool,
) -> RulePreparation<'a> {
    let Some(identities) = ctx.identities() else {
        return RulePreparation::Diagnostics(Vec::new());
    };
    RulePreparation::Structural(Box::new(Collector {
        ctx,
        model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
        types: BlocTypes::new(program),
        methods,
        diagnostics: Vec::new(),
    }))
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    types: BlocTypes<'a>,
    methods: bool,
    diagnostics: Vec<Diagnostic>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let TopLevelDecl::Class(class) = declaration else { return StructuralTraversal::SkipChildren; };
        // Cubits intentionally expose methods as their command interface.
        let applicable = if self.methods {
            self.types.is_event_bloc_class(class, &self.model, self.ctx.signatures())
        } else {
            self.types.is_bloc_class(class, &self.model, self.ctx.signatures())
        };
        if !applicable { return StructuralTraversal::SkipChildren; }
        let owner = self.model.resolve_name(class.name().name());
        let inherited = |name: &str, kind| owner.as_ref().is_some_and(|owner| {
            self.ctx.signatures().is_some_and(|signatures| signatures.has_combined_inherited_member(owner, name, kind, &self.model))
        });
        for member in class.members() {
            if self.methods {
                if let ClassMember::Method(method) = member
                    && !method.is_static()
                    && !method.name().name().starts_with('_')
                    && !crate::lint::declarations::annotation(method.annotations(), "override")
                    && !inherited(method.name().name(), MemberKind::Method)
                {
                    self.diagnostics.push(self.ctx.diagnostic("Keep Bloc command methods private; expose events instead.", method.name().span()));
                }
            } else if let ClassMember::Field(field) = member
                && !field.is_static()
                && !crate::lint::declarations::annotation(field.annotations(), "override")
            {
                for variable in field.declarators() {
                    if !variable.name().name().starts_with('_')
                        && !inherited(variable.name().name(), MemberKind::field(field, variable))
                    {
                        self.diagnostics.push(self.ctx.diagnostic("Keep Bloc instance fields private; expose state instead.", variable.name().span()));
                    }
                }
            }
        }
        StructuralTraversal::SkipChildren
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
