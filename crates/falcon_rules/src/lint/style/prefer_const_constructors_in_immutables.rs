//! Prefers `const` constructors in immutable classes and extension types.
//!
//! Adopted from the official `prefer_const_constructors_in_immutables` lint.
//! Classes and mixin classes are checked only when they have no `with` clause;
//! mixin declarations and classes applying mixins are skipped. The declaration
//! must be annotated `@immutable` or inherit it through `extends` only, not
//! through `implements` or applied mixins. Extension types are also checked
//! when annotated `@immutable`.
//!
//! A non-const, non-primary constructor whose body is `;` is reported when:
//!
//! - it is a redirecting factory to a `const` constructor; or
//! - it is generative, delegates to a `const` constructor, has no non-final
//!   instance fields on its superclass/mixin chain, and every field initializer,
//!   initializer-list expression, assert, and `super`/`this` argument is
//!   potentially constant.
//!
//! Delegation follows an explicit `super` or `this` call, otherwise the
//! superclass's unnamed constructor (or an extension type's representation
//! constructor). Implicit default constructors are not `const`.
//! Ordinary constructor diagnostics cover their first token after metadata.
//!
//! Analyzer 3.13 class and mixin-class primary constructors use the same body,
//! delegation and field-finality checks, but do not reject nonconstant
//! initializer expressions. Immutable extension-type primary constructors need
//! only lack `const` and a block or expression body; even an empty block
//! disqualifies them, but delegation and field finality are not checked.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::const_eval::initializer_potentially_constant;
use crate::lint::declarations::{first_token_after_annotations, member_names};

#[derive(Default)]
pub struct PreferConstConstructorsInImmutables;

impl Rule for PreferConstConstructorsInImmutables {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (name, type_params, members, extends) = match decl {
                TopLevelDecl::Class(class) if class.with_clause.is_empty() => (
                    &class.name,
                    class.type_params.as_slice(),
                    class.element_members(),
                    class.extends.as_ref(),
                ),
                TopLevelDecl::MixinClass(class) if class.with_clause.is_empty() => (
                    &class.name,
                    class.type_params.as_slice(),
                    class.element_members(),
                    class.extends.as_ref(),
                ),
                TopLevelDecl::ExtensionType(extension) => (
                    &extension.name,
                    extension.type_params.as_slice(),
                    extension.element_members(),
                    None,
                ),
                _ => continue,
            };
            let Some(owner) = model.resolve_name(&name.name) else {
                continue;
            };
            if signatures.has_immutable_supertype(&owner, true) != Truth::Yes {
                continue;
            }
            let checker = ClassChecker {
                extends,
                representation_const: members
                    .primary
                    .filter(|_| members.representation)
                    .map(|primary| primary.is_const),
                owner: &owner,
                model: &model,
                signatures,
                shadowed: class_lexical_value_names(type_params, members),
                fields: class_field_initializers(members.declared),
            };
            for member in members.declared {
                let ClassMember::Constructor(constructor) = member else {
                    continue;
                };
                if constructor.is_const || constructor.body.is_some() {
                    continue;
                }
                let reportable = if constructor.is_factory {
                    checker.redirects_to_const(constructor)
                } else {
                    checker.delegates_to_const(&constructor.initializers)
                        && checker.can_be_const(&constructor.params, &constructor.initializers)
                };
                if reportable {
                    let start = first_token_after_annotations(
                        &constructor.annotations,
                        constructor.span,
                        ctx.syntax,
                    );
                    let end = ctx
                        .syntax
                        .tokens_in(Span::new(start, constructor.span.end))
                        .next()
                        .map_or(start, |token| token.offset() + token.len());
                    diags.push(ctx.diagnostic(
                        "prefer-const-constructors-in-immutables",
                        "Declare a const constructor in an immutable class.",
                        Span::new(start, end),
                    ));
                }
            }
            if let Some(primary) = members.primary {
                let has_body = members.declared.iter().any(|member| {
                    matches!(member, ClassMember::PrimaryConstructorBody(body) if body.body.is_some())
                });
                let constructor = members
                    .constructors()
                    .find(|constructor| constructor.is_primary)
                    .expect("primary constructor");
                if !constructor.is_const
                    && !has_body
                    && (members.representation
                        || (checker.delegates_to_const(constructor.initializers)
                            && signatures.has_non_final_field(&owner) == Some(false)))
                {
                    diags.push(
                        ctx.diagnostic(
                            "prefer-const-constructors-in-immutables",
                            "Declare a const constructor in an immutable class.",
                            Span::new(
                                name.span.start,
                                primary
                                    .constructor_name
                                    .as_ref()
                                    .map_or(name.span.end, |name| name.span.end),
                            ),
                        ),
                    );
                }
            }
        }
        diags
    }
}

struct ClassChecker<'a> {
    extends: Option<&'a DartType>,
    representation_const: Option<bool>,
    owner: &'a DeclarationIdentity,
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    /// Type parameters and member names, which shadow top-level names.
    shadowed: HashSet<String>,
    fields: HashMap<&'a str, &'a Expr>,
}

impl ClassChecker<'_> {
    /// `factory C() = D;` where `D`'s constructor is `const`.
    fn redirects_to_const(&self, constructor: &ConstructorDecl) -> bool {
        let Some(redirect) = &constructor.redirect else {
            return false;
        };
        if let Some(name) = &redirect.constructor_name {
            return self
                .is_const_constructor(&self.model.resolve_type(&redirect.type_), &name.name);
        }
        // `= D.named` parses as a prefixed type name.
        let DartType::Named(named) = &redirect.type_ else {
            return false;
        };
        let segments: Vec<&str> = named
            .segments
            .iter()
            .map(|segment| segment.name.as_str())
            .collect();
        if let Some(identity) = self.model.resolve_dotted(&segments) {
            return self.is_const_identity(&identity, "new");
        }
        let Some((name, class)) = segments.split_last() else {
            return false;
        };
        self.model
            .resolve_dotted(class)
            .is_some_and(|identity| self.is_const_identity(&identity, name))
    }

    /// Whether the constructor this one delegates to is `const`.
    fn delegates_to_const(&self, initializers: &[ConstructorInitializer]) -> bool {
        let superclass = || {
            self.extends.map_or_else(
                || self.model.core_type("Object", false),
                |ty| self.model.resolve_type(ty),
            )
        };
        for initializer in initializers {
            match initializer {
                ConstructorInitializer::SuperCall { call_name, .. } => {
                    return self.is_const_constructor(&superclass(), delegate(call_name));
                }
                ConstructorInitializer::ThisCall { call_name, .. } => {
                    return self
                        .signatures
                        .is_const_constructor(self.owner, delegate(call_name));
                }
                ConstructorInitializer::FieldInit { .. }
                | ConstructorInitializer::Assert { .. } => {}
            }
        }
        self.representation_const
            .unwrap_or_else(|| self.is_const_constructor(&superclass(), "new"))
    }

    fn is_const_constructor(&self, ty: &ResolvedType, name: &str) -> bool {
        let ResolvedType::Interface { identity, .. } = ty else {
            return false;
        };
        self.is_const_identity(identity, name)
    }

    fn is_const_identity(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        let is_object = identity.is_sdk("dart:core", "Object");
        (is_object && name == "new") || self.signatures.is_const_constructor(identity, name)
    }

    /// The analyzer's `canBeConst`: no non-final field on the chain and no
    /// constant-verifier error once the constructor is made `const`.
    fn can_be_const(
        &self,
        params: &FormalParamList,
        initializers: &[ConstructorInitializer],
    ) -> bool {
        if self.signatures.has_non_final_field(self.owner) != Some(false) {
            return false;
        }
        let no_parameters = HashSet::new();
        let field_initializers_constant = self.fields.values().all(|initializer| {
            initializer_potentially_constant(
                initializer,
                &no_parameters,
                &self.shadowed,
                self.owner,
                &self.fields,
                self.model,
                self.signatures,
            )
        });
        if !field_initializers_constant {
            return false;
        }
        let parameters: HashSet<String> = params
            .params
            .iter()
            .map(|parameter| parameter.name.name.clone())
            .collect();
        let mut shadowed = self.shadowed.clone();
        shadowed.extend(parameters.iter().cloned());
        let constant = |expr: &Expr| {
            initializer_potentially_constant(
                expr,
                &parameters,
                &shadowed,
                self.owner,
                &self.fields,
                self.model,
                self.signatures,
            )
        };
        initializers.iter().all(|initializer| match initializer {
            ConstructorInitializer::FieldInit { value, .. } => constant(value),
            ConstructorInitializer::Assert {
                condition, message, ..
            } => constant(condition) && message.as_ref().is_none_or(constant),
            ConstructorInitializer::SuperCall { args, .. }
            | ConstructorInitializer::ThisCall { args, .. } => args
                .positional
                .iter()
                .chain(args.named.iter().map(|argument| &argument.value))
                .all(constant),
        })
    }
}

fn class_lexical_value_names(
    type_params: &[TypeParam],
    members: EffectiveMembers<'_>,
) -> HashSet<String> {
    let mut names = member_names(members.declared);
    names.extend(
        type_params
            .iter()
            .map(|parameter| parameter.name.name.clone()),
    );
    names.extend(
        members
            .declaring_params()
            .map(|parameter| parameter.name.name.clone()),
    );
    names
}

/// Initializers of the class's instance fields.
fn class_field_initializers(members: &[ClassMember]) -> HashMap<&str, &Expr> {
    members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Field(field) if !field.is_static => Some(field),
            _ => None,
        })
        .flat_map(|field| {
            field.declarators.iter().filter_map(|declarator| {
                declarator
                    .initializer
                    .as_ref()
                    .map(|initializer| (declarator.name.name.as_str(), initializer))
            })
        })
        .collect()
}

/// The constructor a `super`/`this` call names; the unnamed one is `new`.
fn delegate(name: &Option<Identifier>) -> &str {
    name.as_ref().map_or("new", |name| name.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference as span_test_support;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn reports_constructor_first_token_without_named_suffix_or_parameters() {
        let source = r"
import 'package:meta/meta.dart';
@immutable
class Frozen {
  Frozen();
  @Deprecated('old')
  /* metadata gap */ Frozen.named();
}
@immutable
class External {
  external External();
  factory External.named() = Other;
}
class Other implements External { const Other(); }
";
        let diagnostics = span_test_support::analyze(
            &PreferConstConstructorsInImmutables,
            Path::new("test.dart"),
            source,
            &FalconConfig::default(),
        );
        assert_eq!(diagnostics.len(), 4);
        for (diagnostic, (declaration, expected)) in diagnostics.iter().zip([
            ("Frozen();", "Frozen"),
            ("Frozen.named();", "Frozen"),
            ("external External();", "external"),
            ("factory External.named()", "factory"),
        ]) {
            assert_eq!(diagnostic.span.start, source.find(declaration).unwrap());
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }
}
