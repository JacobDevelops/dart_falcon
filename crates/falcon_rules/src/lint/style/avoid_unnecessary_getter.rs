//! Flags a getter that only exposes a private final field.
//!
//! Replace the backing field and getter with a public final field. Getter/setter
//! pairs are covered separately by `unnecessary-getters-setters`. By default,
//! getters that upcast the field are retained; set `ignore-upcast` to false to
//! diagnose those too.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryGetter;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Ignore getters whose value is explicitly upcast to a broader type.
    ignore_upcast: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { ignore_upcast: true }
    }
}

crate::lint::options::impl_options!(Options);

impl Rule for AvoidUnnecessaryGetter {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let ignore_upcast =
            crate::lint::options::get::<Options>(ctx, Self::NAME).ignore_upcast;
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            ignore_upcast,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    ignore_upcast: bool,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        let Some(members) = crate::lint::declarations::members_of(node) else {
            return StructuralTraversal::Continue;
        };
        let name = match node {
            TopLevelDecl::Class(class) => class.name().name(),
            TopLevelDecl::MixinClass(class) => class.name().name(),
            TopLevelDecl::Mixin(class) => class.name().name(),
            TopLevelDecl::Enum(class) => class.name().name(),
            _ => return StructuralTraversal::Continue,
        };
        let Some(identity) = self.model.resolve_name(name) else { return StructuralTraversal::Continue };
        let receiver = ResolvedType::interface_identity(identity);
        let members: Vec<_> = members.collect();
        for member in &members {
            let ClassMember::Getter(getter) = member else { continue };
            if getter.is_static() || getter.name().name().starts_with('_') || members.iter().any(|member| matches!(member,
                ClassMember::Setter(setter) if setter.name().name() == getter.name().name())) {
                continue;
            }
            let Some(name) = crate::lint::support::field_accessors::getter_returns_field(*getter) else { continue };
            if !name.starts_with('_') { continue; }
            let field = members.iter().find_map(|member| match member {
                ClassMember::Field(field) if !field.is_static()
                    && field.declarators().any(|variable| variable.name().name() == name) => Some(*field),
                _ => None,
            });
            if !field.is_some_and(|field| field.keyword() == VarKeyword::Final
                && (!field.is_late() || field.declarators().all(|variable| variable.initializer().is_some()))) { continue; }
            if self.ignore_upcast {
                let (Some((field_type, field_substitutions)), Some((getter_type, getter_substitutions))) = (
                    self.signatures.resolved_field(&receiver, name, &self.model),
                    self.signatures.resolved_field(&receiver, getter.name().name(), &self.model),
                ) else { continue };
                let field_type = self.model.substitute(&field_type, &field_substitutions);
                let getter_type = self.model.substitute(&getter_type, &getter_substitutions);
                if matches!(field_type.view(), ResolvedTypeView::Unknown) || field_type != getter_type { continue; }
            }
            self.diagnostics.push(self.ctx.diagnostic(
                "This getter only exposes a final field; replace both with a public final field.",
                getter.name().span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
