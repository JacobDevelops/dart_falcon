//! Require public widget constructors to accept or initialize a Flutter `Key`.
//! Primary headers supply parameters; their optional `this` bodies supply
//! constructor annotations and initializers.
//! Diagnostics underline the named constructor identifier, or the type name
//! for unnamed constructors, rather than modifiers or the constructor body.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedSignature, ResolvedType, Rule, RulePreparation,
    SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal, TypeParameterId,
    TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseKeyInWidgetConstructors;

impl Rule for UseKeyInWidgetConstructors {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            model,
            signatures,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
}

impl Collector<'_> {
    fn check_class(&mut self, class: ClassDecl<'_>) {
        if class.name().name().starts_with('_')
            || crate::lint::declarations::has_meta_annotation(
                class.annotations(),
                "visibleForTesting",
                &self.model,
            )
        {
            return;
        }
        let Some(identity) = self.model.resolve_name(class.name().name()) else {
            return;
        };
        let class_type = ResolvedType::interface_identity(identity.clone());
        let widget = DeclarationIdentity::flutter("Widget");
        let mut current = &class_type;
        let mut visited = HashSet::new();
        let extends_widget = loop {
            let ResolvedType::Interface { identity, .. } = current else {
                break false;
            };
            if !visited.insert(identity) {
                break false;
            }
            let Some(superclass) = self.signatures.superclass(identity) else {
                break false;
            };
            if matches!(superclass, ResolvedType::Interface { identity, .. } if identity.matches(&widget))
            {
                break true;
            }
            current = superclass;
        };
        if !extends_widget {
            return;
        }

        let primary_body = class.members().find_map(|member| match member {
            ClassMember::PrimaryConstructorBody(body) => Some(body),
            _ => None,
        });
        let primary = class.primary_constructor().into_iter().map(|constructor| {
            (
                false,
                primary_body.map(|body| body.annotations()),
                constructor.constructor_name(),
                constructor.params(),
                primary_body.map(|body| body.initializers()),
                constructor
                    .constructor_name()
                    .map_or(constructor.name_span(), |name| name.span()),
            )
        });
        let constructors = class.members().filter_map(|member| match member {
            ClassMember::Constructor(constructor) => Some((
                constructor.is_factory(),
                Some(constructor.annotations()),
                constructor.constructor_name(),
                constructor.params(),
                Some(constructor.initializers()),
                constructor
                    .constructor_name()
                    .map_or(constructor.name().span(), |name| name.span()),
            )),
            _ => None,
        });
        let mut constructors = primary.chain(constructors).peekable();
        if constructors.peek().is_none() {
            self.diagnostics.push(self.ctx.diagnostic("Public widgets should have a constructor with a named `key` parameter.", class.name().span()));
            return;
        }

        let mut parameters = TypeParameterScope::default();
        parameters.push(class.type_params(), &self.model);
        let supertype = class
            .extends()
            .map(|ty| self.model.resolve_type_in(ty, &parameters));
        for (is_factory, annotations, name, params, initializers, span) in constructors {
            if is_factory
                || annotations.is_some_and(|annotations| {
                    crate::lint::declarations::has_meta_annotation(
                        annotations,
                        "visibleForTesting",
                        &self.model,
                    )
                })
                || name.is_some_and(|name| name.name().starts_with('_'))
            {
                continue;
            }
            if self.has_resolved_super_key(params, initializers.clone(), supertype.as_ref()) {
                continue;
            }
            let mut known_target = false;
            let mut accepted = false;
            for initializer in initializers.clone().into_iter().flatten() {
                let (target, name, args) = match initializer {
                    ConstructorInitializer::SuperCall(call) => (
                        supertype.as_ref(),
                        call.call_name().map_or("new", |name| name.name()),
                        call.args(),
                    ),
                    ConstructorInitializer::ThisCall(call) => (
                        Some(&class_type),
                        call.call_name().map_or("new", |name| name.name()),
                        call.args(),
                    ),
                    _ => continue,
                };
                let Some(target) = target else {
                    continue;
                };
                let Some((signature, substitutions)) =
                    self.signatures.resolved_constructor(target, name)
                else {
                    continue;
                };
                known_target = true;
                if self.initializer_accepts_key(signature, &substitutions, args) {
                    accepted = true;
                    break;
                }
            }
            if accepted {
                continue;
            }
            if initializers.as_ref().is_none_or(AstChildren::is_empty) {
                let Some(supertype) = &supertype else {
                    continue;
                };
                let Some((signature, substitutions)) =
                    self.signatures.resolved_constructor(supertype, "new")
                else {
                    continue;
                };
                known_target = true;
                if !target_defines_key(signature, &substitutions, &self.model, self.signatures) {
                    continue;
                }
            }
            if known_target {
                self.diagnostics.push(self.ctx.diagnostic("Public widget constructors should accept and forward a named `key` parameter.", span));
            }
        }
    }

    fn initializer_accepts_key(
        &self,
        signature: &ResolvedSignature,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        args: ArgList<'_>,
    ) -> bool {
        !target_defines_key(signature, substitutions, &self.model, self.signatures)
            || args.named().any(|argument| argument.name().name() == "key")
    }

    fn has_resolved_super_key(
        &self,
        params: FormalParamList<'_>,
        initializers: Option<AstChildren<'_, ConstructorInitializer<'_>>>,
        supertype: Option<&ResolvedType>,
    ) -> bool {
        let Some(supertype) = supertype else {
            return false;
        };
        let constructor_name = initializers
            .into_iter()
            .flatten()
            .find_map(|initializer| match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    Some(call.call_name().map_or("new", |name| name.name()))
                }
                _ => None,
            })
            .unwrap_or("new");
        let Some((signature, substitutions)) = self
            .signatures
            .resolved_constructor(supertype, constructor_name)
        else {
            return false;
        };
        target_defines_key(signature, &substitutions, &self.model, self.signatures)
            && params.named().any(|parameter| {
                parameter.initializing() == Some(InitializingFormal::Super)
                    && parameter.name().name() == "key"
            })
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_class_decl(&mut self, node: ClassDecl<'a>) -> StructuralTraversal {
        self.check_class(node);
        StructuralTraversal::SkipChildren
    }
}
}

fn target_defines_key(
    signature: &ResolvedSignature,
    substitutions: &HashMap<TypeParameterId, ResolvedType>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    signature
        .named
        .get("key")
        .map(|ty| model.substitute(ty, substitutions))
        .is_some_and(|ty| {
            signatures.is_subtype_of(&ty, &DeclarationIdentity::flutter("Key"), model)
                == falcon_analyze::Truth::Yes
        })
}

#[cfg(test)]
mod tests {


    use super::*;

    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        crate::lint::test_support::analyze_flutter_rule(
            "use-key-in-widget-constructors",
            source,
            &UseKeyInWidgetConstructors,
        )
    }


    #[test]
    fn super_formal_uses_selected_named_constructor_and_owner_substitution() {
        let diagnostics = diagnostics(
            "import 'package:flutter/widgets.dart'; class _Base<T> extends StatelessWidget { _Base(); _Base.named({required T key}) : super(key: key); } class Child extends _Base<Key?> { Child({super.key}) : super.named(); }",
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn nullable_key_subtypes_require_forwarding() {
        for key in ["LocalKey?", "GlobalKey?"] {
            let source = format!(
                "import 'package:flutter/widgets.dart'; abstract class _Base extends Widget {{ const _Base({{{key} key}}) : super(key: key); }} abstract class Missing extends _Base {{ Missing(); }} abstract class Forwarded extends _Base {{ Forwarded({{super.key}}); }}"
            );
            let diagnostics = diagnostics(&source);
            assert_eq!(diagnostics.len(), 1, "{key}: {diagnostics:?}");
            assert!(source[diagnostics[0].span.start..diagnostics[0].span.end].contains("Missing"));
        }
    }

    #[test]
    fn constructor_diagnostics_select_name_tokens() {
        // Flutter 3.47.7 analyzer: columns 9 and 15, length 5.
        let source = r"import 'package:flutter/widgets.dart';
class Child extends StatelessWidget {
  const Child();
  const Child.named();
  @override
  Widget build(BuildContext context) => const SizedBox();
}
";
        let diagnostics = diagnostics(source);
        let spans: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let unnamed = source.find("Child();").unwrap();
        let named = source.find("named();").unwrap();
        assert_eq!(spans, [(unnamed, unnamed + 5), (named, named + 5)]);
    }

    #[test]
    fn visible_for_testing_exemption_requires_package_meta_identity() {
        let source = r"
import 'package:flutter/widgets.dart';
import 'package:meta/meta.dart' as meta;
const visibleForTesting = 0;
@visibleForTesting
abstract class FakeClass extends Widget { FakeClass(); }
abstract class FakeConstructor extends Widget {
  @visibleForTesting FakeConstructor();
}
@meta.visibleForTesting
abstract class RealClass extends Widget { RealClass(); }
abstract class RealConstructor extends Widget {
  @meta.visibleForTesting RealConstructor();
}
";
        let diagnostics = diagnostics(source);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    }


}
