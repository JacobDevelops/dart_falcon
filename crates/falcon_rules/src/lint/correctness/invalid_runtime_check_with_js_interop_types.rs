//! Reject platform-dependent runtime checks involving JavaScript interop types.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex,
    TypeEnvironment, TypeParameterScope, TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program, walk_stmt, walk_top_level_decl};

pub struct InvalidRuntimeCheckWithJsInteropTypes;

impl Rule for InvalidRuntimeCheckWithJsInteropTypes {
    fn name(&self) -> &'static str {
        "invalid-runtime-check-with-js-interop-types"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            signatures,
            environment: TypeEnvironment::new(),
            type_parameters: TypeParameterScope::default(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

#[derive(Clone)]
struct Normalized {
    ty: ResolvedType,
    interop: Interop,
}

#[derive(Clone, PartialEq, Eq)]
enum Interop {
    None,
    Sdk,
    User(DeclarationIdentity),
    Unknown,
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    file: String,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    environment: TypeEnvironment,
    type_parameters: TypeParameterScope,
}

impl Collector<'_> {
    fn normalize(&self, ty: &ResolvedType) -> Normalized {
        self.normalize_inner(ty, &mut HashSet::new())
    }

    fn normalize_inner(
        &self,
        ty: &ResolvedType,
        visiting: &mut HashSet<DeclarationIdentity>,
    ) -> Normalized {
        match ty {
            ResolvedType::Unknown => Normalized {
                ty: ResolvedType::Unknown,
                interop: Interop::Unknown,
            },
            ResolvedType::TypeParameter { bound, .. } => self.normalize_inner(bound, visiting),
            ResolvedType::Interface {
                identity,
                arguments,
                extension_type,
                ..
            } => {
                if is_js_sdk(identity) {
                    return Normalized {
                        ty: js_type(identity_name(identity)),
                        interop: Interop::Sdk,
                    };
                }
                if !visiting.insert(identity.clone()) {
                    return Normalized {
                        ty: ResolvedType::Unknown,
                        interop: Interop::Unknown,
                    };
                }
                let facts = self.signatures.declaration(identity);
                let static_interop = facts.is_some_and(|facts| {
                    has_annotation(&facts.annotations, "JS")
                        && has_annotation(&facts.annotations, "staticInterop")
                });
                let representation =
                    facts.and_then(|facts| facts.extension_representation.as_ref());
                let result = if static_interop {
                    Normalized {
                        ty: js_type("JSObject"),
                        interop: Interop::User(identity.clone()),
                    }
                } else if *extension_type || representation.is_some() {
                    representation.map_or(
                        Normalized {
                            ty: ResolvedType::Unknown,
                            interop: Interop::Unknown,
                        },
                        |representation| {
                            let normalized = self.normalize_inner(representation, visiting);
                            Normalized {
                                ty: normalized.ty,
                                interop: match normalized.interop {
                                    Interop::Sdk | Interop::User(_) => {
                                        Interop::User(identity.clone())
                                    }
                                    other => other,
                                },
                            }
                        },
                    )
                } else {
                    Normalized {
                        ty: ResolvedType::Interface {
                            identity: identity.clone(),
                            arguments: arguments
                                .iter()
                                .map(|argument| self.normalize_inner(argument, visiting).ty)
                                .collect(),
                            nullable: false,
                            extension_type: false,
                        },
                        interop: Interop::None,
                    }
                };
                visiting.remove(identity);
                result
            }
            ResolvedType::Function {
                return_type,
                positional,
                named,
                ..
            } => Normalized {
                ty: ResolvedType::Function {
                    return_type: Box::new(self.normalize_inner(return_type, visiting).ty),
                    positional: positional
                        .iter()
                        .map(|ty| self.normalize_inner(ty, visiting).ty)
                        .collect(),
                    named: named
                        .iter()
                        .map(|(name, ty)| (name.clone(), self.normalize_inner(ty, visiting).ty))
                        .collect(),
                    nullable: false,
                },
                interop: Interop::None,
            },
            ResolvedType::Record {
                positional, named, ..
            } => Normalized {
                ty: ResolvedType::Record {
                    positional: positional
                        .iter()
                        .map(|ty| self.normalize_inner(ty, visiting).ty)
                        .collect(),
                    named: named
                        .iter()
                        .map(|(name, ty)| (name.clone(), self.normalize_inner(ty, visiting).ty))
                        .collect(),
                    nullable: false,
                },
                interop: Interop::None,
            },
            other => Normalized {
                ty: other.with_nullable(false),
                interop: if matches!(other, ResolvedType::Dynamic) {
                    Interop::Unknown
                } else {
                    Interop::None
                },
            },
        }
    }

    fn invalid_test(&self, left: &ResolvedType, right: &ResolvedType, is_check: bool) -> bool {
        let left_normalized = self.normalize(left);
        let right_normalized = self.normalize(right);
        if matches!(left_normalized.interop, Interop::Unknown)
            || matches!(right_normalized.interop, Interop::Unknown)
        {
            return false;
        }
        let has_direct_interop = !matches!(left_normalized.interop, Interop::None)
            || !matches!(right_normalized.interop, Interop::None);
        if has_direct_interop {
            let left_subtype = self.is_subtype(&left_normalized.ty, &right_normalized.ty);
            let right_subtype = self.is_subtype(&right_normalized.ty, &left_normalized.ty);
            if is_check {
                if !left_subtype && !matches!(right_normalized.ty, ResolvedType::Dynamic) {
                    return true;
                }
                if left_subtype
                    && !matches!(left_normalized.interop, Interop::None)
                    && matches!(right_normalized.interop, Interop::User(_))
                    && !self.original_subtype(left, right)
                {
                    return true;
                }
            } else if !left_subtype
                && !right_subtype
                && !matches!(left_normalized.ty, ResolvedType::Dynamic)
                && !matches!(right_normalized.ty, ResolvedType::Dynamic)
            {
                return true;
            }
        }
        self.invalid_nested(left, right, is_check)
    }

    fn invalid_nested(&self, left: &ResolvedType, right: &ResolvedType, is_check: bool) -> bool {
        match (left, right) {
            (
                ResolvedType::Interface {
                    arguments: left, ..
                },
                ResolvedType::Interface {
                    arguments: right, ..
                },
            ) if left.len() == right.len() => left
                .iter()
                .zip(right)
                .any(|(left, right)| self.invalid_test(left, right, is_check)),
            (
                ResolvedType::Function {
                    return_type: left_return,
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Function {
                    return_type: right_return,
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
                self.invalid_test(left_return, right_return, is_check)
                    || paired_invalid(self, right_positional, left_positional, is_check)
                    || named_invalid(self, right_named, left_named, is_check)
            }
            (
                ResolvedType::Record {
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Record {
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
                paired_invalid(self, left_positional, right_positional, is_check)
                    || named_invalid(self, left_named, right_named, is_check)
            }
            (ResolvedType::TypeParameter { bound, .. }, right) => {
                self.invalid_test(bound, right, is_check)
            }
            (left, ResolvedType::TypeParameter { bound, .. }) => {
                self.invalid_test(left, bound, is_check)
            }
            _ => false,
        }
    }

    fn is_subtype(&self, left: &ResolvedType, right: &ResolvedType) -> bool {
        if matches!(left, ResolvedType::Never | ResolvedType::Dynamic)
            || matches!(right, ResolvedType::Dynamic)
        {
            return true;
        }
        if let ResolvedType::Interface { identity, .. } = right
            && matches!(identity, DeclarationIdentity::Sdk { library, name }
                if library == "dart:core" && name == "Object")
        {
            return true;
        }
        match (left, right) {
            (
                ResolvedType::Interface {
                    identity: left_identity,
                    arguments: left_arguments,
                    ..
                },
                ResolvedType::Interface {
                    identity: right_identity,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                if left_identity == right_identity {
                    return left_arguments.len() == right_arguments.len()
                        && left_arguments
                            .iter()
                            .zip(right_arguments)
                            .all(|(left, right)| self.is_subtype(left, right));
                }
                if js_subtype(left_identity, right_identity) {
                    return true;
                }
                self.signatures
                    .is_subtype_of(left, right_identity, &self.model)
                    == TypeTruth::Yes
            }
            (ResolvedType::Null, ResolvedType::Null) => true,
            (ResolvedType::Void, ResolvedType::Void) => true,
            (ResolvedType::Function { .. }, ResolvedType::Function { .. })
            | (ResolvedType::Record { .. }, ResolvedType::Record { .. }) => left == right,
            _ => false,
        }
    }

    fn original_subtype(&self, left: &ResolvedType, right: &ResolvedType) -> bool {
        match (left, right) {
            (
                ResolvedType::Interface {
                    identity: left_identity,
                    ..
                },
                ResolvedType::Interface {
                    identity: right_identity,
                    ..
                },
            ) => {
                left_identity == right_identity
                    || self
                        .signatures
                        .is_subtype_of(left, right_identity, &self.model)
                        == TypeTruth::Yes
            }
            (ResolvedType::TypeParameter { bound, .. }, right) => {
                self.original_subtype(bound, right)
            }
            _ => false,
        }
    }

    fn check(&mut self, expression: &Expr, target: &DartType, span: &Span, is_check: bool) {
        let source = self.environment.infer_with_signatures(
            expression,
            &self.model,
            self.signatures,
            &self.type_parameters,
        );
        let target = self.model.resolve_type_in(target, &self.type_parameters);
        if self.invalid_test(&source, &target, is_check) {
            self.report(
                span,
                if is_check {
                    "Runtime type checks involving JS interop types are platform-dependent."
                } else {
                    "This cast between JS interop and Dart types is not portable."
                },
            );
        }
    }

    fn report(&mut self, span: &Span, message: &str) {
        self.diagnostics.push(Diagnostic::new(
            "invalid-runtime-check-with-js-interop-types",
            Severity::Warning,
            message,
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn function(&mut self, params: &FormalParamList, body: Option<&FunctionBody>) {
        let saved = std::mem::replace(&mut self.environment, TypeEnvironment::new());
        self.environment
            .bind_params(params, &self.model, &self.type_parameters);
        if let Some(body) = body {
            match body {
                FunctionBody::Block(block) => block
                    .stmts
                    .iter()
                    .for_each(|stmt| visit_statement(self, stmt)),
                FunctionBody::Arrow(expression, _) => self.visit_expr(expression),
                FunctionBody::Native(_, _) => {}
            }
        }
        self.environment = saved;
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let Some(type_params) = declaration_type_params(node) else {
            walk_top_level_decl(self, node);
            return;
        };
        self.type_parameters.push(type_params, &self.model);
        walk_top_level_decl(self, node);
        self.type_parameters.pop();
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.type_parameters.push(&node.type_params, &self.model);
        self.function(&node.params, node.body.as_ref());
        self.type_parameters.pop();
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.type_parameters.push(&node.type_params, &self.model);
        self.function(&node.params, node.body.as_ref());
        self.type_parameters.pop();
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(&node.params, node.body.as_ref());
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Is {
                expr,
                dart_type,
                span,
                ..
            } => self.check(expr, dart_type, span, true),
            Expr::As {
                expr,
                dart_type,
                span,
            } => self.check(expr, dart_type, span, false),
            _ => {}
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(declaration) => {
                walk_stmt(self, node);
                let ty = declaration
                    .var_type
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or(ResolvedType::Unknown);
                for declarator in &declaration.declarators {
                    self.environment
                        .declare(declarator.name.name.clone(), ty.clone());
                }
            }
            Stmt::Block(block) => {
                self.environment.push_scope();
                for statement in &block.stmts {
                    visit_statement(self, statement);
                }
                self.environment.pop_scope();
            }
            Stmt::TryCatch(statement) => {
                for body_statement in &statement.body.stmts {
                    visit_statement(self, body_statement);
                }
                for catch in &statement.catches {
                    if let Some(exception_type) = &catch.exception_type {
                        let ty = self
                            .model
                            .resolve_type_in(exception_type, &self.type_parameters);
                        if !matches!(
                            self.normalize(&ty).interop,
                            Interop::None | Interop::Unknown
                        ) {
                            self.report(
                                &catch.span,
                                "JS interop types cannot be used as catch types.",
                            );
                        }
                    }
                    self.environment.push_scope();
                    if let Some(exception) = &catch.exception_var {
                        let ty = catch
                            .exception_type
                            .as_ref()
                            .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                            .unwrap_or(ResolvedType::Dynamic);
                        self.environment.declare(exception.name.clone(), ty);
                    }
                    for catch_statement in &catch.body.stmts {
                        visit_statement(self, catch_statement);
                    }
                    self.environment.pop_scope();
                }
                if let Some(finally) = &statement.finally {
                    for finally_statement in &finally.stmts {
                        visit_statement(self, finally_statement);
                    }
                }
            }
            Stmt::LocalFunc(function) => {
                self.type_parameters
                    .push(&function.type_params, &self.model);
                self.function(&function.params, Some(&function.body));
                self.type_parameters.pop();
            }
            _ => walk_stmt(self, node),
        }
    }
}

fn declaration_type_params(node: &TopLevelDecl) -> Option<&[TypeParam]> {
    match node {
        TopLevelDecl::Class(declaration) => Some(&declaration.type_params),
        TopLevelDecl::Mixin(declaration) => Some(&declaration.type_params),
        TopLevelDecl::MixinClass(declaration) => Some(&declaration.type_params),
        TopLevelDecl::Enum(declaration) => Some(&declaration.type_params),
        TopLevelDecl::Extension(declaration) => Some(&declaration.type_params),
        TopLevelDecl::ExtensionType(declaration) => Some(&declaration.type_params),
        _ => None,
    }
}

fn paired_invalid(
    collector: &Collector<'_>,
    left: &[ResolvedType],
    right: &[ResolvedType],
    is_check: bool,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .any(|(left, right)| collector.invalid_test(left, right, is_check))
}

fn named_invalid(
    collector: &Collector<'_>,
    left: &[(String, ResolvedType)],
    right: &[(String, ResolvedType)],
    is_check: bool,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .any(|((left_name, left), (right_name, right))| {
                left_name == right_name && collector.invalid_test(left, right, is_check)
            })
}

fn has_annotation(annotations: &[DeclarationIdentity], name: &str) -> bool {
    annotations.iter().any(|annotation| {
        matches!(annotation, DeclarationIdentity::Sdk { library, name: found }
            if library == "dart:js_interop" && found == name)
    })
}

fn js_type(name: &str) -> ResolvedType {
    ResolvedType::Interface {
        identity: DeclarationIdentity::Sdk {
            library: "dart:js_interop".to_string(),
            name: name.to_string(),
        },
        arguments: Vec::new(),
        nullable: false,
        extension_type: true,
    }
}

fn is_js_sdk(identity: &DeclarationIdentity) -> bool {
    matches!(identity, DeclarationIdentity::Sdk { library, .. } if library == "dart:js_interop")
}

fn identity_name(identity: &DeclarationIdentity) -> &str {
    match identity {
        DeclarationIdentity::Project { name, .. }
        | DeclarationIdentity::Sdk { name, .. }
        | DeclarationIdentity::Package { name, .. } => name,
    }
}

fn js_subtype(left: &DeclarationIdentity, right: &DeclarationIdentity) -> bool {
    if !is_js_sdk(left) || !is_js_sdk(right) {
        return false;
    }
    let left = identity_name(left);
    let right = identity_name(right);
    if right == "JSAny" {
        return true;
    }
    if left == right {
        return true;
    }
    if right == "JSObject" {
        return matches!(
            left,
            "JSObject"
                | "JSFunction"
                | "JSExportedDartFunction"
                | "JSBoxedDartObject"
                | "JSArray"
                | "JSPromise"
                | "JSArrayBuffer"
                | "JSDataView"
                | "JSTypedArray"
                | "JSInt8Array"
                | "JSUint8Array"
                | "JSUint8ClampedArray"
                | "JSInt16Array"
                | "JSUint16Array"
                | "JSInt32Array"
                | "JSUint32Array"
                | "JSFloat32Array"
                | "JSFloat64Array"
                | "JSBigInt64Array"
                | "JSBigUint64Array"
        );
    }
    right == "JSTypedArray"
        && matches!(
            left,
            "JSInt8Array"
                | "JSUint8Array"
                | "JSUint8ClampedArray"
                | "JSInt16Array"
                | "JSUint16Array"
                | "JSInt32Array"
                | "JSUint32Array"
                | "JSFloat32Array"
                | "JSFloat64Array"
                | "JSBigInt64Array"
                | "JSBigUint64Array"
        )
}

fn visit_statement(visitor: &mut impl Visitor, statement: &Stmt) {
    visitor.visit_stmt(statement);
}
