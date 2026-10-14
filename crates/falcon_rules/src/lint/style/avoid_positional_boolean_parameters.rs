//! Flags signatures that take more than one positional `bool` parameter.
//!
//! At a call site like `setFlags(true, false)` the literals say nothing about
//! what they mean, and swapping them goes unnoticed. Once a public function,
//! method, constructor, or `Function(...)` type takes two or more positional
//! `bool` parameters, the second one is reported; make them named instead:
//!
//! ```dart
//! void configure(bool verbose, bool dryRun) {}     // `bool dryRun` flagged
//! void configure({bool verbose = false, bool dryRun = false}) {}
//! void toggle(bool on) {}                          // fine: only one
//! ```
//!
//! Private declarations, getters, setters, and operators are skipped, as are
//! members whose signature is fixed by an inherited member they override (and
//! function types written in such a signature). Field and super parameters
//! (`this.flag`, `super.flag`) take their declared type. When falcon cannot
//! resolve whether a member overrides something, it stays silent.
//!
//! Adopted from the official `avoid_positional_boolean_parameters` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_dart_type, walk_function_decl, walk_stmt,
};

use crate::lint::semantic_scope::{SemanticState, resolved_super_param_type};

const RULE: &str = "avoid-positional-boolean-parameters";
const MESSAGE: &str = "'bool' parameters should be named parameters.";

#[derive(Default)]
pub struct AvoidPositionalBooleanParameters;

impl Rule for AvoidPositionalBooleanParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            model: &state.model,
            signatures: &state.signatures,
            container: None,
            fixed_signatures: Vec::new(),
            diags: Vec::new(),
        };
        for declaration in &program.declarations {
            collector.container = collector.enter(declaration);
            collector.visit_top_level_decl(declaration);
        }
        collector.diags
    }
}

/// The member-bearing declaration currently being walked.
struct Container<'p> {
    /// Interface identity whose inherited members can fix a signature; `None`
    /// for extensions, which never override.
    identity: Option<DeclarationIdentity>,
    is_interface: bool,
    members: &'p [ClassMember],
    superclass: Option<&'p DartType>,
}

struct Collector<'a, 'ctx, 'p> {
    ctx: &'a AnalyzeContext<'ctx>,
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    container: Option<Container<'p>>,
    /// Source ranges of member signatures fixed by an overridden member.
    fixed_signatures: Vec<(usize, usize)>,
    diags: Vec<Diagnostic>,
}

fn is_core_bool(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk {
                library: "dart:core",
                name: "bool"
            },
            ..
        }
    )
}

fn has_override(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|annotation| {
        annotation
            .name
            .last()
            .is_some_and(|name| name.name == "override")
    })
}

fn signature_range(start: usize, body: Option<&FunctionBody>, end: usize) -> (usize, usize) {
    (start, body.map_or(end, |body| body.span().start))
}

impl<'p> Collector<'_, '_, 'p> {
    /// Whether a member named `name` has its signature fixed by an inherited
    /// member. Unresolvable hierarchies count as fixed so the rule stays quiet.
    fn is_fixed(&self, name: &str, annotations: &[Annotation]) -> bool {
        if has_override(annotations) {
            return true;
        }
        let Some(container) = &self.container else {
            return false;
        };
        if !container.is_interface {
            return false;
        }
        let Some(identity) = &container.identity else {
            return true;
        };
        self.signatures
            .inherited_member_facts(identity, name)
            .is_none_or(|facts| !facts.is_empty())
    }

    fn in_fixed_signature(&self, span: &Span) -> bool {
        self.fixed_signatures
            .iter()
            .any(|(start, end)| *start <= span.start && span.end <= *end)
    }

    fn is_bool_param(&self, param: &FormalParam, super_constructor: Option<&str>) -> bool {
        if param.function_params.is_some() {
            return false;
        }
        if let Some(ty) = &param.param_type {
            return is_core_bool(&self.model.resolve_type(ty));
        }
        match param.initializing {
            Some(InitializingFormal::This) => self
                .field_type(&param.name.name)
                .is_some_and(|ty| is_core_bool(&self.model.resolve_type(ty))),
            Some(InitializingFormal::Super) => {
                let Some(container) = &self.container else {
                    return false;
                };
                let (Some(superclass), Some(constructor)) =
                    (container.superclass, super_constructor)
                else {
                    return false;
                };
                let superclass = self.model.resolve_type(superclass);
                resolved_super_param_type(
                    param,
                    &superclass,
                    constructor,
                    self.signatures,
                    self.model,
                )
                .is_some_and(|ty| is_core_bool(&ty))
            }
            None => false,
        }
    }

    /// The declared type of a field of the enclosing declaration.
    fn field_type(&self, name: &str) -> Option<&'p DartType> {
        let container = self.container.as_ref()?;
        container.members.iter().find_map(|member| match member {
            ClassMember::Field(field)
                if field
                    .declarators
                    .iter()
                    .any(|declarator| declarator.name.name == name) =>
            {
                field.field_type.as_ref()
            }
            _ => None,
        })
    }

    fn check_params(&mut self, params: &FormalParamList, super_constructor: Option<&str>) {
        let second = params
            .positional()
            .filter(|param| self.is_bool_param(param, super_constructor))
            .nth(1);
        if let Some(second) = second {
            self.diags
                .push(self.ctx.diagnostic(RULE, MESSAGE, second.span));
        }
    }

    fn enter(&self, declaration: &'p TopLevelDecl) -> Option<Container<'p>> {
        let interface = |name: &Identifier, members, superclass| Container {
            identity: self.model.resolve_name(name.name.as_str()),
            is_interface: true,
            members,
            superclass,
        };
        let container = match declaration {
            TopLevelDecl::Class(node) => {
                interface(&node.name, &node.members, node.extends.as_ref())
            }
            TopLevelDecl::MixinClass(node) => {
                interface(&node.name, &node.members, node.extends.as_ref())
            }
            TopLevelDecl::Mixin(node) => interface(&node.name, &node.members, None),
            TopLevelDecl::Enum(node) => interface(&node.name, &node.members, None),
            TopLevelDecl::ExtensionType(node) => interface(&node.name, &node.members, None),
            TopLevelDecl::Extension(node) => Container {
                identity: None,
                is_interface: false,
                members: &node.members,
                superclass: None,
            },
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => return None,
        };
        Some(container)
    }
}

impl Visitor for Collector<'_, '_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if !node.name.name.starts_with('_')
            && let FunctionKind::Function(params) = &node.kind
        {
            self.check_params(params, None);
        }
        walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        match node {
            ClassMember::Constructor(constructor) => {
                let private = constructor
                    .constructor_name
                    .as_ref()
                    .is_some_and(|name| name.name.starts_with('_'));
                if !private {
                    let super_constructor =
                        constructor.initializers.iter().find_map(|init| match init {
                            ConstructorInitializer::SuperCall { call_name, .. } => {
                                Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                            }
                            _ => None,
                        });
                    self.check_params(
                        &constructor.params,
                        Some(super_constructor.unwrap_or("new")),
                    );
                }
            }
            ClassMember::Method(method) => {
                if self.is_fixed(&method.name.name, &method.annotations) {
                    self.fixed_signatures.push(signature_range(
                        method.span.start,
                        method.body.as_ref(),
                        method.span.end,
                    ));
                } else if !method.name.name.starts_with('_') {
                    self.check_params(&method.params, None);
                }
            }
            ClassMember::Getter(getter) => {
                if self.is_fixed(&getter.name.name, &getter.annotations) {
                    self.fixed_signatures.push(signature_range(
                        getter.span.start,
                        getter.body.as_ref(),
                        getter.span.end,
                    ));
                }
            }
            ClassMember::Setter(setter) => {
                if self.is_fixed(&setter.name.name, &setter.annotations) {
                    self.fixed_signatures.push(signature_range(
                        setter.span.start,
                        setter.body.as_ref(),
                        setter.span.end,
                    ));
                }
            }
            ClassMember::Operator(operator) => {
                if self.is_fixed(&operator.op, &operator.annotations) {
                    self.fixed_signatures.push(signature_range(
                        operator.span.start,
                        operator.body.as_ref(),
                        operator.span.end,
                    ));
                }
            }
            ClassMember::Field(_) | ClassMember::Error(_) => {}
        }
        walk_class_member(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node
            && !function.name.name.starts_with('_')
        {
            self.check_params(&function.params, None);
        }
        walk_stmt(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node
            && !self.in_fixed_signature(&function.span)
        {
            let second = function
                .params
                .iter()
                .filter(|param| {
                    !param.is_named && is_core_bool(&self.model.resolve_type(&param.param_type))
                })
                .nth(1);
            if let Some(second) = second {
                let start = second.param_type.span().start;
                let end = second
                    .name
                    .as_ref()
                    .map_or(second.param_type.span().end, |name| name.span.end);
                self.diags
                    .push(self.ctx.diagnostic(RULE, MESSAGE, DiagSpan { start, end }));
            }
        }
        walk_dart_type(self, node);
    }
}
