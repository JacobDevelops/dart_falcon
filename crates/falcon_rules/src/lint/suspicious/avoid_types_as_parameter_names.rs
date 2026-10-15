//! Flags implicit formal parameters, catch variables, and type parameters that
//! shadow a type in their enclosing scope.

use falcon_analyze::{AnalyzeContext, IdentityIndex, LibraryId, NameIdentity, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::*;

#[derive(Default)]
pub struct AvoidTypesAsParameterNames;

impl Rule for AvoidTypesAsParameterNames {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some((identities, file)) = ctx
            .identities
            .and_then(|identities| Some((identities, identities.file(ctx.file_path)?)))
        else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            identities,
            library: identities.library(file),
            type_names: Vec::new(),
            primary_parameter: false,
            representation_constructor: false,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    identities: &'a IdentityIndex,
    library: LibraryId,
    type_names: Vec<Span>,
    primary_parameter: bool,
    representation_constructor: bool,
}

impl Collector<'_> {
    fn resolves_to_library_type(&self, name: &str) -> bool {
        self.identities.resolve(self.library, name.into()) == NameIdentity::Type
            && !self
                .identities
                .resolve_declaration(self.library, name.into())
                .and_then(|identity| {
                    self.ctx
                        .signatures
                        .and_then(|signatures| signatures.type_kind(&identity))
                        .or_else(|| self.ctx.types?.type_kind_identity(&identity))
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
        (name != "_"
            && self
                .type_names
                .iter()
                .any(|span| self.ctx.source.get(span.start..span.end) == Some(name)))
            || self.resolves_to_library_type(name)
    }

    fn enter_types(&mut self, params: &[TypeParam], method: bool) -> usize {
        let previous = self.type_names.len();
        for param in params {
            if self.resolves_to_library_type(&param.name.name)
                || (!method
                    && self.type_names.iter().any(|span| {
                        self.ctx.source.get(span.start..span.end) == Some(param.name.name.as_str())
                    }))
            {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-types-as-parameter-names",
                    "Avoid using an existing type as a type parameter name.",
                    param.name.span,
                ));
            }
        }
        self.type_names.extend(
            params
                .iter()
                .filter(|param| param.name.name != "_")
                .map(|param| param.name.span),
        );
        previous
    }
}

impl Visitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let params = match node {
            TopLevelDecl::Class(node) => &node.type_params,
            TopLevelDecl::ClassTypeAlias(node) => &node.type_params,
            TopLevelDecl::MixinClass(node) => &node.type_params,
            TopLevelDecl::Mixin(node) => &node.type_params,
            TopLevelDecl::Enum(node) => &node.type_params,
            TopLevelDecl::Extension(node) => &node.type_params,
            TopLevelDecl::ExtensionType(node) => &node.type_params,
            TopLevelDecl::TypeAlias(node) => &node.type_params,
            _ => {
                walk_top_level_decl(self, node);
                return;
            }
        };
        let previous = self.enter_types(params, false);
        let representation = self.representation_constructor;
        self.representation_constructor = matches!(node, TopLevelDecl::ExtensionType(_));
        walk_top_level_decl(self, node);
        self.type_names.truncate(previous);
        self.representation_constructor = representation;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let previous = self.enter_types(&node.type_params, false);
        walk_function_decl(self, node);
        self.type_names.truncate(previous);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        let previous = self.enter_types(&node.type_params, true);
        walk_method_decl(self, node);
        self.type_names.truncate(previous);
    }

    fn visit_expr(&mut self, node: &Expr) {
        let previous = if let Expr::FuncExpr { type_params, .. } = node {
            self.enter_types(type_params, false)
        } else {
            self.type_names.len()
        };
        walk_expr(self, node);
        self.type_names.truncate(previous);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        let previous = if let DartType::Function(node) = node {
            self.enter_types(&node.type_params, false)
        } else {
            self.type_names.len()
        };
        walk_dart_type(self, node);
        self.type_names.truncate(previous);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(statement) = node {
            for clause in &statement.catches {
                if let Some(name) = &clause.exception_var
                    && self.resolves_to_type(&name.name)
                {
                    self.diags.push(self.ctx.diagnostic(
                        "avoid-types-as-parameter-names",
                        "Avoid using an existing type as a parameter name.",
                        name.span,
                    ));
                }
            }
        }
        if let Stmt::LocalFunc(function) = node {
            let previous = self.enter_types(&function.type_params, false);
            walk_stmt(self, node);
            self.type_names.truncate(previous);
        } else {
            walk_stmt(self, node);
        }
    }

    fn visit_primary_constructor(&mut self, node: &PrimaryConstructor) {
        let previous = self.primary_parameter;
        self.primary_parameter = true;
        walk_primary_constructor(self, node);
        self.primary_parameter = previous;
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if node.param_type.is_none()
            && node.function_params.is_none()
            && node.initializing != Some(InitializingFormal::This)
            && !(self.primary_parameter
                && (self.representation_constructor || node.is_final || node.is_var))
            && self.resolves_to_type(&node.name.name)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-types-as-parameter-names",
                "Avoid using an existing type as a parameter name.",
                node.name.span,
            ));
        }
        // Nested function parameters and default expressions are not declaring
        // parameters of the enclosing primary constructor.
        let previous = self.primary_parameter;
        self.primary_parameter = false;
        walk_formal_param(self, node);
        self.primary_parameter = previous;
    }
}
