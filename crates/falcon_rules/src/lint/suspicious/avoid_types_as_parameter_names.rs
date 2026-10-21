//! Flags implicit formal parameters, catch variables, and type parameters that
//! shadow a type in their enclosing scope.

use falcon_analyze::{
    AnalyzeContext, IdentityIndex, LibraryId, NameIdentity, Rule, RulePreparation,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidTypesAsParameterNames;

impl Rule for AvoidTypesAsParameterNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some((identities, file)) = ctx
            .identities()
            .and_then(|identities| Some((identities, identities.file(ctx.file_path())?)))
        else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            identities,
            library: identities.library(file),
            type_names: Vec::new(),
            primary_parameter: false,
            representation_constructor: false,
            scopes: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    identities: &'a IdentityIndex,
    library: LibraryId,
    type_names: Vec<&'a str>,
    primary_parameter: bool,
    representation_constructor: bool,
    scopes: Vec<(usize, bool, bool)>,
}

impl<'a> Collector<'a> {
    fn save_scope(&mut self) {
        self.scopes.push((
            self.type_names.len(),
            self.primary_parameter,
            self.representation_constructor,
        ));
    }

    fn restore_scope(&mut self) {
        let (types, primary, representation) = self.scopes.pop().unwrap();
        self.type_names.truncate(types);
        self.primary_parameter = primary;
        self.representation_constructor = representation;
    }

    fn resolves_to_library_type(&self, name: &str) -> bool {
        self.identities.resolve(self.library, name.into()) == NameIdentity::Type
            && !self
                .identities
                .resolve_declaration(self.library, name.into())
                .and_then(|identity| {
                    self.ctx
                        .signatures()
                        .and_then(|signatures| signatures.type_kind(&identity))
                        .or_else(|| self.ctx.types()?.type_kind_identity(&identity))
                })
                .is_some_and(|kind| {
                    matches!(
                        kind,
                        falcon_analyze::resolve::TypeKind::Enum
                            | falcon_analyze::resolve::TypeKind::Mixin
                    )
                })
    }

    fn resolves_to_type(&self, name: &str) -> bool {
        (name != "_" && self.type_names.contains(&name)) || self.resolves_to_library_type(name)
    }

    fn enter_types(&mut self, params: AstChildren<'a, TypeParam<'a>>, method: bool) {
        for param in params.clone() {
            if self.resolves_to_library_type(param.name().name())
                || (!method && self.type_names.contains(&param.name().name()))
            {
                self.diags.push(self.ctx.diagnostic("Avoid using an existing type as a type parameter name.", param.name().span()));
            }
        }
        self.type_names.extend(
            params
                .map(|param| param.name().name())
                .filter(|name| *name != "_"),
        );
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.save_scope();
        let params = match node {
            TopLevelDecl::Class(node) => node.type_params(),
            TopLevelDecl::ClassTypeAlias(node) => node.type_params(),
            TopLevelDecl::MixinClass(node) => node.type_params(),
            TopLevelDecl::Mixin(node) => node.type_params(),
            TopLevelDecl::Enum(node) => node.type_params(),
            TopLevelDecl::Extension(node) => node.type_params(),
            TopLevelDecl::ExtensionType(node) => node.type_params(),
            TopLevelDecl::TypeAlias(node) => node.type_params(),
            _ => return StructuralTraversal::Continue,
        };
        self.enter_types(params, false);
        self.representation_constructor = matches!(node, TopLevelDecl::ExtensionType(_));
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        self.restore_scope();
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.save_scope();
        self.enter_types(node.type_params(), false);
        StructuralTraversal::Continue
    }

    fn exit_function_decl(&mut self, _node: FunctionDecl<'a>) {
        self.restore_scope();
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.save_scope();
        self.enter_types(node.type_params(), true);
        StructuralTraversal::Continue
    }

    fn exit_method_decl(&mut self, _node: MethodDecl<'a>) {
        self.restore_scope();
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.save_scope();
        if let Expr::FuncExpr(function) = node {
            self.enter_types(function.type_params(), false);
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.restore_scope();
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        self.save_scope();
        if let DartType::Function(node) = node {
            self.enter_types(node.type_params(), false);
        }
        StructuralTraversal::Continue
    }

    fn exit_dart_type(&mut self, _node: DartType<'a>) {
        self.restore_scope();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.save_scope();
        if let Stmt::TryCatch(statement) = node {
            for clause in statement.catches() {
                if let Some(name) = clause.exception_var()
                    && self.resolves_to_type(name.name())
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Avoid using an existing type as a parameter name.",
                        name.span(),
                    ));
                }
            }
        }
        if let Stmt::LocalFunc(function) = node {
            self.enter_types(function.type_params(), false);
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        self.restore_scope();
    }

    fn visit_primary_constructor(&mut self, _node: PrimaryConstructor<'a>) -> StructuralTraversal {
        self.save_scope();
        self.primary_parameter = true;
        StructuralTraversal::Continue
    }

    fn exit_primary_constructor(&mut self, _node: PrimaryConstructor<'a>) {
        self.restore_scope();
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        self.save_scope();
        if node.param_type().is_none()
            && node.function_params().is_none()
            && node.initializing() != Some(InitializingFormal::This)
            && !(self.primary_parameter
                && (self.representation_constructor || node.is_final() || node.is_var()))
            && self.resolves_to_type(node.name().name())
        {
            self.diags.push(self.ctx.diagnostic(
                "Avoid using an existing type as a parameter name.",
                node.name().span(),
            ));
        }
        // Nested function parameters and default expressions are not declaring
        // parameters of the enclosing primary constructor.
        self.primary_parameter = false;
        StructuralTraversal::Continue
    }

    fn exit_formal_param(&mut self, _node: FormalParam<'a>) {
        self.restore_scope();
    }
}
}
