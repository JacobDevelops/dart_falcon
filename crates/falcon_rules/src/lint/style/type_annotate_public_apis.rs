//! Requires type annotations on public declarations.
//!
//! A public API's types are part of its contract; leaving them to inference
//! (or `dynamic`) makes the contract implicit and easy to break. Public
//! top-level functions and methods need a return type (setters and
//! `operator []=` excepted), their parameters need types, and public fields
//! and top-level variables need a declared type unless they are `const`, or
//! `final` with an initializer whose inferred type is neither `dynamic` nor
//! `Null`. Parameters of public constructors of public, non-enum classes are
//! checked too.
//! Missing operator return types highlight the operator name alone.
//!
//! ```dart
//! var answer = 42;           // flagged
//! install(id, url) {}        // flagged
//! final pi = 3.14;           // ok: inferred `double`
//! void install(String id) {} // ok
//! ```
//!
//! The analyzer's visitor also descends into the initializers of untyped
//! fields and top-level variables, flagging closure parameters and locals
//! declared there; falcon checks only the declarations themselves, since those
//! nested names are not public API.
//!
//! Adopted from the official `type_annotate_public_apis` lint.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::declarations::documented_node_span;
use crate::lint::style::always_declare_return_types::operator_span;
use crate::lint::style::always_specify_types::setter_parameter_span;
use falcon_analyze::semantic_scope::SemanticState;

const RULE: &str = "type-annotate-public-apis";
const MESSAGE: &str = "Missing type annotation on a public API.";

#[derive(Default)]
pub struct TypeAnnotatePublicApis;

impl Rule for TypeAnnotatePublicApis {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let checker = Checker {
            ctx,
            state,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(checker))
    }
}

struct Checker<'a> {
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Checker<'_> {
    fn visit_top_level_decl(&mut self, declaration: &TopLevelDecl) -> StructuralTraversal {
        self.declaration(declaration);
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

impl Checker<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }

    fn declaration(&mut self, declaration: &TopLevelDecl) {
        match declaration {
            TopLevelDecl::Variable(variable) => {
                if variable.var_type.is_none() {
                    self.variables(variable.keyword, &variable.declarators);
                }
            }
            TopLevelDecl::Function(function) if !is_private(&function.name.name) => {
                let setter = matches!(function.kind, FunctionKind::Setter(_));
                if function.return_type.is_none() && !setter {
                    self.report(function.name.span);
                } else if let Some(params) = function.params() {
                    self.params(params);
                }
            }
            // Only old-style aliases have a formal parameter list as written.
            TopLevelDecl::TypeAlias(alias) if !is_private(&alias.name.name) => {
                if let DartType::Function(function) = &alias.aliased
                    && let Some(params) = &alias.legacy_params
                {
                    if function.return_type.is_none() {
                        self.report(alias.name.span);
                    } else {
                        self.params(params);
                    }
                }
            }
            TopLevelDecl::Class(class) => {
                if let Some(primary) = &class.primary_constructor {
                    self.primary_constructor(primary, &class.name);
                }
                self.members(&class.members, Some(&class.name));
            }
            TopLevelDecl::MixinClass(class) => {
                if let Some(primary) = &class.primary_constructor {
                    self.primary_constructor(primary, &class.name);
                }
                self.members(&class.members, Some(&class.name));
            }
            TopLevelDecl::Mixin(mixin) => self.members(&mixin.members, Some(&mixin.name)),
            TopLevelDecl::Enum(enumeration) => self.members(&enumeration.members, None),
            TopLevelDecl::Extension(extension) => self.members(&extension.members, None),
            TopLevelDecl::ExtensionType(extension) => {
                self.primary_constructor(&extension.primary_constructor, &extension.name);
                self.members(&extension.members, Some(&extension.name));
            }
            _ => {}
        }
    }

    fn primary_constructor(&mut self, primary: &PrimaryConstructor, owner: &Identifier) {
        if !is_private(&owner.name)
            && primary
                .constructor_name
                .as_ref()
                .is_none_or(|name| !is_private(&name.name))
        {
            self.params(&primary.params);
        }
    }

    /// `constructor_owner` is the enclosing type when its constructors are
    /// public API candidates (enums have none).
    fn members(&mut self, members: &[ClassMember], constructor_owner: Option<&Identifier>) {
        for member in members {
            match member {
                ClassMember::Field(field) => {
                    if field.field_type.is_none() {
                        self.variables(field.keyword, &field.declarators);
                    }
                }
                ClassMember::Constructor(constructor) => {
                    let public_owner =
                        constructor_owner.is_some_and(|owner| !is_private(&owner.name));
                    let public_name = constructor
                        .constructor_name
                        .as_ref()
                        .is_none_or(|name| !is_private(&name.name));
                    if public_owner && public_name {
                        self.params(&constructor.params);
                    }
                }
                ClassMember::Method(method) if !is_private(&method.name.name) => {
                    if method.return_type.is_none() {
                        self.report(method.name.span);
                    }
                    self.params(&method.params);
                }
                ClassMember::Getter(getter) => {
                    if !is_private(&getter.name.name) && getter.return_type.is_none() {
                        self.report(getter.name.span);
                    }
                }
                ClassMember::Setter(setter) => {
                    if !is_private(&setter.name.name)
                        && setter.param_type.is_none()
                        && !setter.param.name.chars().all(|c| c == '_')
                    {
                        self.report(setter_parameter_span(setter, self.ctx));
                    }
                }
                ClassMember::Operator(operator) => {
                    if operator.return_type.is_none() && operator.op != "[]=" {
                        self.report(operator_span(operator, self.ctx.syntax));
                    }
                    self.params(&operator.params);
                }
                ClassMember::Method(_)
                | ClassMember::PrimaryConstructorBody(_)
                | ClassMember::Error(_) => {}
            }
        }
    }

    fn variables(&mut self, keyword: VarKeyword, declarators: &[VarDeclarator]) {
        if keyword == VarKeyword::Const {
            return;
        }
        for declarator in declarators {
            if is_private(&declarator.name.name) {
                continue;
            }
            if keyword == VarKeyword::Final
                && declarator
                    .initializer
                    .as_ref()
                    .is_some_and(|initializer| !self.lacks_inferred_type(initializer))
            {
                continue;
            }
            self.report(declarator.name.span);
        }
    }

    /// Only a proven `dynamic` or `Null` initializer type leaves a `final`
    /// declaration without a useful inferred type; unknown types stay silent.
    fn lacks_inferred_type(&self, initializer: &Expr) -> bool {
        matches!(
            self.state.infer(initializer),
            ResolvedType::Dynamic | ResolvedType::Null
        )
    }

    fn params(&mut self, params: &FormalParamList) {
        // Regular function-typed formals are an annotation boundary. Initializing
        // formals still expose their nested parameters to the public API check.
        for param in &params.params {
            if param.initializing.is_some()
                && let Some(nested) = &param.function_params
            {
                self.params(nested);
            }
            if param.function_params.is_none()
                && param.param_type.is_none()
                && param.initializing.is_none()
                && !param.name.name.chars().all(|c| c == '_')
            {
                self.report(documented_node_span(
                    param.span,
                    &param.annotations,
                    self.ctx,
                ));
            }
        }
    }
}
