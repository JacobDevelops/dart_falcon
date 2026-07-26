//! Require public widget constructors to accept or initialize a Flutter `Key`.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedSignature, ResolvedType, Rule, SemanticModel,
    SignatureIndex, TypeEnvironment, TypeParameterId, TypeParameterScope, TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, for_each_expr, walk_program};

pub struct UseKeyInWidgetConstructors;

impl Rule for UseKeyInWidgetConstructors {
    fn name(&self) -> &'static str {
        "use-key-in-widget-constructors"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            model,
            signatures,
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    file: String,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
}

impl Collector<'_> {
    fn check_class(&mut self, class: &ClassDecl) {
        if class.name.name.starts_with('_') {
            return;
        }
        let Some(identity) = self
            .model
            .resolve_name(std::slice::from_ref(&class.name.name))
        else {
            return;
        };
        let class_type = ResolvedType::Interface {
            identity: identity.clone(),
            arguments: Vec::new(),
            nullable: false,
            extension_type: false,
        };
        let widget = flutter_identity("Widget");
        if identity == widget
            || self
                .signatures
                .is_subtype_of(&class_type, &widget, &self.model)
                != TypeTruth::Yes
        {
            return;
        }

        let constructors = class
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Constructor(constructor) => Some(constructor),
                _ => None,
            })
            .collect::<Vec<_>>();
        if constructors.is_empty() {
            self.report(
                &class.name.span,
                "Public widgets should have a constructor with a named 'key' parameter.",
            );
            return;
        }

        let mut parameters = TypeParameterScope::default();
        parameters.push(&class.type_params, &self.model);
        let supertype = class
            .extends
            .as_ref()
            .map(|ty| self.model.resolve_type_in(ty, &parameters));
        for constructor in constructors {
            if constructor.is_factory
                || constructor
                    .constructor_name
                    .as_ref()
                    .is_some_and(|name| name.name.starts_with('_'))
            {
                continue;
            }
            if self.has_resolved_super_key(constructor, supertype.as_ref()) {
                continue;
            }
            let mut known_target = false;
            let mut accepted = false;
            for initializer in &constructor.initializers {
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
                if self.initializer_accepts_key(
                    &signature,
                    &substitutions,
                    args,
                    constructor,
                    &parameters,
                ) {
                    accepted = true;
                    break;
                }
            }
            if accepted {
                continue;
            }
            if constructor.initializers.is_empty() {
                let Some(supertype) = &supertype else {
                    continue;
                };
                let Some((signature, substitutions)) =
                    self.signatures.resolved_constructor(supertype, "new")
                else {
                    continue;
                };
                known_target = true;
                if !target_defines_key(&signature, &substitutions, &self.model) {
                    continue;
                }
            }
            if known_target {
                self.report(
                    &constructor.span,
                    "Public widget constructors should accept and forward a named 'key' parameter.",
                );
            }
        }
    }

    fn initializer_accepts_key(
        &self,
        signature: &ResolvedSignature,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        args: &ArgList,
        constructor: &ConstructorDecl,
        type_parameters: &TypeParameterScope,
    ) -> bool {
        if !target_defines_key(signature, substitutions, &self.model) {
            return true;
        }
        let Some(argument) = args
            .named
            .iter()
            .find(|argument| argument.name.name == "key")
        else {
            return false;
        };
        let mut environment = TypeEnvironment::new();
        environment.bind_params(&constructor.params, &self.model, type_parameters);
        let argument_type = environment.infer_with_signatures(
            &argument.value,
            &self.model,
            self.signatures,
            type_parameters,
        );
        if is_flutter_key(&argument_type) {
            return true;
        }
        let named_key = constructor.params.named.iter().any(|parameter| {
            parameter.name.name == "key"
                && parameter
                    .param_type
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, type_parameters))
                    .is_some_and(|ty| is_flutter_key(&ty))
        });
        if !named_key {
            return false;
        }
        let mut uses_key = false;
        for_each_expr(&argument.value, &mut |expression| {
            uses_key |= matches!(expression, Expr::Ident(identifier) if identifier.name == "key");
        });
        uses_key
    }

    fn has_resolved_super_key(
        &self,
        constructor: &ConstructorDecl,
        supertype: Option<&ResolvedType>,
    ) -> bool {
        let Some(supertype) = supertype else {
            return false;
        };
        let constructor_name = constructor
            .initializers
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
        target_defines_key(&signature, &substitutions, &self.model)
            && constructor
                .params
                .named
                .iter()
                .any(|parameter| parameter.is_super && parameter.name.name == "key")
    }

    fn report(&mut self, span: &Span, message: &str) {
        self.diagnostics.push(Diagnostic::new(
            "use-key-in-widget-constructors",
            Severity::Warning,
            message,
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.check_class(node);
    }
}

fn target_defines_key(
    signature: &ResolvedSignature,
    substitutions: &HashMap<TypeParameterId, ResolvedType>,
    model: &SemanticModel<'_>,
) -> bool {
    signature
        .named
        .get("key")
        .map(|ty| model.substitute(ty, substitutions))
        .is_some_and(|ty| is_flutter_key(&ty))
}

fn is_flutter_key(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Package { package, name },
            ..
        } if package == "flutter" && name == "Key"
    )
}

fn flutter_identity(name: &str) -> DeclarationIdentity {
    DeclarationIdentity::Package {
        package: "flutter".to_string(),
        name: name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;

    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        let path = PathBuf::from("/project/lib/main.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let sources = [IdentitySource {
            path: &path,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config)
            .with_types(&types)
            .with_identities(&identities)
            .with_signatures(&signatures);
        UseKeyInWidgetConstructors.analyze(&program, &context)
    }

    #[test]
    fn super_formal_uses_selected_named_constructor_and_owner_substitution() {
        let diagnostics = diagnostics(
            "import 'package:flutter/widgets.dart'; class _Base<T> extends StatelessWidget { _Base(); _Base.named({required T key}) : super(key: key); } class Child extends _Base<Key?> { Child({super.key}) : super.named(); }",
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn generic_constructor_key_type_is_substituted_before_validation() {
        let diagnostics = diagnostics(
            "import 'package:flutter/widgets.dart'; class _Base<T> extends StatelessWidget { _Base.named({required T key}) : super(key: key); } class Child extends _Base<Key> { Child() : super.named(key: null); }",
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }
}
