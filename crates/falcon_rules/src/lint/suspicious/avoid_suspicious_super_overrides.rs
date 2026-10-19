//! Finds getters hiding fields supplied via the immediate super constructor's
//! `this.` formals or declaring primary-constructor parameters.
//! Initializer-list assignments and forwarding through intermediate classes are not tracked.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, ResolvedType, Rule, RulePreparation,
    SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct AvoidSuspiciousSuperOverrides;

impl Rule for AvoidSuspiciousSuperOverrides {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types),
            signatures,
            owner: None,
            passed_fields: HashSet::new(),
            active: None,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    owner: Option<DeclarationIdentity>,
    passed_fields: HashSet<&'a str>,
    active: Option<(Identifier<'a>, bool)>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.owner = None;
        self.passed_fields.clear();
        let members = match node {
            TopLevelDecl::Class(class) => class.element_members(),
            TopLevelDecl::MixinClass(class) => class.element_members(),
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(declaration) = crate::lint::inherited_members::inheriting_declaration(node) else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(owner) = self.model.resolve_name(declaration.name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(ResolvedType::Interface { identity: parent, .. }) = self.signatures.superclass(&owner) else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(parent_facts) = self.signatures.declaration(parent) else {
            return StructuralTraversal::SkipChildren;
        };
        for constructor in members.constructors().filter(|constructor| !constructor.is_factory) {
            let call = constructor.initializers.clone().find_map(|initializer| match initializer {
                ConstructorInitializer::SuperCall(call) => Some(call), _ => None,
            });
            let name = call.and_then(SuperCallInitializer::call_name).map_or("new", Identifier::name);
            let Some(parent_constructor) = parent_facts.constructors.iter().find(|constructor| constructor.name == name) else {
                continue;
            };
            let mut position = 0;
            for parameter in &parent_constructor.parameters {
                let supplied = if parameter.is_named {
                    call.is_some_and(|call| call.args().named().any(|argument| argument.name().name() == parameter.name))
                        || constructor.params.params().any(|formal| formal.initializing() == Some(InitializingFormal::Super) && matches!(formal.kind(), ParamKind::Named { .. }) && formal.name().name() == parameter.name)
                } else {
                    let index = position;
                    position += 1;
                    call.is_some_and(|call| call.args().positional().nth(index).is_some())
                        || constructor.params.params().filter(|formal| formal.initializing() == Some(InitializingFormal::Super) && matches!(formal.kind(), ParamKind::Positional | ParamKind::OptionalPositional)).nth(index).is_some()
                };
                if parameter.is_field && supplied {
                    self.passed_fields.insert(parameter.name.as_str());
                }
            }
        }
        self.owner = Some(owner);
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, getter: GetterDecl<'a>) -> StructuralTraversal {
        self.active = None;
        if getter.is_static() || getter.body().is_none() || !self.passed_fields.contains(getter.name().name()) {
            return StructuralTraversal::SkipChildren;
        }
        let Some(owner) = &self.owner else { return StructuralTraversal::SkipChildren; };
        if self.signatures.inherited_concrete_member(owner, getter.name().name()).flatten()
            .is_some_and(|member| matches!(member.kind, MemberKind::Field { .. }))
        {
            self.active = Some((getter.name(), false));
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, expression: Expr<'a>) -> StructuralTraversal {
        if let Some((name, referenced)) = &mut self.active
            && let Expr::Field(field) = expression
            && matches!(field.object().unparenthesized(), Expr::Super(_))
            && field.field().name() == name.name()
        { *referenced = true; }
        StructuralTraversal::Continue
    }

    fn exit_getter_decl(&mut self, _getter: GetterDecl<'a>) {
        if let Some((name, false)) = self.active.take() {
            self.diagnostics.push(self.ctx.diagnostic("avoid-suspicious-super-overrides", "This getter hides a field already supplied to the super constructor; remove it or reference the super field.", name.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
