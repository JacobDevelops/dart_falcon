//! Require public widget constructors to accept or initialize a Flutter `Key`.
//! Primary headers supply parameters; their optional `this` bodies supply
//! constructor annotations and initializers.
//! Diagnostics underline the named constructor identifier, or the type name
//! for unnamed constructors, rather than modifiers or the constructor body.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedSignature, ResolvedType, Rule, RulePreparation,
    SemanticModel, SignatureIndex, TypeParameterId, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct UseKeyInWidgetConstructors;

impl Rule for UseKeyInWidgetConstructors {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            model,
            signatures,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diagnostics)
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
}

impl Collector<'_> {
    fn check_class(&mut self, class: &ClassDecl) {
        if class.name.name.starts_with('_')
            || crate::lint::declarations::has_meta_annotation(
                &class.annotations,
                "visibleForTesting",
                &self.model,
            )
        {
            return;
        }
        let Some(identity) = self.model.resolve_name(&class.name.name) else {
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
            if matches!(superclass, ResolvedType::Interface { identity, .. } if *identity == widget)
            {
                break true;
            }
            current = superclass;
        };
        if !extends_widget {
            return;
        }

        let primary_body = class.members.iter().find_map(|member| match member {
            ClassMember::PrimaryConstructorBody(body) => Some(body),
            _ => None,
        });
        let primary = class.primary_constructor.iter().map(|constructor| {
            (
                false,
                primary_body.map_or(&[][..], |body| body.annotations.as_slice()),
                constructor.constructor_name.as_ref(),
                &constructor.params,
                primary_body.map_or(&[][..], |body| body.initializers.as_slice()),
                constructor
                    .constructor_name
                    .as_ref()
                    .map_or(constructor.name_span, |name| name.span),
            )
        });
        let constructors = class.members.iter().filter_map(|member| match member {
            ClassMember::Constructor(constructor) => Some((
                constructor.is_factory,
                constructor.annotations.as_slice(),
                constructor.constructor_name.as_ref(),
                &constructor.params,
                constructor.initializers.as_slice(),
                constructor
                    .constructor_name
                    .as_ref()
                    .map_or(constructor.name.span, |name| name.span),
            )),
            _ => None,
        });
        let mut constructors = primary.chain(constructors).peekable();
        if constructors.peek().is_none() {
            self.diagnostics.push(self.ctx.diagnostic(
                "use-key-in-widget-constructors",
                "Public widgets should have a constructor with a named 'key' parameter.",
                class.name.span,
            ));
            return;
        }

        let mut parameters = TypeParameterScope::default();
        parameters.push(&class.type_params, &self.model);
        let supertype = class
            .extends
            .as_ref()
            .map(|ty| self.model.resolve_type_in(ty, &parameters));
        for (is_factory, annotations, name, params, initializers, span) in constructors {
            if is_factory
                || crate::lint::declarations::has_meta_annotation(
                    annotations,
                    "visibleForTesting",
                    &self.model,
                )
                || name.is_some_and(|name| name.name.starts_with('_'))
            {
                continue;
            }
            if self.has_resolved_super_key(params, initializers, supertype.as_ref()) {
                continue;
            }
            let mut known_target = false;
            let mut accepted = false;
            for initializer in initializers {
                let (target, name, args) = match initializer {
                    ConstructorInitializer::SuperCall {
                        call_name, args, ..
                    } => (
                        supertype.as_ref(),
                        call_name.as_ref().map_or("new", |name| name.name.as_str()),
                        args,
                    ),
                    ConstructorInitializer::ThisCall {
                        call_name, args, ..
                    } => (
                        Some(&class_type),
                        call_name.as_ref().map_or("new", |name| name.name.as_str()),
                        args,
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
            if initializers.is_empty() {
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
                self.diagnostics.push(self.ctx.diagnostic(
                    "use-key-in-widget-constructors",
                    "Public widget constructors should accept and forward a named 'key' parameter.",
                    span,
                ));
            }
        }
    }

    fn initializer_accepts_key(
        &self,
        signature: &ResolvedSignature,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        args: &ArgList,
    ) -> bool {
        !target_defines_key(signature, substitutions, &self.model, self.signatures)
            || args
                .named
                .iter()
                .any(|argument| argument.name.name == "key")
    }

    fn has_resolved_super_key(
        &self,
        params: &FormalParamList,
        initializers: &[ConstructorInitializer],
        supertype: Option<&ResolvedType>,
    ) -> bool {
        let Some(supertype) = supertype else {
            return false;
        };
        let constructor_name = initializers
            .iter()
            .find_map(|initializer| match initializer {
                ConstructorInitializer::SuperCall { call_name, .. } => {
                    Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
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
                parameter.initializing == Some(InitializingFormal::Super)
                    && parameter.name.name == "key"
            })
    }
}

impl Visitor for Collector<'_> {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.check_class(node);
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
    use falcon_analyze::{
        AnalysisEnvironment, ExternalLibraries, ExternalSources, IdentityIndex, IdentitySource,
        LibrarySource, RuleAnalysis, TypeIndex, canonical_or_lexical,
    };
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;

    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        analyze_rule(source, &UseKeyInWidgetConstructors)
    }

    fn analyze_rule(source: &str, rule: &dyn Rule) -> Vec<Diagnostic> {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let app = root.join("app");
        let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
        for path in [
            app.join(".dart_tool"),
            app.join("lib"),
            sdk.join("core"),
            root.join("flutter/packages/flutter/lib"),
            root.join("meta/lib"),
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(
            sdk.join("libraries.json"),
            r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            sdk.join("core/core.dart"),
            include_str!("../../../tests/support/sdk/lib/core/core.dart"),
        )
        .unwrap();
        std::fs::write(
            root.join("flutter/packages/flutter/lib/widgets.dart"),
            r"
abstract class Key { const Key.empty(); }
abstract class LocalKey extends Key { const LocalKey() : super.empty(); }
abstract class GlobalKey<T> extends Key { const GlobalKey() : super.empty(); }
abstract class Widget {
  const Widget({this.key});
  final Key? key;
}
abstract class StatelessWidget extends Widget { const StatelessWidget({super.key}); }
",
        )
        .unwrap();
        std::fs::write(
            root.join("meta/lib/meta.dart"),
            "class _Annotation { const _Annotation(); } const visibleForTesting = _Annotation(); const internal = _Annotation();",
        )
        .unwrap();
        std::fs::write(
            app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"flutterRoot":"../../flutter","packages":[
                {"name":"app","rootUri":"../","packageUri":"lib/"},
                {"name":"flutter","rootUri":"../../flutter/packages/flutter","packageUri":"lib/"},
                {"name":"meta","rootUri":"../../meta","packageUri":"lib/"}
            ]}"#,
        )
        .unwrap();
        let path = app.join("lib/main.dart");
        std::fs::write(&path, source).unwrap();
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let environment = AnalysisEnvironment::discover([path.as_path()]);
        assert_eq!(
            environment.sdk(&path).unwrap().root,
            canonical_or_lexical(&root.join("flutter/bin/cache/dart-sdk")),
        );
        let external = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), &program, source)],
            |candidate| candidate == path,
        );
        let identities = IdentityIndex::build(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        let mut types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        types.add_external(&external, &identities);
        let signatures = SignatureIndex::build(
            &[(path.clone(), &program)],
            Some(&external),
            &identities,
            &types,
        );
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_types(&types)
            .with_identities(&identities)
            .with_signatures(&signatures);
        rule.analyze(&program, &context)
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

    #[test]
    fn internal_constructor_exemption_requires_package_meta_identity() {
        let source = r"
import 'package:meta/meta.dart' as meta;
const internal = 0;
class _Private {}
@internal
class Fake { Fake(_Private value); }
@meta.internal
class Real { Real(_Private value); }
";
        let diagnostics = analyze_rule(
            source,
            &crate::lint::correctness::library_private_types_in_public_api::LibraryPrivateTypesInPublicApi,
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }
}
