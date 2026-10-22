//! Flags a positional `bool` parameter in a public signature.
//!
//! At a call site like `setVisible(true)` the literal says nothing about what
//! it means. The first positional `bool` parameter (required or optional) of a
//! public function, method, constructor, or `Function(...)` type is
//! reported; make it named instead:
//!
//! ```dart
//! void configure(int level, bool verbose) {}   // `bool verbose` flagged
//! void configure(int level, {bool verbose = false}) {}
//! ```
//!
//! Private functions, methods, and constructors (by their own name), getters,
//! method setters, and operators are skipped, as are methods whose signature
//! is fixed by an inherited member they override. Top-level setters are
//! checked like functions. `Function(...)` types are checked wherever they are
//! written, including in overriding signatures and private declarations;
//! legacy `typedef void F(bool b);` aliases and function-typed parameters
//! (`void f(bool b)` as a parameter) are not function types and are skipped.
//! Field and super parameters (`this.flag`, `super.flag`) take their declared
//! type. When falcon cannot resolve whether a method overrides something, it
//! stays silent.
//!
//! Adopted from the official `avoid_positional_boolean_parameters` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticState, resolved_super_param_type};
use crate::lint::support::annotations::has_override;

const MESSAGE: &str = "`bool` parameters should be named parameters.";

#[derive(Default)]
pub struct AvoidPositionalBooleanParameters;

impl Rule for AvoidPositionalBooleanParameters {
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
            container: None,
            diags: Vec::new(),
        }))
    }
}

/// The member-bearing declaration currently being walked.
struct Container<'p> {
    /// Interface identity whose inherited members can fix a signature; `None`
    /// for extensions, which never override.
    identity: Option<DeclarationIdentity>,
    is_interface: bool,
    members: AstChildren<'p, ClassMember<'p>>,
    superclass: Option<DartType<'p>>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    container: Option<Container<'a>>,
    diags: Vec<Diagnostic>,
}

fn is_core_bool(ty: &ResolvedType) -> bool {
    matches!(
        ty.view(),
        ResolvedTypeView::Interface {
            identity,
            ..
        } if identity.is_sdk("dart:core", "bool")
    )
}


impl<'a> Collector<'a> {
    /// Whether a member named `name` has its signature fixed by an inherited
    /// member. Unresolvable hierarchies count as fixed so the rule stays quiet.
    fn is_fixed(&self, name: &str, annotations: AstChildren<'a, Annotation<'a>>) -> bool {
        if has_override(annotations, self.state.model) {
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
        self.state
            .signatures
            .inherited_member_facts(identity, name)
            .is_none_or(|facts| !facts.is_empty())
    }

    fn is_bool_param(
        &self,
        param: FormalParam<'a>,
        params: FormalParamList<'a>,
        super_constructor: Option<&str>,
    ) -> bool {
        if param.function_params().is_some() {
            return false;
        }
        if let Some(ty) = param.param_type() {
            return is_core_bool(&self.state.model.resolve_type(ty));
        }
        match param.initializing() {
            Some(InitializingFormal::This) => self
                .field_type(param.name().name())
                .is_some_and(|ty| is_core_bool(&self.state.model.resolve_type(ty))),
            Some(InitializingFormal::Super) => {
                let Some(container) = &self.container else {
                    return false;
                };
                let (Some(superclass), Some(constructor)) =
                    (container.superclass, super_constructor)
                else {
                    return false;
                };
                let superclass = self.state.model.resolve_type(superclass);
                resolved_super_param_type(
                    param,
                    params,
                    &superclass,
                    constructor,
                    &self.state.signatures,
                    &self.state.model,
                )
                .is_some_and(|ty| is_core_bool(&ty))
            }
            None => false,
        }
    }

    /// The declared type of a field of the enclosing declaration.
    fn field_type(&self, name: &str) -> Option<DartType<'a>> {
        let container = self.container.as_ref()?;
        container.members.clone().find_map(|member| match member {
            ClassMember::Field(field)
                if field
                    .declarators()
                    .any(|declarator| declarator.name().name() == name) =>
            {
                field.field_type()
            }
            _ => None,
        })
    }

    fn check_params(&mut self, params: FormalParamList<'a>, super_constructor: Option<&str>) {
        if let Some(first) = params
            .positional()
            .find(|param| self.is_bool_param(*param, params, super_constructor))
        {
            self.diags
                .push(self.ctx.diagnostic(MESSAGE, first.span()));
        }
    }

    fn enter(&self, declaration: TopLevelDecl<'a>) -> Option<Container<'a>> {
        let interface = |name: Identifier<'a>, members, superclass| Container {
            identity: self.state.model.resolve_name(name.name()),
            is_interface: true,
            members,
            superclass,
        };
        let container = match declaration {
            TopLevelDecl::Class(node) => interface(node.name(), node.members(), node.extends()),
            TopLevelDecl::MixinClass(node) => {
                interface(node.name(), node.members(), node.extends())
            }
            TopLevelDecl::Mixin(node) => interface(node.name(), node.members(), None),
            TopLevelDecl::Enum(node) => interface(node.name(), node.members(), None),
            TopLevelDecl::ExtensionType(node) => interface(node.name(), node.members(), None),
            TopLevelDecl::Extension(node) => Container {
                identity: None,
                is_interface: false,
                members: node.members(),
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

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_library_directive(&mut self, _: LibraryDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_part_of_directive(&mut self, _: PartOfDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_part_directive(&mut self, _: PartDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_import(&mut self, _: ImportDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_export(&mut self, _: ExportDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.container = self.enter(node);
        StructuralTraversal::Continue
    }

    fn visit_primary_constructor(&mut self, node: PrimaryConstructor<'a>) -> StructuralTraversal {
        if !node
            .constructor_name()
            .is_some_and(|name| name.name().starts_with('_'))
        {
            let target = self.container.as_ref().and_then(|container| {
                container.members.clone().find_map(|member| match member {
                    ClassMember::PrimaryConstructorBody(body) => {
                        body.initializers().find_map(|init| match init {
                            ConstructorInitializer::SuperCall(call) => {
                                Some(call.call_name().map_or("new", |name| name.name()))
                            }
                            _ => None,
                        })
                    }
                    _ => None,
                })
            });
            self.check_params(node.params(), Some(target.unwrap_or("new")));
        }
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if !node.name().name().starts_with('_')
            && let Some(params) = node.params()
        {
            self.check_params(params, None);
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        match node {
            ClassMember::Constructor(constructor) => {
                let private = constructor
                    .constructor_name()
                    .is_some_and(|name| name.name().starts_with('_'));
                if !private {
                    let super_constructor =
                        constructor.initializers().find_map(|init| match init {
                            ConstructorInitializer::SuperCall(call) => {
                                Some(call.call_name().map_or("new", |name| name.name()))
                            }
                            _ => None,
                        });
                    self.check_params(
                        constructor.params(),
                        Some(super_constructor.unwrap_or("new")),
                    );
                }
            }
            ClassMember::Method(method) => {
                if !method.name().name().starts_with('_')
                    && !self.is_fixed(method.name().name(), method.annotations())
                {
                    self.check_params(method.params(), None);
                }
            }
            ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Operator(_)
            | ClassMember::Field(_)
            | ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Error(_) => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node
            && !function.name().name().starts_with('_')
        {
            self.check_params(function.params(), None);
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Function(function) = node {
            let first = function.params().find(|param| {
                !param.is_named() && is_core_bool(&self.state.model.resolve_type(param.param_type()))
            });
            if let Some(first) = first {
                let start = first.param_type().span().start;
                let end = first
                    .name()
                    .map_or(first.param_type().span().end, |name| name.span().end);
                self.diags
                    .push(self.ctx.diagnostic(MESSAGE, DiagSpan { start, end }));
            }
        }
        StructuralTraversal::Continue
    }
}
}
