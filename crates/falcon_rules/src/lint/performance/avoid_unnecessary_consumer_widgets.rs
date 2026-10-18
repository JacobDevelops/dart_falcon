//! Flags Riverpod consumer widgets whose build reference is unused.
//! A direct ConsumerWidget or HookConsumerWidget subclass can use a regular
//! widget when it does not need its WidgetRef. References are matched to the
//! parameter declaration, so nested consumers with their own ref do not count.
//! Abstract/generic classes, mixins, interfaces, missing bodies and uncertain
//! reference types are excluded, as are locally subclassed widgets and helpers
//! that use a WidgetRef parameter.

use crate::lint::riverpod::is_exact_type;
use falcon_analyze::resolve::lexical_usage::used_parameters;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryConsumerWidgets;

impl Rule for AvoidUnnecessaryConsumerWidgets {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types);
        let subclassed = std::iter::once(program)
            .chain(
                ctx.library
                    .into_iter()
                    .flat_map(|library| library.siblings().iter().copied()),
            )
            .flat_map(|program| program.declarations())
            .filter_map(|declaration| match declaration {
                TopLevelDecl::Class(class) => class.extends(),
                TopLevelDecl::ClassTypeAlias(alias) => Some(alias.superclass()),
                _ => None,
            })
            .filter_map(|ty| match model.resolve_type(ty) {
                ResolvedType::Interface { identity, .. } => Some(identity),
                _ => None,
            })
            .collect();
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            subclassed,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    subclassed: Vec<DeclarationIdentity>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn helper_uses_ref(&self, method: MethodDecl<'_>) -> bool {
        if method.is_static() {
            return false;
        }
        let candidate = |param: FormalParam<'_>| {
            param.name().name() == "ref"
                || param.param_type().is_some_and(|ty| {
                    is_exact_type(self.ctx, &self.model.resolve_type(ty), &["WidgetRef"])
                })
        };
        if !method.params().params().any(candidate) {
            return false;
        }
        let Some(body) = method.body() else {
            return true;
        };
        if matches!(body, FunctionBody::Native(_)) || !method.type_params().is_empty() {
            return true;
        }
        let used = used_parameters(method.params(), body);
        method
            .params()
            .params()
            .any(|param| candidate(param) && used.contains(&param.name().span().start))
    }

    fn check_class(&mut self, class: ClassDecl<'_>) {
        if class.modifiers().is_abstract
            || class.modifiers().restriction == Some(ClassRestriction::Sealed)
            || !class.type_params().is_empty()
            || !class.with_clause().is_empty()
            || !class.implements().is_empty()
            || self
                .subclassed
                .iter()
                .any(|identity| identity.name() == class.name().name())
        {
            return;
        }
        let Some(superclass) = class.extends() else {
            return;
        };
        let superclass = self.model.resolve_type(superclass);
        if !is_exact_type(
            self.ctx,
            &superclass,
            &["ConsumerWidget", "HookConsumerWidget"],
        ) {
            return;
        }
        let mut build = None;
        for member in class.members() {
            match member {
                ClassMember::Method(method) if method.name().name() == "build" => {
                    if build.is_some() {
                        return;
                    }
                    build = Some(method);
                }
                ClassMember::Method(method) if self.helper_uses_ref(method) => return,
                ClassMember::Error(_) => return,
                _ => {}
            }
        }
        let Some(build) = build.filter(|method| {
            !method.is_static() && !method.is_external() && method.type_params().is_empty()
        }) else {
            return;
        };
        let Some(body) = build
            .body()
            .filter(|body| !matches!(body, FunctionBody::Native(_)))
        else {
            return;
        };
        let mut params = build.params().params();
        let (Some(_context), Some(reference), None) = (params.next(), params.next(), params.next())
        else {
            return;
        };
        if !matches!(reference.kind(), ParamKind::Positional)
            || !reference.param_type().is_some_and(|ty| {
                is_exact_type(self.ctx, &self.model.resolve_type(ty), &["WidgetRef"])
            })
            || used_parameters(build.params(), body).contains(&reference.name().span().start)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-unnecessary-consumer-widgets",
            if is_exact_type(self.ctx, &superclass, &["HookConsumerWidget"]) {
                "This widget does not use its ref; convert it to a HookWidget."
            } else {
                "This widget does not use its ref; convert it to a StatelessWidget."
            },
            class.name().span(),
        ));
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Class(class) = declaration {
            self.check_class(class);
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
