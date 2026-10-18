//! Keeps notifier state behind Riverpod's observable state property.

use crate::lint::riverpod::is_identity;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidPublicNotifierProperties;

impl Rule for AvoidPublicNotifierProperties {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let (name, members) = match declaration {
            TopLevelDecl::Class(class) => (class.name().name(), class.members()),
            TopLevelDecl::MixinClass(class) => (class.name().name(), class.members()),
            _ => return StructuralTraversal::SkipChildren,
        };
        let (Some(identities), Some(signatures)) = (self.ctx.identities, self.ctx.signatures) else {
            return StructuralTraversal::SkipChildren;
        };
        let model = SemanticModel::new(self.ctx.file_path, self.ctx.parsed, identities, self.ctx.types);
        let Some(identity) = model.resolve_name(name) else {
            return StructuralTraversal::SkipChildren;
        };
        let notifier = |identity: &falcon_analyze::DeclarationIdentity| {
            is_identity(self.ctx, identity, &[
                "Notifier", "NotifierBase", "AsyncNotifier", "AsyncNotifierBase",
                "StreamNotifier", "StreamNotifierBase", "AutoDisposeNotifier",
                "AutoDisposeAsyncNotifier", "AutoDisposeStreamNotifier", "StateNotifier",
                "$Notifier", "$AsyncNotifier", "$StreamNotifier",
            ])
        };
        if !notifier(&identity) && signatures.supertype_matches(&identity, &notifier) != Truth::Yes {
            return StructuralTraversal::SkipChildren;
        }
        for member in members {
            match member {
                ClassMember::Field(field) if !field.is_static() => {
                    for variable in field.declarators() {
                        self.property(variable.name());
                    }
                }
                ClassMember::Getter(getter) if !getter.is_static() => self.property(getter.name()),
                ClassMember::Setter(setter) if !setter.is_static() => self.property(setter.name()),
                _ => {}
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

impl Collector<'_> {
    fn property(&mut self, name: Identifier<'_>) {
        if !name.name().starts_with('_') && !matches!(name.name(), "state" | "ref" | "mounted") {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-public-notifier-properties",
                "Expose notifier data through state rather than a public property.",
                name.span(),
            ));
        }
    }
}
