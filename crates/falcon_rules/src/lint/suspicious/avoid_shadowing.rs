//! Flags local bindings and parameters that hide an enclosing type's member.
//!
//! Renaming the local keeps unqualified member references unambiguous. Checks
//! variables, local functions, closure and executable parameters, loop bindings,
//! catch bindings and declaration patterns inside type members. Declared static
//! members are visible in static bodies; inherited instance members are checked
//! only in instance bodies using resolved declaration identities. Initializing
//! formals, type-signature parameter names and primary constructor declarations
//! are excluded. This rule does not check shadowing of other locals or globals.
//! Unknown inherited types are not guessed from their spelling.
//! SDK members are checked only when the signature inventory proves an instance
//! member; unclassified SDK name-table entries are deliberately not used.

use std::borrow::Cow;
use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralNode, StructuralRuleVisitor, StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidShadowing;

impl Rule for AvoidShadowing {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types);
        let signatures = ctx
            .signatures
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(SignatureIndex::from_program(program, &model)));
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            members: HashMap::new(),
            receiver: None,
            instance: false,
            diagnostics: Vec::new(),
        }))
    }
}

pub(super) fn has_instance_member(
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    receiver: &ResolvedType,
    name: &str,
) -> bool {
    let receiver = match receiver {
        ResolvedType::TypeParameter {
            bound,
            nullable: false,
            ..
        } => bound.as_ref(),
        receiver => receiver,
    };
    if !matches!(
        receiver,
        ResolvedType::Interface {
            nullable: false,
            ..
        } | ResolvedType::Record {
            nullable: false,
            ..
        }
    ) {
        return false;
    }
    receiver.record_field(name).is_some()
        || signatures
            .resolved_member_owner(receiver, name, model)
            .is_some()
        || signatures.resolved_field(receiver, name, model).is_some()
        || signatures
            .resolved_writable_field(receiver, name, model)
            .is_some()
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: Cow<'a, SignatureIndex>,
    members: HashMap<&'a str, bool>,
    receiver: Option<ResolvedType>,
    instance: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn check(&mut self, name: Identifier<'a>) {
        if name.name() == "_" || name.is_missing() {
            return;
        }
        let declared = self
            .members
            .get(name.name())
            .is_some_and(|is_static| *is_static || self.instance);
        // `State.build(BuildContext context)` is idiomatic and both names denote the same context.
        let inherited = !declared
            && name.name() != "context"
            && self.instance
            && self.receiver.as_ref().is_some_and(|receiver| {
                has_instance_member(&self.model, &self.signatures, receiver, name.name())
            });
        if declared || inherited {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-shadowing",
                "This binding shadows an enclosing member. Rename the binding.",
                name.span(),
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.members.clear();
        self.receiver = None;
        let Some(members) = crate::lint::declarations::members_of(node) else {
            return StructuralTraversal::SkipChildren;
        };
        let name = match node {
            TopLevelDecl::Class(node) => Some(node.name()),
            TopLevelDecl::Mixin(node) => Some(node.name()),
            TopLevelDecl::MixinClass(node) => Some(node.name()),
            TopLevelDecl::Enum(node) => Some(node.name()),
            TopLevelDecl::ExtensionType(node) => Some(node.name()),
            _ => None,
        };
        self.receiver = name
            .and_then(|name| self.model.resolve_name(name.name()))
            .map(ResolvedType::interface_identity);
        if let TopLevelDecl::Extension(extension) = node {
            let mut parameters = TypeParameterScope::default();
            parameters.push(extension.type_params(), &self.model);
            self.receiver = Some(self.model.resolve_type_in(extension.on_type(), &parameters));
        }
        if let TopLevelDecl::Enum(declaration) = node {
            for variant in declaration.variants() {
                self.members.insert(variant.name().name(), true);
            }
        }
        self.instance = false;
        for member in members {
            match member {
                ClassMember::Field(field) => {
                    for declarator in field.declarators() {
                        self.members.insert(declarator.name().name(), field.is_static());
                    }
                }
                ClassMember::Method(method) => {
                    self.members.insert(method.name().name(), method.is_static());
                }
                ClassMember::Getter(getter) => {
                    self.members.insert(getter.name().name(), getter.is_static());
                }
                ClassMember::Setter(setter) => {
                    self.members.insert(setter.name().name(), setter.is_static());
                }
                _ => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if !matches!(node, ClassMember::Field(_) | ClassMember::Constructor(_))
            && falcon_syntax::cst::visit::declared_member_body(node).is_none()
        {
            return StructuralTraversal::SkipChildren;
        }
        self.instance = match node {
            ClassMember::Method(node) => !node.is_static(),
            ClassMember::Getter(node) => !node.is_static(),
            ClassMember::Setter(node) => !node.is_static(),
            ClassMember::Field(field) => field.is_late() && !field.is_static(),
            ClassMember::Constructor(node) => !node.is_factory(),
            ClassMember::Operator(_) | ClassMember::PrimaryConstructorBody(_) => true,
            ClassMember::Error(_) => false,
        };
        StructuralTraversal::Continue
    }

    fn visit_primary_constructor(&mut self, _node: PrimaryConstructor<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
        match parent {
            StructuralNode::DartType(_) | StructuralNode::Annotation(_) => false,
            StructuralNode::FormalParam(_) => matches!(child, StructuralNode::Expr(_)),
            StructuralNode::PatternAssignStmt(_) => !matches!(child, StructuralNode::Pattern(_)),
            _ => true,
        }
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        for declarator in node.declarators() {
            self.check(declarator.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_local_func_decl(&mut self, node: LocalFuncDecl<'a>) -> StructuralTraversal {
        self.check(node.name());
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        if node.initializing().is_none() {
            self.check(node.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        if let Pattern::Variable(variable) = node {
            self.check(variable.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_for_init(&mut self, node: ForInit<'a>) -> StructuralTraversal {
        if let ForInit::ForIn(node) = node && node.declares_variable() {
            self.check(node.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_catch_clause(&mut self, node: CatchClause<'a>) -> StructuralTraversal {
        if let Some(name) = node.exception_var() {
            self.check(name);
        }
        if let Some(name) = node.stack_trace_var() {
            self.check(name);
        }
        StructuralTraversal::Continue
    }
}
}
