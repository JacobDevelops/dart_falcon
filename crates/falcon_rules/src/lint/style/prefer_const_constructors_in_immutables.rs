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
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::const_eval::initializer_potentially_constant;
use crate::lint::declarations::{first_token_after_annotations, member_names};

#[derive(Default)]
pub struct PreferConstConstructorsInImmutables;

impl Rule for PreferConstConstructorsInImmutables {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            declarations: program.declarations(),
            scope: None,
            model,
            signatures,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: AstChildren<'a, TopLevelDecl<'a>>,
    scope: Option<ClassScope<'a>>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diagnostics: Vec<Diagnostic>,
}

struct ClassScope<'a> {
    name: Identifier<'a>,
    members: EffectiveMembers<'a>,
    extends: Option<DartType<'a>>,
    representation_const: Option<bool>,
    owner: DeclarationIdentity,
    shadowed: HashSet<&'a str>,
    fields: HashMap<&'a str, Expr<'a>>,
}

impl<'a> ClassScope<'a> {
    fn checker<'s>(
        &'s self,
        model: &'s SemanticModel<'a>,
        signatures: &'s SignatureIndex,
    ) -> ClassChecker<'s, 'a> {
        ClassChecker {
            extends: self.extends,
            representation_const: self.representation_const,
            owner: &self.owner,
            model,
            signatures,
            shadowed: &self.shadowed,
            fields: &self.fields,
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let decl = self.declarations.next().expect("shared declaration order");
        debug_assert_eq!(decl.syntax().id(), declaration.syntax().id());
        self.scope = None;
        let (name, type_params, members, extends) = match decl {
            TopLevelDecl::Class(class) if class.with_clause().is_empty() => (
                class.name(),
                class.type_params(),
                class.element_members(),
                class.extends(),
            ),
            TopLevelDecl::MixinClass(class) if class.with_clause().is_empty() => (
                class.name(),
                class.type_params(),
                class.element_members(),
                class.extends(),
            ),
            TopLevelDecl::ExtensionType(extension) => (
                extension.name(),
                extension.type_params(),
                extension.element_members(),
                None,
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(owner) = self.model.resolve_name(name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        if self.signatures.has_immutable_supertype(&owner, true) != Truth::Yes {
            return StructuralTraversal::SkipChildren;
        }
        self.scope = Some(ClassScope {
            name,
            extends,
            representation_const: members.primary
                .filter(|_| members.representation)
                .map(|primary| primary.is_const()),
            owner,
            shadowed: class_lexical_value_names(type_params, members.clone()),
            fields: class_field_initializers(members.declared.clone()),
            members,
        });
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let ClassMember::Constructor(constructor) = member else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(scope) = &self.scope else {
            return StructuralTraversal::SkipChildren;
        };
        if constructor.is_const() || constructor.body().is_some() {
            return StructuralTraversal::SkipChildren;
        }
        let checker = scope.checker(&self.model, self.signatures);
        let reportable = if constructor.is_factory() {
            checker.redirects_to_const(constructor)
        } else {
            checker.delegates_to_const(constructor.initializers())
                && checker.can_be_const(constructor.params(), constructor.initializers())
        };
        if reportable {
            let start = first_token_after_annotations(
                constructor.annotations(),
                constructor.span(),
                self.ctx.tree(),
                self.ctx.source,
            );
            let end = self.ctx.tree()
                .tokens_in(self.ctx.source, Span::new(start, constructor.span().end))
                .next()
                .map_or(start, |token| token.offset() + token.len());
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-const-constructors-in-immutables",
                "Declare a const constructor in an immutable class.",
                Span::new(start, end),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn exit_top_level_decl(&mut self, _decl: TopLevelDecl<'a>) {
        let Some(scope) = self.scope.take() else {
            return;
        };
        let Some(primary) = scope.members.primary else {
            return;
        };
        let has_body = scope.members.declared.clone().any(|member| {
            matches!(member, ClassMember::PrimaryConstructorBody(body) if body.body().is_some())
        });
        let constructor = scope.members.constructors()
            .find(|constructor| constructor.is_primary)
            .expect("primary constructor");
        let checker = scope.checker(&self.model, self.signatures);
        if !constructor.is_const
            && !has_body
            && (scope.members.representation
                || (checker.delegates_to_const(constructor.initializers)
                    && self.signatures.has_non_final_field(&scope.owner) == Some(false)))
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-const-constructors-in-immutables",
                "Declare a const constructor in an immutable class.",
                Span::new(
                    scope.name.span().start,
                    primary.constructor_name()
                        .map_or(scope.name.span().end, |name| name.span().end),
                ),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

struct ClassChecker<'s, 'a> {
    extends: Option<DartType<'a>>,
    representation_const: Option<bool>,
    owner: &'s DeclarationIdentity,
    model: &'s SemanticModel<'a>,
    signatures: &'s SignatureIndex,
    /// Type parameters and member names, which shadow top-level names.
    shadowed: &'s HashSet<&'a str>,
    fields: &'s HashMap<&'a str, Expr<'a>>,
}

impl<'a> ClassChecker<'_, 'a> {
    /// `factory C() = D;` where `D`'s constructor is `const`.
    fn redirects_to_const(&self, constructor: ConstructorDecl<'a>) -> bool {
        let Some(redirect) = constructor.redirect() else {
            return false;
        };
        if let Some(name) = redirect.constructor_name() {
            return self
                .is_const_constructor(&self.model.resolve_type(redirect.type_()), name.name());
        }
        // `= D.named` parses as a prefixed type name.
        let DartType::Named(named) = redirect.type_() else {
            return false;
        };
        let segments: Vec<&str> = named.segments().map(|segment| segment.name()).collect();
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
    fn delegates_to_const(
        &self,
        initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    ) -> bool {
        let superclass = || {
            self.extends.map_or_else(
                || self.model.core_type("Object", false),
                |ty| self.model.resolve_type(ty),
            )
        };
        for initializer in initializers {
            match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    return self.is_const_constructor(&superclass(), delegate(call.call_name()));
                }
                ConstructorInitializer::ThisCall(call) => {
                    return self
                        .signatures
                        .is_const_constructor(self.owner, delegate(call.call_name()));
                }
                ConstructorInitializer::FieldInit(_) | ConstructorInitializer::Assert(_) => {}
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
        params: FormalParamList<'a>,
        mut initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    ) -> bool {
        if self.signatures.has_non_final_field(self.owner) != Some(false) {
            return false;
        }
        let no_parameters = HashSet::new();
        let field_initializers_constant = self.fields.values().all(|initializer| {
            initializer_potentially_constant(
                *initializer,
                &no_parameters,
                self.shadowed,
                self.owner,
                self.fields,
                self.model,
                self.signatures,
            )
        });
        if !field_initializers_constant {
            return false;
        }
        let parameters: HashSet<&str> = params
            .params()
            .map(|parameter| parameter.name().name())
            .collect();
        let mut shadowed = (*self.shadowed).clone();
        shadowed.extend(parameters.iter().copied());
        let constant = |expr: Expr<'a>| {
            initializer_potentially_constant(
                expr,
                &parameters,
                &shadowed,
                self.owner,
                self.fields,
                self.model,
                self.signatures,
            )
        };
        initializers.all(|initializer| match initializer {
            ConstructorInitializer::FieldInit(field) => constant(field.value()),
            ConstructorInitializer::Assert(assertion) => {
                constant(assertion.condition()) && assertion.message().is_none_or(constant)
            }
            ConstructorInitializer::SuperCall(call) => call
                .args()
                .positional()
                .chain(call.args().named().map(|argument| argument.value()))
                .all(constant),
            ConstructorInitializer::ThisCall(call) => call
                .args()
                .positional()
                .chain(call.args().named().map(|argument| argument.value()))
                .all(constant),
        })
    }
}

fn class_lexical_value_names<'a>(
    type_params: AstChildren<'a, TypeParam<'a>>,
    members: EffectiveMembers<'a>,
) -> HashSet<&'a str> {
    let mut names = member_names(members.declared.clone());
    names.extend(type_params.map(|parameter| parameter.name().name()));
    names.extend(
        members
            .declaring_params()
            .map(|parameter| parameter.name().name()),
    );
    names
}

/// Initializers of the class's instance fields.
fn class_field_initializers<'a>(
    members: AstChildren<'a, ClassMember<'a>>,
) -> HashMap<&'a str, Expr<'a>> {
    members
        .filter_map(|member| match member {
            ClassMember::Field(field) if !field.is_static() => Some(field),
            _ => None,
        })
        .flat_map(|field| {
            field.declarators().filter_map(|declarator| {
                declarator
                    .initializer()
                    .map(|initializer| (declarator.name().name(), initializer))
            })
        })
        .collect()
}

/// The constructor a `super`/`this` call names; the unnamed one is `new`.
fn delegate(name: Option<Identifier<'_>>) -> &str {
    name.map_or("new", |name| name.name())
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

    #[test]
    fn reports_ordinary_constructors_before_the_primary_constructor() {
        let source = r"
// @dart = 3.13
import 'package:meta/meta.dart';
@immutable
class Frozen() {
  Frozen.named();
}
";
        let diagnostics = span_test_support::analyze(
            &PreferConstConstructorsInImmutables,
            Path::new("test.dart"),
            source,
            &FalconConfig::default(),
        );
        let spans: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let ordinary = source.find("Frozen.named").unwrap();
        let primary = source.find("Frozen()").unwrap();
        assert_eq!(
            spans,
            [
                (ordinary, ordinary + "Frozen".len()),
                (primary, primary + "Frozen".len()),
            ]
        );
    }
}
