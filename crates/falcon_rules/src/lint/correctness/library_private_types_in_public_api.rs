//! Avoid using library-private types in public APIs.
//!
//! Adopted from the official `library_private_types_in_public_api` lint.
//!
//! Checks the types a public declaration exposes: type-parameter bounds,
//! signatures, field and top-level variable types, extension `on` types, mixin
//! `on` constraints, extension-type representations and function-type
//! aliases. Like the analyzer, supertypes (`extends`, `with`, `implements`),
//! non-function type aliases and record type annotations are not checked, and
//! constructors of enums and of sealed, `abstract final`, `abstract interface`
//! or `@internal` classes are exempt. Untyped initializing and super formals
//! use their resolved semantic types, including declarations in part files.

use falcon_analyze::{AnalyzeContext, ResolvedTypeView, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::declarations::has_meta_annotation;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct LibraryPrivateTypesInPublicApi;

impl Rule for LibraryPrivateTypesInPublicApi {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if std::iter::once(program)
            .chain(
                ctx.library()
                    .into_iter()
                    .flat_map(|library| library.siblings().iter().copied()),
            )
            .any(|unit| {
                unit.library_directive().is_some_and(|library| {
                    library.name().next().is_some_and(|name| !is_public(name))
                })
            })
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = falcon_analyze::SemanticModel::new(
            ctx.file_path(),
            ctx.parsed(),
            identities,
            ctx.types(),
        );
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            public_constructors: Vec::new(),
        };
        for declaration in program.declarations() {
            collector.top_level(declaration, &model);
        }
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    public_constructors: Vec<Span>,
}

impl<'a> Collector<'a> {
    fn top_level(
        &mut self,
        declaration: TopLevelDecl<'a>,
        model: &falcon_analyze::SemanticModel<'_>,
    ) {
        match declaration {
            TopLevelDecl::Class(node) if is_public(node.name()) => {
                self.type_params(node.type_params());
                let closed = has_meta_annotation(node.annotations(), "internal", model)
                    || match node.modifiers().restriction {
                        Some(ClassRestriction::Sealed) => true,
                        Some(ClassRestriction::Final | ClassRestriction::Interface) => {
                            node.modifiers().is_abstract
                        }
                        _ => false,
                    };
                self.members(node.members(), closed, false);
            }
            TopLevelDecl::ClassTypeAlias(node) if is_public(node.name()) => {
                self.type_params(node.type_params());
            }
            TopLevelDecl::Mixin(node) if is_public(node.name()) => {
                self.types(node.on_clause());
                self.type_params(node.type_params());
                self.members(node.members(), false, false);
            }
            TopLevelDecl::MixinClass(node) if is_public(node.name()) => {
                self.type_params(node.type_params());
                self.members(
                    node.members(),
                    has_meta_annotation(node.annotations(), "internal", model),
                    false,
                );
            }
            TopLevelDecl::Enum(node) if is_public(node.name()) => {
                self.type_params(node.type_params());
                self.members(node.members(), true, false);
            }
            TopLevelDecl::Extension(node) if node.name().is_some_and(is_public) => {
                self.type_params(node.type_params());
                self.type_(node.on_type());
                self.members(node.members(), false, false);
            }
            TopLevelDecl::ExtensionType(node) if is_public(node.name()) => {
                self.type_params(node.type_params());
                if let Some(representation) = node.representation()
                    && is_public(representation.name())
                {
                    self.optional_type(representation.param_type());
                }
                self.members(node.members(), false, true);
            }
            TopLevelDecl::Function(node) if is_public(node.name()) => {
                self.optional_type(node.return_type());
                self.type_params(node.type_params());
                if let Some(params) = node.params() {
                    self.params(params);
                }
            }
            TopLevelDecl::Variable(node)
                if node
                    .declarators()
                    .any(|declarator| is_public(declarator.name())) =>
            {
                self.optional_type(node.var_type());
            }
            TopLevelDecl::TypeAlias(node) if is_public(node.name()) => {
                self.type_params(node.type_params());
                // Only function-type aliases are checked; the analyzer visits
                // a non-function aliased type nowhere.
                if matches!(node.aliased(), DartType::Function(_)) {
                    self.type_(node.aliased());
                }
            }
            _ => {}
        }
    }

    /// `closed` exempts constructors (enums and effectively private
    /// classes); `extension_type` skips the invalid instance fields of
    /// extension types.
    fn members(
        &mut self,
        members: AstChildren<'a, ClassMember<'a>>,
        closed: bool,
        extension_type: bool,
    ) {
        for member in members {
            match member {
                ClassMember::Field(node)
                    if !(extension_type && !node.is_static())
                        && node
                            .declarators()
                            .any(|declarator| is_public(declarator.name())) =>
                {
                    self.optional_type(node.field_type());
                }
                ClassMember::Constructor(node)
                    if !closed && node.constructor_name().is_none_or(is_public) =>
                {
                    self.public_constructors.push(node.span());
                    self.params(node.params());
                }
                ClassMember::Method(node) if is_public(node.name()) => {
                    self.optional_type(node.return_type());
                    self.type_params(node.type_params());
                    self.params(node.params());
                }
                ClassMember::Getter(node) if is_public(node.name()) => {
                    self.optional_type(node.return_type());
                }
                ClassMember::Setter(node) if is_public(node.name()) => {
                    self.optional_type(node.param_type());
                }
                ClassMember::Operator(node) => {
                    self.optional_type(node.return_type());
                    self.params(node.params());
                }
                _ => {}
            }
        }
    }

    fn params(&mut self, params: FormalParamList<'a>) {
        for param in params.positional() {
            self.param(param);
        }
        for param in params.named() {
            if is_public(param.name()) {
                self.param(param);
            }
        }
    }

    fn param(&mut self, param: FormalParam<'a>) {
        // Implicit initializing-formal types are checked by the shared
        // semantic constructor visitor after its parameters are bound.
        self.optional_type(param.param_type());
        self.type_params(param.type_params());
        if let Some(function_params) = param.function_params() {
            self.params(function_params);
        }
    }

    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic("Avoid using a private type in a public API.", span));
    }

    fn type_params(&mut self, params: AstChildren<'a, TypeParam<'a>>) {
        for param in params {
            self.optional_type(param.bound());
        }
    }

    fn types(&mut self, types: AstChildren<'a, DartType<'a>>) {
        for type_ in types {
            self.type_(type_);
        }
    }

    fn optional_type(&mut self, type_: Option<DartType<'a>>) {
        if let Some(type_) = type_ {
            self.type_(type_);
        }
    }

    fn type_(&mut self, type_: DartType<'a>) {
        match type_ {
            DartType::Named(node) => {
                if let Some(name) = node.segments().next_back().filter(|name| !is_public(*name)) {
                    self.report(name.span());
                }
                self.types(node.type_args());
            }
            DartType::Function(node) => {
                self.optional_type(node.return_type());
                self.type_params(node.type_params());
                for param in node.params() {
                    self.type_(param.param_type());
                }
            }
            // The analyzer does not visit record type annotations.
            DartType::Record(_) | DartType::Void(_) | DartType::Dynamic(_) => {}
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_constructor(&mut self, node: ConstructorDecl<'a>, state: &SemanticState<'a>) {
        if !self.public_constructors.contains(&node.span()) {
            return;
        }
        for param in node.params().params() {
            if param.param_type().is_some()
                || param.function_params().is_some()
                || param.initializing().is_none()
                || (matches!(param.kind(), ParamKind::Named { .. }) && !is_public(param.name()))
            {
                continue;
            }
            if let ResolvedTypeView::Interface { identity, .. } =
                state.environment.lookup(param.name().name()).view()
                && identity.name().starts_with('_')
            {
                self.report(param.name().span());
            }
        }
    }
}

fn is_public(identifier: Identifier<'_>) -> bool {
    !identifier.name().starts_with('_')
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let diagnostics = crate::lint::test_support::analyze_flutter_rule(
            "library-private-types-in-public-api",
            source,
            &LibraryPrivateTypesInPublicApi,
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }
}
