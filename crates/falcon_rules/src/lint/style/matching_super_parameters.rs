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

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct MatchingSuperParameters;

impl Rule for MatchingSuperParameters {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let TopLevelDecl::Class(class) = decl else {
                continue;
            };
            let Some(superclass) = &class.extends else {
                continue;
            };
            let ResolvedType::Interface { identity, .. } = model.resolve_type(superclass) else {
                continue;
            };
            let Some(facts) = signatures.declaration(&identity) else {
                continue;
            };
            for (params, initializers) in constructor_inputs(class) {
                let super_params: Vec<&FormalParam> = params
                    .params
                    .iter()
                    .filter(|param| {
                        param.initializing == Some(InitializingFormal::Super)
                            && !matches!(param.kind, ParamKind::Named { .. })
                    })
                    .collect();
                if super_params.is_empty() {
                    continue;
                }
                let target = initializers
                    .iter()
                    .find_map(|initializer| match initializer {
                        ConstructorInitializer::SuperCall { call_name, .. } => {
                            Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
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
                let super_positional: Vec<&str> = super_constructor
                    .parameters
                    .iter()
                    .filter(|parameter| !parameter.is_named)
                    .map(|parameter| parameter.name.as_str())
                    .collect();
                if super_positional.len() < super_params.len() {
                    continue;
                }
                for (param, expected) in super_params.iter().zip(&super_positional) {
                    if param.name.name != *expected {
                        diags.push(ctx.diagnostic(
                    "matching-super-parameters",
                    format!(
                        "The super parameter named '{}' does not share the same name as the corresponding parameter in the super constructor, '{expected}'.",
                        param.name.name
                    ),
                    param.span,
                ));
                    }
                }
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn constructor_inputs(
    class: &ClassDecl,
) -> impl Iterator<Item = (&FormalParamList, &[ConstructorInitializer])> {
    let constructors = class.members.iter().filter_map(|member| match member {
        ClassMember::Constructor(constructor) => {
            Some((&constructor.params, constructor.initializers.as_slice()))
        }
        _ => None,
    });
    let primary = class.primary_constructor.as_ref().map(|constructor| {
        let initializers = class
            .members
            .iter()
            .find_map(|member| match member {
                ClassMember::PrimaryConstructorBody(body) => Some(body.initializers.as_slice()),
                _ => None,
            })
            .unwrap_or(&[]);
        (&constructor.params, initializers)
    });
    constructors.chain(primary)
}
