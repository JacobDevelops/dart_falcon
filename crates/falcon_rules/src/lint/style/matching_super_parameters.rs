//! Flags positional super parameters named differently from the super
//! constructor's corresponding parameter.
//!
//! A positional `super.x` parameter is forwarded by position, so its name is
//! free — but giving it a different name from the parameter it fills in the
//! super constructor misleads readers about which value goes where.
//!
//! ```dart
//! class Rectangle {
//!   final int width, height;
//!   Rectangle(this.width, this.height);
//! }
//!
//! class ColoredRectangle extends Rectangle {
//!   ColoredRectangle(
//!     super.height,             // flagged: corresponds to `width`
//!     super.width,              // flagged: corresponds to `height`
//!   );
//! }
//! ```
//!
//! The super constructor is the one invoked by `super(...)` / `super.name(...)`
//! in the initializer list, or the superclass's unnamed constructor. The rule
//! stays silent when that constructor cannot be resolved.
//!
//! Adopted from the official `matching_super_parameters` lint.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MatchingSuperParameters;

impl Rule for MatchingSuperParameters {
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
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
            let TopLevelDecl::Class(class) = decl else {
                return StructuralTraversal::SkipChildren;
            };
            let Some(superclass) = class.extends() else {
                return StructuralTraversal::SkipChildren;
            };
            let ResolvedType::Interface { identity, .. } = self.model.resolve_type(superclass) else {
                return StructuralTraversal::SkipChildren;
            };
            let Some(facts) = self.signatures.declaration(&identity) else {
                return StructuralTraversal::SkipChildren;
            };
            for constructor in class.element_members().constructors() {
                let params = constructor.params;
                let mut initializers = constructor.initializers;
                let super_params = params
                    .params()
                    .filter(|param| {
                        param.initializing() == Some(InitializingFormal::Super)
                            && !matches!(param.kind(), ParamKind::Named { .. })
                    });
                let count = super_params.clone().count();
                if count == 0 {
                    continue;
                }
                let target = initializers
                    .find_map(|initializer| match initializer {
                        ConstructorInitializer::SuperCall(call) => {
                            Some(call.call_name().map_or("new", |name| name.name()))
                        }
                        _ => None,
                    })
                    .unwrap_or("new");
                let Some(super_constructor) = facts
                    .constructors
                    .iter()
                    .find(|candidate| candidate.name == target)
                else {
                    continue;
                };
                let super_positional = super_constructor
                    .parameters
                    .iter()
                    .filter(|parameter| !parameter.is_named)
                    .map(|parameter| parameter.name.as_str());
                if super_positional.clone().count() < count {
                    continue;
                }
                for (param, expected) in super_params.zip(super_positional) {
                    if param.name().name() != expected {
                        self.diags.push(self.ctx.diagnostic(
                    format!(
                        "The super parameter named '{}' does not share the same name as the corresponding parameter in the super constructor, '{expected}'.",
                        param.name().name()
                    ),
                    param.span(),
                ));
                    }
                }
            }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
