//! Avoid using library-private types in public APIs.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

pub struct LibraryPrivateTypesInPublicApi;

impl Rule for LibraryPrivateTypesInPublicApi {
    fn name(&self) -> &'static str {
        "library-private-types-in-public-api"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        for declaration in &program.declarations {
            collector.top_level(declaration);
        }
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn top_level(&mut self, declaration: &TopLevelDecl) {
        match declaration {
            TopLevelDecl::Class(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                self.optional_type(node.extends.as_ref());
                self.types(&node.with_clause);
                self.types(&node.implements);
                self.members(&node.members, false);
            }
            TopLevelDecl::ClassTypeAlias(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                self.type_(&node.superclass);
                self.types(&node.with_clause);
                self.types(&node.implements);
            }
            TopLevelDecl::Mixin(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                self.types(&node.on_clause);
                self.types(&node.implements);
                self.members(&node.members, false);
            }
            TopLevelDecl::MixinClass(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                self.optional_type(node.extends.as_ref());
                self.types(&node.with_clause);
                self.types(&node.implements);
                self.members(&node.members, false);
            }
            TopLevelDecl::Enum(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                self.types(&node.with_clause);
                self.types(&node.implements);
                self.members(&node.members, true);
            }
            TopLevelDecl::Extension(node) if node.name.as_ref().is_some_and(is_public) => {
                self.type_params(&node.type_params);
                self.type_(&node.on_type);
                self.members(&node.members, false);
            }
            TopLevelDecl::ExtensionType(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                if is_public(&node.representation.field_name) {
                    self.type_(&node.representation.field_type);
                }
                self.types(&node.implements);
                self.members(&node.members, false);
            }
            TopLevelDecl::Function(node) if is_public(&node.name) => {
                self.optional_type(node.return_type.as_ref());
                self.type_params(&node.type_params);
                self.params(&node.params);
            }
            TopLevelDecl::Variable(node)
                if node
                    .declarators
                    .iter()
                    .any(|declarator| is_public(&declarator.name)) =>
            {
                self.optional_type(node.var_type.as_ref());
            }
            TopLevelDecl::TypeAlias(node) if is_public(&node.name) => {
                self.type_params(&node.type_params);
                self.type_(&node.aliased);
            }
            _ => {}
        }
    }

    fn members(&mut self, members: &[ClassMember], enclosing_enum: bool) {
        for member in members {
            match member {
                ClassMember::Field(node)
                    if node
                        .declarators
                        .iter()
                        .any(|declarator| is_public(&declarator.name)) =>
                {
                    self.optional_type(node.field_type.as_ref());
                }
                ClassMember::Constructor(node)
                    if !enclosing_enum && node.constructor_name.as_ref().is_none_or(is_public) =>
                {
                    self.params(&node.params);
                }
                ClassMember::Method(node) if is_public(&node.name) => {
                    self.optional_type(node.return_type.as_ref());
                    self.type_params(&node.type_params);
                    self.params(&node.params);
                }
                ClassMember::Getter(node) if is_public(&node.name) => {
                    self.optional_type(node.return_type.as_ref());
                }
                ClassMember::Setter(node) if is_public(&node.name) => {
                    self.optional_type(node.param_type.as_ref());
                }
                ClassMember::Operator(node) => {
                    self.optional_type(node.return_type.as_ref());
                    self.params(&node.params);
                }
                _ => {}
            }
        }
    }

    fn params(&mut self, params: &FormalParamList) {
        for param in params.positional.iter().chain(&params.optional_positional) {
            self.param(param);
        }
        for param in &params.named {
            if is_public(&param.name) {
                self.param(param);
            }
        }
    }

    fn param(&mut self, param: &FormalParam) {
        self.optional_type(param.param_type.as_ref());
        self.type_params(&param.type_params);
        if let Some(function_params) = &param.function_params {
            self.params(function_params);
        }
    }

    fn type_params(&mut self, params: &[TypeParam]) {
        for param in params {
            self.optional_type(param.bound.as_ref());
        }
    }

    fn types(&mut self, types: &[DartType]) {
        for type_ in types {
            self.type_(type_);
        }
    }

    fn optional_type(&mut self, type_: Option<&DartType>) {
        if let Some(type_) = type_ {
            self.type_(type_);
        }
    }

    fn type_(&mut self, type_: &DartType) {
        match type_ {
            DartType::Named(node) => {
                if let Some(name) = node.segments.last().filter(|name| !is_public(name)) {
                    self.flag(&name.span);
                }
                self.types(&node.type_args);
            }
            DartType::Function(node) => {
                self.optional_type(node.return_type.as_deref());
                self.type_params(&node.type_params);
                for param in &node.params {
                    self.type_(&param.param_type);
                }
            }
            DartType::Record(node) => {
                self.types(&node.positional);
                for field in &node.named {
                    self.type_(&field.field_type);
                }
            }
            DartType::Void { .. } | DartType::Dynamic { .. } | DartType::Never { .. } => {}
        }
    }

    fn flag(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "library-private-types-in-public-api",
            Severity::Warning,
            "Avoid using a private type in a public API.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn is_public(identifier: &Identifier) -> bool {
    !identifier.name.starts_with('_')
}
