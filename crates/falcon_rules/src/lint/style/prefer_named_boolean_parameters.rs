//! Flags explicit positional `bool` parameters in private declarations and unconstrained closures.
//!
//! This complements `avoid-positional-boolean-parameters`, covering declarations
//! that shipped rule skips. Enabling both gives full coverage of the explicit
//! `bool`/`bool?` forms described here without duplicate diagnostics.
//! Named arguments make boolean values understandable at call sites.
//!
//! Private named functions (including local functions), constructors whose own
//! name starts with `_`, and private methods without inherited contracts are
//! checked. Closures are checked only as direct initializers of untyped variables
//! or fields, not in arguments, returns, assignments, or typed declarations.
//! The first required or optional positional `bool`/`bool?` is reported.
//! Named parameters, public signatures, setters, operators, extension-type
//! representation parameters, and `Function(...)` types are excluded, as are
//! aliases and inferred field/super parameters. Explicit types must resolve to
//! `dart:core`'s `bool`, not a shadowing declaration or type parameter. Methods
//! with override annotations or unresolved inheritance are left alone.
//!
//! ```dart
//! void _configure(bool verbose) {} // Prefer `{required bool verbose}`.
//! final configure = (bool verbose) {}; // Prefer `({required bool verbose})`.
//! ```

use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation,
    StructuralRuleVisitor, StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

const RULE: &str = "prefer-named-boolean-parameters";

#[derive(Default)]
pub struct PreferNamedBooleanParameters;

impl Rule for PreferNamedBooleanParameters {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            state,
            owner: None,
            is_interface: false,
            is_extension_type: false,
            type_parameters: TypeParameterScope::default(),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    owner: Option<DeclarationIdentity>,
    is_interface: bool,
    is_extension_type: bool,
    type_parameters: TypeParameterScope,
    diags: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn is_fixed(&self, name: &str, mut annotations: AstChildren<'a, Annotation<'a>>) -> bool {
        if annotations.any(|annotation| {
            annotation
                .name()
                .next_back()
                .is_some_and(|name| name.name() == "override")
        }) {
            return true;
        }
        if !self.is_interface {
            return false;
        }
        self.owner.as_ref().is_none_or(|owner| {
            self.state
                .signatures
                .inherited_member_facts(owner, name)
                .is_none_or(|facts| !facts.is_empty())
        })
    }

    fn check_initializers(&mut self, declarations: AstChildren<'a, VarDeclarator<'a>>) {
        for declaration in declarations {
            if let Some(initializer) = declaration.initializer()
                && let Expr::FuncExpr(function) = initializer.unparenthesized()
            {
                self.type_parameters
                    .push(function.type_params(), &self.state.model);
                self.check(function.params());
                self.type_parameters.pop();
            }
        }
    }

    fn check(&mut self, params: FormalParamList<'a>) {
        let first = params.positional().find(|param| {
            if param.function_params().is_some() {
                return false;
            }
            let Some(DartType::Named(ty)) = param.param_type() else {
                return false;
            };
            let mut segments = ty.segments();
            if !segments.next().is_some_and(|name| name.name() == "bool")
                || segments.next().is_some()
                || ty.type_args().next().is_some()
            {
                return false;
            }
            matches!(
                self.state.model.resolve_type_in(DartType::Named(ty), &self.type_parameters),
                ResolvedType::Interface { identity, .. }
                    if identity.is_sdk("dart:core", "bool")
            )
        });
        if let Some(param) = first {
            self.diags.push(self.ctx.diagnostic(
                RULE,
                "Prefer a named boolean parameter instead of a positional parameter.",
                param.span(),
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.is_extension_type = matches!(node, TopLevelDecl::ExtensionType(_));
        let name = match node {
            TopLevelDecl::Class(node) => Some(node.name()),
            TopLevelDecl::MixinClass(node) => Some(node.name()),
            TopLevelDecl::Mixin(node) => Some(node.name()),
            TopLevelDecl::Enum(node) => Some(node.name()),
            TopLevelDecl::ExtensionType(node) => Some(node.name()),
            _ => None,
        };
        self.is_interface = name.is_some();
        self.owner = name.and_then(|name| self.state.model.resolve_name(name.name()));
        let params = match node {
            TopLevelDecl::Class(node) => node.type_params(),
            TopLevelDecl::ClassTypeAlias(node) => node.type_params(),
            TopLevelDecl::MixinClass(node) => node.type_params(),
            TopLevelDecl::Mixin(node) => node.type_params(),
            TopLevelDecl::Enum(node) => node.type_params(),
            TopLevelDecl::Extension(node) => node.type_params(),
            TopLevelDecl::ExtensionType(node) => node.type_params(),
            TopLevelDecl::Function(node) => node.type_params(),
            TopLevelDecl::TypeAlias(node) => node.type_params(),
            TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => AstChildren::empty(),
        };
        self.type_parameters.push(params, &self.state.model);
        if let TopLevelDecl::Function(function) = node
            && function.name().name().starts_with('_')
            && let FunctionKind::Function(params) = function.kind()
        {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _: TopLevelDecl<'a>) {
        self.type_parameters.pop();
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        match node {
            ClassMember::Method(method) => {
                self.type_parameters.push(method.type_params(), &self.state.model);
                if method.name().name().starts_with('_')
                    && !self.is_fixed(method.name().name(), method.annotations())
                {
                    self.check(method.params());
                }
            }
            ClassMember::Constructor(constructor)
                if constructor.constructor_name().is_some_and(|name| name.name().starts_with('_')) =>
            {
                self.check(constructor.params());
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, node: ClassMember<'a>) {
        if matches!(node, ClassMember::Method(_)) {
            self.type_parameters.pop();
        }
    }

    fn visit_primary_constructor(&mut self, node: PrimaryConstructor<'a>) -> StructuralTraversal {
        if !self.is_extension_type
            && node.constructor_name().is_some_and(|name| name.name().starts_with('_'))
        {
            self.check(node.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.type_parameters.push(function.type_params(), &self.state.model);
            if function.name().name().starts_with('_') {
                self.check(function.params());
            }
        }
        if let Stmt::LocalVar(declaration) = node
            && declaration.var_type().is_none()
        {
            self.check_initializers(declaration.declarators());
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if matches!(node, Stmt::LocalFunc(_)) {
            self.type_parameters.pop();
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            self.type_parameters.push(function.type_params(), &self.state.model);
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.type_parameters.pop();
        }
    }

    fn visit_top_level_var(&mut self, node: TopLevelVarDecl<'a>) -> StructuralTraversal {
        if node.var_type().is_none() {
            self.check_initializers(node.declarators());
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.field_type().is_none() {
            for declaration in node.declarators() {
                if !self.is_fixed(declaration.name().name(), node.annotations())
                    && let Some(initializer) = declaration.initializer()
                    && let Expr::FuncExpr(function) = initializer.unparenthesized()
                {
                    self.type_parameters.push(function.type_params(), &self.state.model);
                    self.check(function.params());
                    self.type_parameters.pop();
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
