//! Reports uses of deprecated declarations from their own package.
//!
//! Canonical project identities and lexical scopes prove references; unknown
//! receivers and ambiguous imports remain silent. Deprecated declarations form
//! a usage frontier, so their bodies and metadata are exempt. Deprecation
//! messages are trimmed and punctuated like the analyzer's messages.
//! Package membership is checked only for proven deprecations and cached per source file.
//!
//! Adopted from the official `deprecated_member_use_from_same_package` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, QualifiedName, ResolvedType, Rule,
    SemanticModel, SignatureIndex, TypeParameterScope, evaluate_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_annotation, walk_class_member, walk_constructor_decl, walk_dart_type, walk_expr,
    walk_formal_param, walk_function_decl, walk_method_decl, walk_stmt, walk_top_level_decl,
};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

const ID: &str = "deprecated-member-use-from-same-package";

#[derive(Default)]
pub struct DeprecatedMemberUseFromSamePackage;

impl Rule for DeprecatedMemberUseFromSamePackage {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(package) = crate::cross_file::enclosing_package(ctx.file_path) else {
            return Vec::new();
        };
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut frontier = Frontier {
            model: state.model,
            signatures: ctx.signatures,
            owner: None,
            superclass: None,
            declaration: None,
            metadata_scope: false,
            in_metadata: false,
            excluded: Vec::new(),
            metadata: Vec::new(),
            writes: HashSet::new(),
            reads: HashSet::new(),
            parameters: TypeParameterScope::default(),
            types: Vec::new(),
        };
        frontier.visit_program(program);
        let declarations = frontier.excluded.len();
        for (declaration, metadata) in frontier.metadata {
            if frontier.excluded[..declarations].iter().any(|excluded| {
                excluded.start <= declaration.start && excluded.end >= declaration.end
            }) {
                frontier.excluded.push(metadata);
            }
        }
        let mut uses = Uses {
            ctx,
            package_root: package.lib_root,
            same_package: HashMap::new(),
            excluded: frontier.excluded,
            writes: frontier.writes,
            reads: frontier.reads,
            diagnostics: Vec::new(),
            reported: HashSet::new(),
        };
        for target in &frontier.types {
            uses.report(target);
        }
        visit_program(&mut uses, program, state);
        uses.diagnostics
    }
}

/// Only resolved dart:core annotations count, not user-defined lookalikes.
fn deprecation(
    annotations: &[Annotation],
    model: SemanticModel<'_>,
    signatures: Option<&SignatureIndex>,
) -> Option<String> {
    for annotation in annotations {
        if annotation.constructor_name.is_some() {
            continue;
        }
        let Some(name) = QualifiedName::from_identifiers(&annotation.name) else {
            continue;
        };
        if annotation.args.is_none()
            && model
                .resolve_sdk_member(name)
                .is_some_and(|id| id.is_sdk("dart:core", "deprecated"))
        {
            return Some(String::new());
        }
        if annotation.args.is_some()
            && model
                .resolve_name(name)
                .is_some_and(|id| id.is_sdk("dart:core", "Deprecated"))
        {
            // Dynamic/unevaluated metadata cannot prove a deprecated constant.
            let expression = annotation.args.as_ref()?.positional.first()?;
            let value;
            let message = if let Expr::StringLit(message) = expression {
                message.value.as_str()
            } else {
                let Some(ConstantValue::String(message)) = signatures.and_then(|signatures| {
                    evaluate_constant(expression, None, &HashMap::new(), &model, signatures)
                }) else {
                    continue;
                };
                value = message;
                value.as_str()
            };
            let message = message.trim();
            return Some(if message.is_empty() || message == "." {
                String::new()
            } else if message.ends_with(['.', '?', '!']) {
                message.to_owned()
            } else {
                format!("{message}.")
            });
        }
    }
    None
}

struct Frontier<'a> {
    model: SemanticModel<'a>,
    signatures: Option<&'a SignatureIndex>,
    owner: Option<DeclarationIdentity>,
    superclass: Option<DeclarationIdentity>,
    declaration: Option<Span>,
    metadata_scope: bool,
    in_metadata: bool,
    excluded: Vec<Span>,
    metadata: Vec<(Span, Span)>,
    writes: HashSet<usize>,
    reads: HashSet<usize>,
    parameters: TypeParameterScope,
    types: Vec<Target>,
}

impl Visitor for Frontier<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let previous = self.declaration.replace(*node.span());
        let previous_scope = std::mem::replace(
            &mut self.metadata_scope,
            matches!(
                node,
                TopLevelDecl::Function(_) | TopLevelDecl::Variable(_) | TopLevelDecl::TypeAlias(_)
            ),
        );
        let parameters: &[TypeParam] = match node {
            TopLevelDecl::Class(n) => &n.type_params,
            TopLevelDecl::Mixin(n) => &n.type_params,
            TopLevelDecl::MixinClass(n) => &n.type_params,
            TopLevelDecl::Enum(n) => &n.type_params,
            TopLevelDecl::Extension(n) => &n.type_params,
            TopLevelDecl::ExtensionType(n) => &n.type_params,
            TopLevelDecl::ClassTypeAlias(n) => &n.type_params,
            TopLevelDecl::TypeAlias(n) => &n.type_params,
            _ => &[],
        };
        self.parameters.push(parameters, &self.model);
        if let TopLevelDecl::Enum(enumeration) = node {
            for variant in &enumeration.variants {
                for annotation in &variant.annotations {
                    self.metadata.push((variant.span, annotation.span));
                }
                if let Some(owner) = self.model.resolve_name(&enumeration.name.name) {
                    let constructor = variant.constructor_name.as_ref();
                    let target = constructor_target(
                        owner,
                        constructor.map_or("new", |name| name.name.as_str()),
                        constructor.map_or(variant.name.span, |name| name.span),
                    );
                    if let Some(args) = &variant.args {
                        for argument in &args.named {
                            self.types.push(Target {
                                owner: target.owner.clone(),
                                selector: format!(
                                    "param:{}:{}",
                                    target.selector, argument.name.name
                                ),
                                name: argument.name.name.clone(),
                                span: argument.name.span,
                            });
                        }
                    }
                    self.types.push(target);
                }
            }
        }
        let (owner, superclass, members, class_name) = match node {
            TopLevelDecl::Class(n) => (
                self.model.resolve_name(&n.name.name),
                n.extends.as_ref().filter(|_| n.with_clause.is_empty()),
                Some(n.members.as_slice()),
                Some(&n.name),
            ),
            _ => (None, None, None, None),
        };
        let superclass =
            superclass.and_then(
                |ty| match self.model.resolve_type_in(ty, &self.parameters) {
                    ResolvedType::Interface { identity, .. } => Some(identity),
                    _ => None,
                },
            );
        let old_owner = std::mem::replace(&mut self.owner, owner);
        let old_superclass = std::mem::replace(&mut self.superclass, superclass);
        if let (Some(members), Some(class_name), Some(superclass)) =
            (members, class_name, &self.superclass)
            && !members
                .iter()
                .any(|member| matches!(member, ClassMember::Constructor(_)))
        {
            self.types.push(constructor_target(
                superclass.clone(),
                "new",
                class_name.span,
            ));
        }
        walk_top_level_decl(self, node);
        self.declaration = previous;
        self.parameters.pop();
        self.owner = old_owner;
        self.superclass = old_superclass;
        self.metadata_scope = previous_scope;
    }
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.parameters.push(&node.type_params, &self.model);
        walk_function_decl(self, node);
        self.parameters.pop();
    }
    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.parameters.push(&node.type_params, &self.model);
        walk_method_decl(self, node);
        self.parameters.pop();
    }
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let mut explicit = false;
        for initializer in &node.initializers {
            let (owner, name, args, span) = match initializer {
                ConstructorInitializer::SuperCall {
                    call_name,
                    args,
                    span,
                } => (self.superclass.as_ref(), call_name.as_ref(), args, *span),
                ConstructorInitializer::ThisCall {
                    call_name,
                    args,
                    span,
                } => (self.owner.as_ref(), call_name.as_ref(), args, *span),
                _ => continue,
            };
            explicit = true;
            if let Some(owner) = owner {
                let target = constructor_target(
                    owner.clone(),
                    name.map_or("new", |n| n.name.as_str()),
                    name.map_or(span, |n| n.span),
                );
                for argument in &args.named {
                    self.types.push(Target {
                        owner: target.owner.clone(),
                        selector: format!("param:{}:{}", target.selector, argument.name.name),
                        name: argument.name.name.clone(),
                        span: argument.name.span,
                    });
                }
                self.types.push(target);
            }
        }
        if !explicit
            && !node.is_factory
            && let Some(superclass) = &self.superclass
        {
            self.types.push(constructor_target(
                superclass.clone(),
                "new",
                node.name.span,
            ));
        }
        if let Some(redirect) = &node.redirect
            && let ResolvedType::Interface { identity, .. } = self
                .model
                .resolve_type_in(&redirect.type_, &self.parameters)
        {
            self.types.push(constructor_target(
                identity,
                redirect
                    .constructor_name
                    .as_ref()
                    .map_or("new", |n| n.name.as_str()),
                redirect
                    .constructor_name
                    .as_ref()
                    .map_or(redirect.span, |n| n.span),
            ));
        }
        // The parser may retain an unprefixed `.named` suffix in the type.
        if let Some(redirect) = &node.redirect
            && redirect.constructor_name.is_none()
            && let DartType::Named(named) = &redirect.type_
            && let [class_name, constructor] = named.segments.as_slice()
            && let Some(owner) = self.model.resolve_name(&class_name.name)
            && self
                .signatures
                .is_some_and(|index| index.constructor(&owner, &constructor.name).is_some())
        {
            self.types.push(constructor_target(
                owner,
                &constructor.name,
                constructor.span,
            ));
        }
        walk_constructor_decl(self, node);
    }
    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            self.parameters.push(&function.type_params, &self.model);
            walk_dart_type(self, node);
            self.parameters.pop();
            return;
        }
        if let DartType::Named(named) = node
            && let ResolvedType::Interface { identity, .. } =
                self.model.resolve_type_in(node, &self.parameters)
            && let Some(name) = named.segments.last()
        {
            let identity = QualifiedName::from_identifiers(&named.segments)
                .and_then(|name| self.model.resolve_name(name))
                .unwrap_or(identity);
            self.types.push(Target {
                name: identity.name().to_owned(),
                owner: identity,
                selector: String::new(),
                span: name.span,
            });
        }
        walk_dart_type(self, node);
    }
    fn visit_class_member(&mut self, node: &ClassMember) {
        let previous = self.declaration.replace(*node.span());
        // Field annotations are not traversed by the generic member walker.
        if let ClassMember::Field(field) = node {
            for annotation in &field.annotations {
                self.visit_annotation(annotation);
            }
        }
        walk_class_member(self, node);
        self.declaration = previous;
    }
    fn visit_formal_param(&mut self, node: &FormalParam) {
        let previous = self.declaration.replace(node.span);
        walk_formal_param(self, node);
        self.declaration = previous;
    }
    fn visit_annotation(&mut self, node: &Annotation) {
        let variant = self
            .metadata
            .iter()
            .find(|(_, annotation)| *annotation == node.span)
            .map(|(declaration, _)| *declaration);
        let declaration = variant.or(self.declaration);
        if deprecation(std::slice::from_ref(node), self.model, self.signatures).is_some()
            && let Some(span) = declaration
        {
            self.excluded.push(span);
        }
        if variant.is_none()
            && let Some(declaration) = declaration
        {
            self.metadata.push((declaration, node.span));
        }
        if let Some(name) = QualifiedName::from_identifiers(&node.name)
            && let Some(owner) = self
                .model
                .resolve_name(name)
                .or_else(|| self.model.resolve_value(name))
            && let Some(identifier) = node.name.last()
        {
            self.types.push(Target {
                owner: owner.clone(),
                selector: String::new(),
                name: owner.name().to_owned(),
                span: identifier.span,
            });
            if let Some(args) = &node.args {
                let constructor = node.constructor_name.as_ref();
                let target = constructor_target(
                    owner,
                    constructor.map_or("new", |n| n.name.as_str()),
                    constructor.map_or(identifier.span, |n| n.span),
                );
                for argument in &args.named {
                    self.types.push(Target {
                        owner: target.owner.clone(),
                        selector: format!("param:{}:{}", target.selector, argument.name.name),
                        name: argument.name.name.clone(),
                        span: argument.name.span,
                    });
                }
                self.types.push(target);
            }
        }
        let previous = std::mem::replace(&mut self.in_metadata, self.metadata_scope);
        walk_annotation(self, node);
        self.in_metadata = previous;
    }
    fn visit_stmt(&mut self, node: &Stmt) {
        let previous = std::mem::replace(&mut self.metadata_scope, false);
        walk_stmt(self, node);
        self.metadata_scope = previous;
    }
    fn visit_expr(&mut self, node: &Expr) {
        if self.in_metadata
            && let Expr::Ident(identifier) = node
            && self.parameters.lookup(&identifier.name).is_none()
            && let Some(owner) = self
                .model
                .resolve_value(&identifier.name)
                .or_else(|| self.model.resolve_name(&identifier.name))
        {
            self.types.push(Target {
                owner,
                selector: String::new(),
                name: identifier.name.clone(),
                span: identifier.span,
            });
        }
        match node {
            Expr::Assign { target, op, .. } => {
                self.writes.insert(target.span().start);
                if *op != AssignOp::Eq {
                    self.reads.insert(target.span().start);
                }
            }
            Expr::Unary {
                op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                operand,
                ..
            }
            | Expr::PostfixIncDec { operand, .. } => {
                self.writes.insert(operand.span().start);
                self.reads.insert(operand.span().start);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

struct Uses<'a> {
    ctx: &'a AnalyzeContext<'a>,
    package_root: std::path::PathBuf,
    same_package: HashMap<std::path::PathBuf, bool>,
    excluded: Vec<Span>,
    writes: HashSet<usize>,
    reads: HashSet<usize>,
    diagnostics: Vec<Diagnostic>,
    reported: HashSet<(usize, usize)>,
}

struct Target {
    owner: DeclarationIdentity,
    selector: String,
    name: String,
    span: Span,
}

fn constructor_target(owner: DeclarationIdentity, name: &str, span: Span) -> Target {
    Target {
        name: if name == "new" {
            owner.name().to_owned()
        } else {
            format!("{}.{name}", owner.name())
        },
        owner,
        selector: format!("new:{name}"),
        span,
    }
}

impl Uses<'_> {
    fn report(&mut self, target: &Target) {
        if self
            .excluded
            .iter()
            .any(|span| span.start <= target.span.start && target.span.end <= span.end)
        {
            return;
        }
        let Some(identities) = self.ctx.identities else {
            return;
        };
        let Some((path, annotations)) = identities
            .source_annotations(&target.owner, &target.selector)
            .or_else(|| {
                if target.selector.is_empty() || target.selector == "set:" {
                    identities.source_annotations(&target.owner, "variable")
                } else {
                    None
                }
            })
            .or_else(|| {
                let (prefix, name) = target
                    .selector
                    .strip_prefix("static:")
                    .map_or(("", target.selector.as_str()), |name| ("static:", name));
                let name = name.strip_prefix("set:").unwrap_or(name);
                identities.source_annotations(&target.owner, &format!("{prefix}field:{name}"))
            })
        else {
            return;
        };
        let model = SemanticModel::new(path, identities, self.ctx.types);
        let Some(message) = deprecation(annotations, model, self.ctx.signatures) else {
            return;
        };
        let same_package = if let Some(&same_package) = self.same_package.get(path) {
            same_package
        } else {
            let same_package = crate::cross_file::enclosing_package(path)
                .is_some_and(|package| package.lib_root == self.package_root);
            self.same_package.insert(path.to_owned(), same_package);
            same_package
        };
        if !same_package {
            return;
        }
        if !self.reported.insert((target.span.start, target.span.end)) {
            return;
        }
        let base = format!("'{}' is deprecated and shouldn't be used.", target.name);
        self.diagnostics.push(self.ctx.diagnostic(
            ID,
            if message.is_empty() {
                base
            } else {
                format!("{base} {message}")
            },
            target.span,
        ));
    }

    fn member(
        &self,
        receiver: &ResolvedType,
        name: &str,
        span: Span,
        state: &SemanticState<'_>,
        write: bool,
    ) -> Option<Target> {
        let owner = if write {
            let ResolvedType::Interface { identity, .. } = receiver else {
                return None;
            };
            // A directly declared setter/field is proven by its source metadata.
            let identities = self.ctx.identities?;
            if identities
                .source_annotations(identity, &format!("set:{name}"))
                .is_some()
                || identities
                    .source_annotations(identity, &format!("field:{name}"))
                    .is_some()
            {
                identity.clone()
            } else {
                return None;
            }
        } else {
            state
                .signatures
                .resolved_member_owner(receiver, name, &state.model)?
        };
        let selector = if write {
            format!("set:{name}")
        } else {
            name.to_owned()
        };
        Some(Target {
            owner,
            selector,
            name: name.to_owned(),
            span,
        })
    }

    fn target(
        &self,
        node: &Expr,
        state: &SemanticState<'_>,
        access: Option<bool>,
    ) -> Option<Target> {
        let mut node = node;
        while let Expr::GenericInstantiation { target, .. } = node {
            node = target;
        }
        let write = access.unwrap_or_else(|| self.writes.contains(&node.span().start));
        match node {
            Expr::Ident(id) if !state.environment.is_bound(&id.name) => {
                if let Some(receiver) = &state.enclosing_type {
                    return self.member(receiver, &id.name, id.span, state, write);
                }
                Some(Target {
                    owner: state
                        .model
                        .resolve_value(&id.name)
                        .or_else(|| state.model.resolve_name(&id.name))?,
                    selector: if write {
                        "set:".to_owned()
                    } else {
                        String::new()
                    },
                    name: id.name.clone(),
                    span: id.span,
                })
            }
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(prefix) = object.as_ref()
                    && !state.environment.is_bound(&prefix.name)
                {
                    let qualified = QualifiedName::prefixed(&prefix.name, &field.name);
                    if let Some(owner) = state
                        .model
                        .resolve_value(qualified)
                        .or_else(|| state.model.resolve_name(qualified))
                    {
                        return Some(Target {
                            owner,
                            selector: if write {
                                "set:".to_owned()
                            } else {
                                String::new()
                            },
                            name: field.name.clone(),
                            span: field.span,
                        });
                    }
                    if let Some(owner) = state.model.resolve_name(&prefix.name) {
                        let selector =
                            if state.signatures.constructor(&owner, &field.name).is_some() {
                                format!("new:{}", field.name)
                            } else if write {
                                format!("static:set:{}", field.name)
                            } else {
                                format!("static:{}", field.name)
                            };
                        return Some(Target {
                            owner,
                            selector,
                            name: field.name.clone(),
                            span: field.span,
                        });
                    }
                }
                let mut target = self.member(
                    &state.infer(object),
                    &field.name,
                    *node.span(),
                    state,
                    write,
                )?;
                target.span = field.span;
                Some(target)
            }
            _ => None,
        }
    }

    fn named_arguments(&mut self, target: &Target, args: &ArgList) {
        for argument in &args.named {
            let name = &argument.name;
            self.report(&Target {
                owner: target.owner.clone(),
                selector: format!("param:{}:{}", target.selector, name.name),
                name: name.name.clone(),
                span: name.span,
            });
        }
    }
}

fn binary_operator(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::IntDiv => "~/",
        BinaryOp::Mod => "%",
        BinaryOp::EqEq | BinaryOp::NotEq => "==",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce => return None,
    })
}

impl SemanticRuleVisitor for Uses<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Some(target) = self.target(node, state, None) {
            self.report(&target);
        }
        if self.reads.contains(&node.span().start)
            && let Some(target) = self.target(node, state, Some(false))
        {
            self.report(&target);
        }
        let operator = match node {
            Expr::Binary {
                op, left, right, ..
            } => {
                if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq)
                    && (matches!(state.infer(left), ResolvedType::Null)
                        || matches!(state.infer(right), ResolvedType::Null))
                {
                    None
                } else {
                    binary_operator(*op).map(|name| (left.as_ref(), name, 1))
                }
            }
            Expr::Assign { op, target, .. } => {
                let binary = match op {
                    AssignOp::PlusEq => BinaryOp::Add,
                    AssignOp::MinusEq => BinaryOp::Sub,
                    AssignOp::MulEq => BinaryOp::Mul,
                    AssignOp::DivEq => BinaryOp::Div,
                    AssignOp::IntDivEq => BinaryOp::IntDiv,
                    AssignOp::ModEq => BinaryOp::Mod,
                    AssignOp::AndEq => BinaryOp::BitAnd,
                    AssignOp::OrEq => BinaryOp::BitOr,
                    AssignOp::XorEq => BinaryOp::BitXor,
                    AssignOp::ShlEq => BinaryOp::Shl,
                    AssignOp::ShrEq => BinaryOp::Shr,
                    AssignOp::UShrEq => BinaryOp::UShr,
                    AssignOp::Eq | AssignOp::NullCoalesceEq => BinaryOp::NullCoalesce,
                };
                binary_operator(binary).map(|name| (target.as_ref(), name, 1))
            }
            Expr::Unary { op, operand, .. } => match op {
                UnaryOp::Minus => Some((operand.as_ref(), "-", 0)),
                UnaryOp::Tilde => Some((operand.as_ref(), "~", 0)),
                UnaryOp::PlusPlus => Some((operand.as_ref(), "+", 1)),
                UnaryOp::MinusMinus => Some((operand.as_ref(), "-", 1)),
                UnaryOp::Bang => None,
            },
            Expr::PostfixIncDec { op, operand, .. } => Some((
                operand.as_ref(),
                if *op == PostfixIncDec::Increment {
                    "+"
                } else {
                    "-"
                },
                1,
            )),
            Expr::Index { object, .. } => Some((
                object.as_ref(),
                if self.writes.contains(&node.span().start) {
                    "[]="
                } else {
                    "[]"
                },
                if self.writes.contains(&node.span().start) {
                    2
                } else {
                    1
                },
            )),
            _ => None,
        };
        let index_read = if let Expr::Index { object, .. } = node
            && self.reads.contains(&node.span().start)
            && self.writes.contains(&node.span().start)
        {
            Some((object.as_ref(), "[]", 1))
        } else {
            None
        };
        for (receiver, name, arity) in [operator, index_read].into_iter().flatten() {
            if let Some(owner) =
                state
                    .signatures
                    .resolved_member_owner(&state.infer(receiver), name, &state.model)
            {
                self.report(&Target {
                    owner,
                    selector: format!("op:{arity}:{name}"),
                    name: name.to_owned(),
                    span: *node.span(),
                });
            }
        }
        match node {
            Expr::Call { callee, args, .. } => {
                if let Some(mut target) = self.target(callee, state, None) {
                    if target.selector.is_empty()
                        && state.signatures.constructor(&target.owner, "new").is_some()
                    {
                        target.selector = "new:new".to_owned();
                        self.report(&target);
                    }
                    self.named_arguments(&target, args);
                }
            }
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                if let ResolvedType::Interface { identity, .. } = state
                    .model
                    .resolve_type_in(dart_type, &state.type_parameters)
                {
                    let name = constructor_name
                        .as_ref()
                        .map_or("new", |id| id.name.as_str());
                    let span = constructor_name
                        .as_ref()
                        .map_or(*dart_type.span(), |id| id.span);
                    let target = Target {
                        name: if name == "new" {
                            identity.name().to_owned()
                        } else {
                            format!("{}.{name}", identity.name())
                        },
                        owner: identity,
                        selector: format!("new:{name}"),
                        span,
                    };
                    self.report(&target);
                    self.named_arguments(&target, args);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, SignatureIndex, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    fn analyze(source: &str, path: &Path) -> Vec<Diagnostic> {
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        DeprecatedMemberUseFromSamePackage.analyze(&program, &ctx)
    }

    fn fixture_path() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/corpus/deprecated-member-use-from-same-package/bad.dart"
        ))
    }

    #[test]
    fn normalizes_messages() {
        for (annotation, expected) in [
            ("@deprecated", "'old' is deprecated and shouldn't be used."),
            (
                "@Deprecated('  Use fresh  ')",
                "'old' is deprecated and shouldn't be used. Use fresh.",
            ),
            (
                "@Deprecated('Use fresh!')",
                "'old' is deprecated and shouldn't be used. Use fresh!",
            ),
            (
                "@Deprecated(' . ')",
                "'old' is deprecated and shouldn't be used.",
            ),
            (
                "@Deprecated('  ')",
                "'old' is deprecated and shouldn't be used.",
            ),
        ] {
            let source = format!("{annotation}\nvoid old() {{}}\nvoid use() {{ old(); }}");
            let diagnostics = analyze(&source, fixture_path());
            assert_eq!(diagnostics.len(), 1, "{annotation}");
            assert_eq!(diagnostics[0].message, expected);
        }
    }

    #[test]
    fn requires_an_enclosing_package() {
        assert!(
            analyze(
                "@deprecated void old() {} void use() { old(); }",
                Path::new("/not-a-dart-package/example.dart")
            )
            .is_empty()
        );
    }

    #[test]
    fn ignores_annotation_and_parameter_shadowing() {
        for source in [
            "class Deprecated { const Deprecated(String message); } @Deprecated('fake') void old() {} void use() { old(); }",
            "@deprecated void old() {} void use(void Function() old) { old(); }",
            "@deprecated void old() { old(); } @deprecated void use() { old(); }",
        ] {
            assert!(analyze(source, fixture_path()).is_empty(), "{source}");
        }
    }
}
