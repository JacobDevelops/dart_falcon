//! Reports uses of deprecated declarations from their own package.
//!
//! Canonical project identities and lexical scopes prove code and documentation
//! references; unknown receivers and ambiguous imports remain silent. Deprecated
//! declarations and libraries form a usage frontier, so their bodies, documentation,
//! and metadata are exempt, including part files of deprecated libraries. Library
//! directive messages use canonical package URIs, and local functions stay within
//! their lexical block, switch case, or unbraced statement body. Deprecation messages
//! are trimmed and punctuated like the analyzer's messages. Membership is cached per source.
//! Cascade diagnostics consume shared semantic traversal callbacks, including
//! receiver inference and indexed-assignment classification.
//! Extension owners honor inferred arguments and bounds; `super` owners follow
//! concrete mixin constraints before interface contracts. Optional parameter
//! deprecations are checked even when the enclosing member is not deprecated.
//! Explicit construction resolves dotted constructor selectors separately from
//! import prefixes; generic class qualifiers retain their constructor metadata.
//! Constructor aliases resolve to the underlying declaration before constructor
//! and deprecated-parameter metadata is inspected. Compound assignments and
//! increments inspect both accessors, while simple assignments inspect only setters.
//! Local metadata follows binding declaration identities, including loop variables;
//! local function metadata also applies to references before its lexical declaration.
//! Enum constants do not name deprecated constructors; implicit extension operators
//! use the same applicable-extension lookup as ordinary members.
//! Member and parameter selectors borrow names and keep declaration kind, access,
//! and static dispatch distinct; retained metadata owns names only during indexing.
//! Grouped references are visited, but grouped callees do not expose declaration
//! parameter metadata as direct invocations do.
//! Null-shortened receiver chains recover reached declaration types until parentheses,
//! preserving declared nullable members and applicable nullable extensions.
//! Named constructor uses cover their full qualifier; implicit super uses cover
//! the complete constructor declaration that makes the implicit call.
//!
//! Adopted from the official `deprecated_member_use_from_same_package` lint.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, MemberSelector,
    MemberSelectorKind as SelectorKind, QualifiedName, ResolvedType, Rule, RulePreparation,
    SemanticModel, SignatureIndex, SourceSelector, TypeParameterScope, evaluate_constant,
    resolve_new_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::TokenKind;
use falcon_syntax::cst::visit::{
    Visitor, walk_annotation, walk_class_member, walk_dart_type, walk_expr, walk_formal_param,
    walk_function_decl, walk_method_decl, walk_stmt, walk_top_level_decl,
};
use falcon_syntax::cst::*;

use crate::lint::doc_comments::{DocElement, DocLookup, DocResolver, DocScopes, references};
use falcon_analyze::semantic_scope::{CascadeAssignment, SemanticRuleVisitor, SemanticState};

const ID: &str = "deprecated-member-use-from-same-package";

#[derive(Default)]
pub struct DeprecatedMemberUseFromSamePackage;

impl Rule for DeprecatedMemberUseFromSamePackage {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(package) = crate::cross_file::enclosing_package(ctx.file_path) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        if ctx
            .identities
            .and_then(|identities| {
                let (source, annotations) = identities.library_source_annotations(ctx.file_path)?;
                deprecation(
                    annotations.iter().map(|node| node.cast::<Annotation>()),
                    SemanticModel::new(source, &annotations.first()?.file, identities, ctx.types),
                    ctx.signatures,
                )
            })
            .is_some()
            || program.library_directive().is_some_and(|library| {
                deprecation(library.annotations(), state.model, ctx.signatures).is_some()
            })
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mut frontier = Frontier {
            ctx,
            model: state.model,
            signatures: ctx.signatures,
            identities: ctx.identities,
            owner: None,
            superclass: None,
            declaration: None,
            metadata_scope: false,
            in_metadata: false,
            excluded: Vec::new(),
            metadata: Vec::new(),
            writes: HashSet::new(),
            reads: HashSet::new(),
            increments: HashSet::new(),
            parameters: TypeParameterScope::default(),
            types: StoredTargets::default(),
            extension_owners: Vec::new(),
            blocks: Vec::new(),
            locals: Vec::new(),
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
            package_root: package.lib_root.clone(),
            same_package: HashMap::new(),
            excluded: frontier.excluded,
            writes: frontier.writes,
            reads: frontier.reads,
            increments: frontier.increments,
            diagnostics: Vec::new(),
            reported: HashSet::new(),
            locals: frontier.locals,
            extension_owners: frontier.extension_owners,
            cascade_targets: HashSet::new(),
        };
        for target in &frontier.types.0 {
            uses.report(&target.borrow());
        }
        for (uri, combinators, directive_span) in program
            .imports()
            .map(|node| (node.uri(), node.combinators(), node.span()))
            .chain(
                program
                    .exports()
                    .map(|node| (node.uri(), node.combinators(), node.span())),
            )
        {
            let Some(crate::cross_file::ResolvedRef::Path(path)) =
                crate::cross_file::resolve_directive_uri(
                    ctx.file_path,
                    &uri.value(),
                    Some(&package),
                )
            else {
                continue;
            };
            let Some(identities) = ctx.identities else {
                continue;
            };
            let Some(file) = identities.file(&path) else {
                continue;
            };
            let library = identities.library(file);
            if let Some((source, annotations)) = identities.library_source_annotations(&path)
                && let Some(node) = annotations.first()
                && let Some(message) = deprecation(
                    annotations.iter().map(|node| node.cast::<Annotation>()),
                    SemanticModel::new(&path, &node.file, identities, ctx.types),
                    ctx.signatures,
                )
            {
                let name = identities
                    .library_uri(library)
                    .map(str::to_owned)
                    .unwrap_or_else(|| file_uri(source));
                uses.emit_message(source, &name, directive_span, &message);
            }
            for combinator in combinators {
                if let ImportCombinator::Show(combinator) = combinator {
                    for name in combinator.names() {
                        if let Some(owner) = identities.exported_any(library, name.name()) {
                            uses.report(&Target {
                                owner,
                                selector: MemberSelector::DECLARATION.into(),
                                name: name.name().into(),
                                span: name.span(),
                            });
                        }
                    }
                }
            }
        }
        let sdk_loaded = ctx
            .identities
            .is_some_and(|identities| identities.has_sdk_sources());
        let scopes = DocScopes::new(program, &state.model, ctx.source);
        let resolver = DocResolver::new(
            program,
            &state.model,
            &state.signatures,
            &scopes,
            sdk_loaded,
        );
        for block in ctx.tree().doc_blocks(ctx.source) {
            let Some(scope) = scopes.scope_for(&block) else {
                continue;
            };
            if uses
                .excluded
                .iter()
                .any(|span| span.start <= scope.span.start && scope.span.start < span.end)
            {
                continue;
            }
            let (links, _) = references(&block);
            for link in links {
                let Some(parsed) = link.parse() else {
                    continue;
                };
                let lookups = resolver.resolve(&parsed, scope);
                for (name, lookup) in parsed.names.iter().zip(lookups) {
                    let span = Span::new(name.offset, name.offset + name.text.len());
                    let target = match lookup {
                        DocLookup::Found(DocElement::Declaration {
                            identity: Some(owner),
                            ..
                        }) => Target {
                            name: name.text.into(),
                            owner,
                            selector: MemberSelector::DECLARATION.into(),
                            span,
                        },
                        DocLookup::Found(DocElement::Member { owner, selector }) => {
                            if selector.member.kind == SelectorKind::Constructor {
                                constructor_target(owner, selector.member.name, span)
                            } else {
                                Target {
                                    owner,
                                    selector,
                                    name: name.text.into(),
                                    span,
                                }
                            }
                        }
                        _ => continue,
                    };
                    // The documented declaration was checked against the
                    // deprecated frontier above.
                    uses.emit(&target);
                }
            }
        }
        RulePreparation::Semantic(Box::new(uses))
    }
}

/// Only resolved dart:core annotations count, not user-defined lookalikes.
fn deprecation<'a>(
    annotations: impl IntoIterator<Item = Annotation<'a>>,
    model: SemanticModel<'_>,
    signatures: Option<&SignatureIndex>,
) -> Option<String> {
    for annotation in annotations {
        if annotation.constructor_name().is_some() {
            continue;
        }
        let Some(name) = QualifiedName::from_identifiers(annotation.name()) else {
            continue;
        };
        if annotation.args().is_none()
            && model
                .resolve_sdk_member(name)
                .is_some_and(|id| id.is_sdk("dart:core", "deprecated"))
        {
            return Some(String::new());
        }
        if annotation.args().is_some()
            && model
                .resolve_name(name)
                .is_some_and(|id| id.is_sdk("dart:core", "Deprecated"))
        {
            // Dynamic/unevaluated metadata cannot prove a deprecated constant.
            let expression = annotation.args()?.positional().next()?;
            let message = if let Expr::StringLit(message) = expression.unparenthesized() {
                message.value()
            } else {
                let Some(ConstantValue::String(message)) = signatures.and_then(|signatures| {
                    evaluate_constant(expression, None, &HashMap::new(), &model, signatures)
                }) else {
                    continue;
                };
                Cow::Owned(message)
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
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: Option<&'a SignatureIndex>,
    identities: Option<&'a falcon_analyze::IdentityIndex>,
    owner: Option<DeclarationIdentity>,
    superclass: Option<DeclarationIdentity>,
    declaration: Option<Span>,
    metadata_scope: bool,
    in_metadata: bool,
    excluded: Vec<Span>,
    metadata: Vec<(Span, Span)>,
    writes: HashSet<Span>,
    reads: HashSet<Span>,
    increments: HashSet<Span>,
    parameters: TypeParameterScope,
    types: StoredTargets,
    extension_owners: Vec<ExtensionOwner>,
    blocks: Vec<Span>,
    locals: Vec<LocalDeprecation>,
}

impl Frontier<'_> {
    fn arguments(&mut self, target: &Target, args: ArgList<'_>) {
        self.types
            .extend(parameter_targets(target, args, self.identities));
    }
}

impl<'a> Visitor<'a> for Frontier<'a> {
    fn visit_block(&mut self, node: Block<'a>) {
        self.blocks.push(node.span());
        falcon_syntax::cst::visit::walk_block(self, node);
        self.blocks.pop();
    }
    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        self.blocks.push(node.span());
        falcon_syntax::cst::visit::walk_switch_case(self, node);
        self.blocks.pop();
    }
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        let previous = self.declaration.replace(node.span());
        let previous_scope = std::mem::replace(
            &mut self.metadata_scope,
            matches!(
                node,
                TopLevelDecl::Function(_) | TopLevelDecl::Variable(_) | TopLevelDecl::TypeAlias(_)
            ),
        );
        let parameters = match node {
            TopLevelDecl::Class(n) => Some(n.type_params()),
            TopLevelDecl::Mixin(n) => Some(n.type_params()),
            TopLevelDecl::MixinClass(n) => Some(n.type_params()),
            TopLevelDecl::Enum(n) => Some(n.type_params()),
            TopLevelDecl::Extension(n) => Some(n.type_params()),
            TopLevelDecl::ExtensionType(n) => Some(n.type_params()),
            TopLevelDecl::ClassTypeAlias(n) => Some(n.type_params()),
            TopLevelDecl::TypeAlias(n) => Some(n.type_params()),
            _ => None,
        }
        .into_iter()
        .flatten();
        self.parameters.push(parameters, &self.model);
        if let TopLevelDecl::Enum(enumeration) = node {
            for variant in enumeration.variants() {
                for annotation in variant.annotations() {
                    self.metadata.push((variant.span(), annotation.span()));
                }
            }
        }
        let (owner, superclass) = match node {
            TopLevelDecl::Class(n) => (
                self.model.resolve_name(n.name().name()),
                n.extends().filter(|_| n.with_clause().is_empty()),
            ),
            TopLevelDecl::Mixin(n) => (self.model.resolve_name(n.name().name()), None),
            TopLevelDecl::MixinClass(n) => (self.model.resolve_name(n.name().name()), None),
            TopLevelDecl::Enum(n) => (self.model.resolve_name(n.name().name()), None),
            TopLevelDecl::Extension(n) => {
                let owner = n
                    .name()
                    .and_then(|name| self.model.resolve_extension(name.name()));
                let mut members = Vec::new();
                for member in n.members() {
                    match member {
                        ClassMember::Field(field) => {
                            members.extend(field.declarators().map(|variable| {
                                (variable.name().name().to_owned(), field.is_static())
                            }))
                        }
                        ClassMember::Method(method) => {
                            members.push((method.name().name().to_owned(), method.is_static()))
                        }
                        ClassMember::Getter(getter) => {
                            members.push((getter.name().name().to_owned(), getter.is_static()))
                        }
                        ClassMember::Setter(setter) => {
                            members.push((setter.name().name().to_owned(), setter.is_static()))
                        }
                        _ => {}
                    }
                }
                self.extension_owners.push(ExtensionOwner {
                    span: n.span(),
                    identity: owner.clone(),
                    members,
                });
                (owner, None)
            }
            TopLevelDecl::ExtensionType(n) => (self.model.resolve_name(n.name().name()), None),
            _ => (None, None),
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
        walk_top_level_decl(self, node);
        self.declaration = previous;
        self.parameters.pop();
        self.owner = old_owner;
        self.superclass = old_superclass;
        self.metadata_scope = previous_scope;
    }
    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        self.parameters.push(node.type_params(), &self.model);
        walk_function_decl(self, node);
        self.parameters.pop();
    }
    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        self.parameters.push(node.type_params(), &self.model);
        walk_method_decl(self, node);
        self.parameters.pop();
    }
    fn visit_constructor(&mut self, node: ConstructorRef<'a>) {
        let mut explicit = false;
        for initializer in node.initializers.clone() {
            if let ConstructorInitializer::FieldInit(initializer) = initializer
                && let Some(owner) = &self.owner
            {
                let field = initializer.field();
                self.types.push(Target {
                    owner: owner.clone(),
                    selector: MemberSelector::new(SelectorKind::Field, false, field.name()).into(),
                    name: field.name().into(),
                    span: field.span(),
                });
            }
        }
        for initializer in node.initializers {
            let (owner, name, args, span) = match initializer {
                ConstructorInitializer::SuperCall(call) => (
                    self.superclass.as_ref(),
                    call.call_name(),
                    call.args(),
                    call.span(),
                ),
                ConstructorInitializer::ThisCall(call) => (
                    self.owner.as_ref(),
                    call.call_name(),
                    call.args(),
                    call.span(),
                ),
                _ => continue,
            };
            explicit = true;
            if let Some(owner) = owner {
                let target = constructor_target(
                    owner.clone(),
                    name.map_or("new", |n| n.name()),
                    name.map_or(span, |n| n.span()),
                );
                self.arguments(&target, args);
                self.types.push(target);
            }
        }
        if !explicit
            && !node.is_factory
            && !node.is_primary
            && let Some(superclass) = &self.superclass
        {
            let span = crate::lint::declarations::documented_node_span(
                node.span,
                node.annotations,
                self.ctx,
            );
            self.types
                .push(constructor_target(superclass.clone(), "new", span));
        }
        if let Some(redirect) = &node.redirect
            && let ResolvedType::Interface { identity, .. } = self
                .model
                .resolve_type_in(redirect.type_(), &self.parameters)
        {
            self.types.push(constructor_target(
                identity,
                redirect
                    .constructor_name()
                    .as_ref()
                    .map_or("new", |n| n.name()),
                redirect
                    .constructor_name()
                    .as_ref()
                    .map_or(redirect.span(), |n| n.span()),
            ));
        }
        // The parser may retain an unprefixed `.named` suffix in the type.
        if let Some(redirect) = &node.redirect
            && redirect.constructor_name().is_none()
            && let DartType::Named(named) = redirect.type_()
            && let (Some(class_name), Some(constructor), None) = (
                named.segments().next(),
                named.segments().nth(1),
                named.segments().nth(2),
            )
            && let Some(owner) = self.model.resolve_name(class_name.name())
            && self
                .signatures
                .is_some_and(|index| index.constructor(&owner, constructor.name()).is_some())
        {
            self.types.push(constructor_target(
                owner,
                constructor.name(),
                constructor.span(),
            ));
        }
    }
    fn visit_dart_type(&mut self, node: DartType<'a>) {
        if let DartType::Function(function) = node {
            self.parameters.push(function.type_params(), &self.model);
            walk_dart_type(self, node);
            self.parameters.pop();
            return;
        }
        if let DartType::Named(named) = node
            && let Some(name) = named.segments().last()
            && !(named.segments().count() == 1 && self.parameters.contains(name.name()))
            && let Some(identity) = QualifiedName::from_identifiers(named.segments())
                .and_then(|name| self.model.resolve_name(name))
        {
            self.types.push_declaration(identity, name.span());
        }
        walk_dart_type(self, node);
    }
    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        let previous = self.declaration.replace(node.span());
        // Field annotations are not traversed by the generic member walker.
        if let ClassMember::Field(field) = node {
            for annotation in field.annotations() {
                self.visit_annotation(annotation);
            }
        }
        walk_class_member(self, node);
        self.declaration = previous;
    }
    fn visit_formal_param(&mut self, node: FormalParam<'a>) {
        let previous = self.declaration.replace(node.span());
        walk_formal_param(self, node);
        self.declaration = previous;
    }
    fn visit_annotation(&mut self, node: Annotation<'a>) {
        let variant = self
            .metadata
            .iter()
            .find(|(_, annotation)| *annotation == node.span())
            .map(|(declaration, _)| *declaration);
        let declaration = variant.or(self.declaration);
        if deprecation([node], self.model, self.signatures).is_some()
            && let Some(span) = declaration
        {
            self.excluded.push(span);
        }
        if variant.is_none()
            && let Some(declaration) = declaration
        {
            self.metadata.push((declaration, node.span()));
        }
        if let Some(name) = QualifiedName::from_identifiers(node.name())
            && let Some(owner) = self
                .model
                .resolve_name(name)
                .or_else(|| self.model.resolve_value(name))
            && let Some(identifier) = node.name().last()
        {
            self.types
                .push_declaration(owner.clone(), identifier.span());
            if node.args().is_some() {
                let constructor = node.constructor_name();
                let target = constructor_target(
                    owner,
                    constructor.map_or("new", |n| n.name()),
                    constructor.map_or(identifier.span(), |n| n.span()),
                );
                // `@A()` names the class; only `@A.named()` names a constructor.
                if constructor.is_some() {
                    self.types.push(target);
                }
            }
        }
        let previous = std::mem::replace(&mut self.in_metadata, self.metadata_scope);
        walk_annotation(self, node);
        self.in_metadata = previous;
    }
    fn visit_stmt(&mut self, node: Stmt<'a>) {
        let previous = std::mem::replace(&mut self.metadata_scope, false);
        let declaration = self.declaration;
        if let Stmt::PatternDecl(pattern) = node {
            self.declaration = Some(pattern.span());
        }
        if let Stmt::LocalFunc(function) = node {
            self.declaration = Some(function.span());
            if let Some(scope) = self.blocks.iter().rev().find(|scope| {
                scope.start <= function.span().start && function.span().end <= scope.end
            }) {
                let message = deprecation(function.annotations(), self.model, self.signatures);
                let mut parameters = Vec::new();
                for (position, parameter) in function.params().positional().enumerate() {
                    if parameter.kind() == ParamKind::OptionalPositional
                        && let Some(message) =
                            deprecation(parameter.annotations(), self.model, self.signatures)
                    {
                        parameters.push(LocalParameter {
                            position: Some(position),
                            name: parameter.name().name().to_owned(),
                            message,
                        });
                    }
                }
                for parameter in function.params().named() {
                    if parameter.kind() == (ParamKind::Named { required: false })
                        && let Some(message) =
                            deprecation(parameter.annotations(), self.model, self.signatures)
                    {
                        parameters.push(LocalParameter {
                            position: None,
                            name: parameter.name().name().to_owned(),
                            message,
                        });
                    }
                }
                self.locals.push(LocalDeprecation {
                    name: function.name().name().to_owned(),
                    scope: *scope,
                    binding: function.name().span(),
                    function: true,
                    message,
                    parameters,
                });
            }
        }
        let scopes = self.blocks.len();
        match node {
            Stmt::If(statement) => {
                self.blocks.push(statement.then_branch().span());
                if let Some(branch) = &statement.else_branch() {
                    self.blocks.push(branch.span());
                }
            }
            Stmt::For(statement) => self.blocks.push(statement.body().span()),
            Stmt::While(statement) => self.blocks.push(statement.body().span()),
            Stmt::DoWhile(statement) => self.blocks.push(statement.body().span()),
            _ => {}
        }
        walk_stmt(self, node);
        self.blocks.truncate(scopes);
        self.declaration = declaration;
        self.metadata_scope = previous;
    }
    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        let previous = self.declaration.replace(node.full_range());
        if let Some(scope) = self.blocks.last()
            && let Some(message) = deprecation(node.annotations(), self.model, self.signatures)
        {
            for variable in node.declarators() {
                self.locals.push(LocalDeprecation {
                    name: variable.name().name().to_owned(),
                    scope: *scope,
                    binding: variable.name().span(),
                    function: false,
                    message: Some(message.clone()),
                    parameters: Vec::new(),
                });
            }
        }
        falcon_syntax::cst::visit::walk_local_var(self, node);
        self.declaration = previous;
    }
    fn visit_for_header(&mut self, node: ForHeader<'a>) {
        let previous = self.declaration;
        if let Some(ForInit::ForIn(initializer)) = node.init()
            && initializer.declares_variable()
        {
            let name = initializer.name();
            self.declaration = Some(name.span());
            if let Some(scope) = self.blocks.last()
                && let Some(message) =
                    deprecation(initializer.annotations(), self.model, self.signatures)
            {
                self.locals.push(LocalDeprecation {
                    name: name.name().to_owned(),
                    scope: *scope,
                    binding: name.span(),
                    function: false,
                    message: Some(message),
                    parameters: Vec::new(),
                });
            }
        }
        falcon_syntax::cst::visit::walk_for_header(self, node);
        self.declaration = previous;
    }
    fn visit_expr(&mut self, node: Expr<'a>) {
        if self.in_metadata
            && let Expr::Ident(identifier) = node
            && self.parameters.lookup(identifier.name()).is_none()
            && let Some(owner) = self
                .model
                .resolve_value(identifier.name())
                .or_else(|| self.model.resolve_name(identifier.name()))
        {
            self.types.push(Target {
                owner,
                selector: MemberSelector::DECLARATION.into(),
                name: identifier.name().into(),
                span: identifier.span(),
            });
        }
        match node {
            Expr::Assign(assignment) => {
                self.writes.insert(assignment.target().span());
                if assignment.op() != AssignOp::Eq {
                    self.reads.insert(assignment.target().span());
                }
            }
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                self.writes.insert(unary.operand().span());
                self.reads.insert(unary.operand().span());
                self.increments.insert(unary.operand().span());
            }
            Expr::PostfixIncDec(postfix) => {
                self.writes.insert(postfix.operand().span());
                self.reads.insert(postfix.operand().span());
                self.increments.insert(postfix.operand().span());
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
    writes: HashSet<Span>,
    reads: HashSet<Span>,
    increments: HashSet<Span>,
    diagnostics: Vec<Diagnostic>,
    reported: HashSet<(usize, usize, String)>,
    locals: Vec<LocalDeprecation>,
    extension_owners: Vec<ExtensionOwner>,
    cascade_targets: HashSet<Span>,
}

struct ExtensionOwner {
    span: Span,
    identity: Option<DeclarationIdentity>,
    members: Vec<(String, bool)>,
}

struct Target<'a> {
    owner: DeclarationIdentity,
    selector: SourceSelector<'a>,
    name: Cow<'a, str>,
    span: Span,
}

#[derive(Default)]
struct StoredTargets(Vec<StoredTarget>);

struct StoredTarget {
    owner: DeclarationIdentity,
    kind: SelectorKind,
    is_static: bool,
    member: String,
    parameter: Option<String>,
    name: Option<String>,
    span: Span,
}

impl StoredTargets {
    fn push_declaration(&mut self, owner: DeclarationIdentity, span: Span) {
        self.0.push(StoredTarget {
            owner,
            kind: SelectorKind::Declaration,
            is_static: false,
            member: String::new(),
            parameter: None,
            name: None,
            span,
        });
    }

    fn push(&mut self, target: Target<'_>) {
        let default_name = target.selector.parameter.unwrap_or_else(|| {
            if target.selector.member.name.is_empty() {
                target.owner.name()
            } else {
                target.selector.member.name
            }
        });
        let name = (target.name.as_ref() != default_name).then(|| target.name.into_owned());
        self.0.push(StoredTarget {
            owner: target.owner,
            kind: target.selector.member.kind,
            is_static: target.selector.member.is_static,
            member: target.selector.member.name.to_owned(),
            parameter: target.selector.parameter.map(str::to_owned),
            name,
            span: target.span,
        });
    }

    fn extend<'a>(&mut self, targets: impl Iterator<Item = Target<'a>>) {
        for target in targets {
            self.push(target);
        }
    }
}

impl StoredTarget {
    fn borrow(&self) -> Target<'_> {
        Target {
            owner: self.owner.clone(),
            selector: SourceSelector {
                member: MemberSelector::new(self.kind, self.is_static, &self.member),
                parameter: self.parameter.as_deref(),
            },
            name: self
                .name
                .as_deref()
                .unwrap_or_else(|| {
                    self.parameter.as_deref().unwrap_or_else(|| {
                        if self.member.is_empty() {
                            self.owner.name()
                        } else {
                            &self.member
                        }
                    })
                })
                .into(),
            span: self.span,
        }
    }
}

struct LocalDeprecation {
    name: String,
    scope: Span,
    binding: Span,
    function: bool,
    message: Option<String>,
    parameters: Vec<LocalParameter>,
}

struct LocalParameter {
    position: Option<usize>,
    name: String,
    message: String,
}

fn constructor_target<'a>(owner: DeclarationIdentity, name: &'a str, span: Span) -> Target<'a> {
    Target {
        name: "".into(),
        owner,
        selector: MemberSelector::new(SelectorKind::Constructor, false, name).into(),
        span,
    }
}

fn parameter_targets<'a>(
    target: &'a Target<'a>,
    args: ArgList<'a>,
    identities: Option<&'a falcon_analyze::IdentityIndex>,
) -> impl Iterator<Item = Target<'a>> + 'a {
    let positional = args
        .positional()
        .enumerate()
        .filter_map(move |(position, argument)| {
            let name = identities?.deprecated_positional_parameter(
                &target.owner,
                target.selector.member,
                position,
            )?;
            Some(Target {
                owner: target.owner.clone(),
                selector: SourceSelector {
                    member: target.selector.member,
                    parameter: Some(name),
                },
                name: name.into(),
                span: argument.span(),
            })
        });
    positional.chain(args.named().map(move |argument| Target {
        owner: target.owner.clone(),
        selector: SourceSelector {
            member: target.selector.member,
            parameter: Some(argument.name().name()),
        },
        name: argument.name().name().into(),
        span: argument.name().span(),
    }))
}

fn file_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    let mut uri = String::with_capacity(7 + path.len());
    uri.push_str("file://");
    falcon_analyze::environment::percent_encode(&path, &mut uri);
    uri
}

fn push_deprecation(
    ctx: &AnalyzeContext<'_>,
    excluded: &[Span],
    reported: &mut HashSet<(usize, usize, String)>,
    diagnostics: &mut Vec<Diagnostic>,
    name: &str,
    span: Span,
    detail: &str,
) {
    if excluded
        .iter()
        .any(|excluded| excluded.start <= span.start && span.end <= excluded.end)
    {
        return;
    }
    let base = format!("'{name}' is deprecated and shouldn't be used.");
    let message = if detail.is_empty() {
        base
    } else {
        format!("{base} {detail}")
    };
    if reported.insert((span.start, span.end, message.clone())) {
        diagnostics.push(ctx.diagnostic(ID, message, span));
    }
}

impl<'ctx> Uses<'ctx> {
    fn local(&self, name: &str, span: Span, state: &SemanticState<'_>) -> Option<usize> {
        self.locals
            .iter()
            .enumerate()
            .filter(|(_, local)| {
                local.name == name
                    && local.scope.start <= span.start
                    && span.end <= local.scope.end
                    && (state.environment.binding_declaration(name) == Some(local.binding)
                        || (local.function
                            && span.start < local.binding.start
                            && state
                                .environment
                                .binding_declaration(name)
                                .is_none_or(|binding| {
                                    binding.start < local.scope.start
                                        || local.scope.end < binding.end
                                })))
            })
            .min_by_key(|(_, local)| local.scope.end - local.scope.start)
            .map(|(index, _)| index)
    }
    fn report(&mut self, target: &Target) {
        if self
            .excluded
            .iter()
            .any(|span| span.start <= target.span.start && target.span.end <= span.end)
        {
            return;
        }
        self.emit(target);
    }

    /// Reports a deprecated target in this package outside deprecated declarations.
    fn emit(&mut self, target: &Target) {
        let Some(identities) = self.ctx.identities else {
            return;
        };
        let Some((path, annotations)) = identities
            .source_annotations(&target.owner, target.selector)
            .or_else(|| {
                let member = target.selector.member;
                if target.selector.parameter.is_none()
                    && member.name.is_empty()
                    && matches!(member.kind, SelectorKind::Declaration | SelectorKind::Write)
                {
                    identities.source_annotations(
                        &target.owner,
                        MemberSelector::new(SelectorKind::Variable, false, "").into(),
                    )
                } else {
                    None
                }
            })
            .or_else(|| {
                let member = target.selector.member;
                if target.selector.parameter.is_some()
                    || !matches!(member.kind, SelectorKind::Read | SelectorKind::Write)
                {
                    return None;
                }
                identities.source_annotations(
                    &target.owner,
                    MemberSelector::new(SelectorKind::Field, member.is_static, member.name).into(),
                )
            })
        else {
            return;
        };
        let Some(annotation) = annotations.first() else {
            return;
        };
        let model = SemanticModel::new(path, &annotation.file, identities, self.ctx.types);
        let Some(message) = deprecation(
            annotations.iter().map(|node| node.cast::<Annotation>()),
            model,
            self.ctx.signatures,
        ) else {
            return;
        };
        let name = if target.selector.parameter.is_none()
            && target.selector.member.kind == SelectorKind::Constructor
            && target.name.is_empty()
        {
            if target.selector.member.name == "new" {
                target.owner.name().into()
            } else {
                Cow::Owned(format!(
                    "{}.{}",
                    target.owner.name(),
                    target.selector.member.name
                ))
            }
        } else {
            Cow::Borrowed(target.name.as_ref())
        };
        self.emit_message(path, &name, target.span, &message);
    }

    fn emit_message(&mut self, path: &std::path::Path, name: &str, span: Span, message: &str) {
        if self
            .excluded
            .iter()
            .any(|excluded| excluded.start <= span.start && span.end <= excluded.end)
        {
            return;
        }
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
        push_deprecation(
            self.ctx,
            &self.excluded,
            &mut self.reported,
            &mut self.diagnostics,
            name,
            span,
            message,
        );
    }

    fn member<'node>(
        &self,
        receiver: &ResolvedType,
        name: &'node str,
        span: Span,
        state: &SemanticState<'_>,
        write: bool,
    ) -> Option<Target<'node>>
    where
        'ctx: 'node,
    {
        if name == "call"
            && (matches!(receiver, ResolvedType::Function { .. })
                || matches!(receiver, ResolvedType::Interface { identity, .. }
                    if identity.is_sdk("dart:core", "Function")))
        {
            return None;
        }
        let owner = if write {
            state
                .signatures
                .resolved_writable_member_owner(receiver, name, &state.model)
        } else {
            state
                .signatures
                .resolved_readable_member_owner(receiver, name, &state.model)
        }
        .or_else(|| {
            state
                .signatures
                .resolved_extension_access_owner(receiver, name, write, &state.model)
        })?;
        let selector = MemberSelector::new(
            if write {
                SelectorKind::Write
            } else {
                SelectorKind::Read
            },
            false,
            name,
        )
        .into();
        Some(Target {
            owner,
            selector,
            name: name.into(),
            span,
        })
    }
    fn cascade_index_span(&self, start: usize, index: Expr<'_>) -> Span {
        let end = self
            .ctx
            .tree()
            .tokens_in(
                self.ctx.source,
                Span::new(index.span().end, self.ctx.source.len()),
            )
            .find(|token| token.kind() == TokenKind::RBracket)
            .map_or(index.span().end, |token| token.span().end);
        Span { start, end }
    }

    fn extension(&self, name: QualifiedName<'_>) -> Option<DeclarationIdentity> {
        SemanticModel::new(
            self.ctx.file_path,
            self.ctx.parsed,
            self.ctx.identities?,
            self.ctx.types,
        )
        .resolve_extension(name)
    }

    fn extension_member<'node>(
        &self,
        owner: &DeclarationIdentity,
        name: &'node str,
        span: Span,
        write: bool,
    ) -> Option<Target<'node>>
    where
        'ctx: 'node,
    {
        let selector = self
            .ctx
            .identities?
            .static_member_selector(owner, name, write)?;
        Some(Target {
            owner: owner.clone(),
            selector: selector.into(),
            name: name.into(),
            span,
        })
    }

    fn extension_reference(
        &self,
        object: Expr<'_>,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        let mut callee = object;
        while let Expr::GenericInstantiation(instantiation) = callee {
            callee = instantiation.target();
        }
        // A value or type in the current namespace shadows imported extensions.
        match callee {
            Expr::Ident(name)
                if !state.environment.is_bound(name.name())
                    && state.model.resolve_value(name.name()).is_none()
                    && state.model.resolve_name(name.name()).is_none() =>
            {
                self.extension(name.name().into())
            }
            Expr::Field(access) => {
                let field = access.field();
                let Expr::Ident(prefix) = access.object().unparenthesized() else {
                    return None;
                };
                if state.environment.is_bound(prefix.name()) {
                    return None;
                }
                let name = QualifiedName::prefixed(prefix.name(), field.name());
                if state.model.resolve_value(name).is_some()
                    || state.model.resolve_name(name).is_some()
                {
                    return None;
                }
                self.extension(name)
            }
            _ => None,
        }
    }

    fn extension_override(
        &self,
        object: Expr<'_>,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        let Expr::Call(call) = object.unparenthesized() else {
            return None;
        };
        self.extension_reference(call.callee(), state)
    }

    /// Resolve a named constructor reference, retaining its complete qualifier.
    fn constructor_reference<'node>(
        &self,
        node: Expr<'node>,
        state: &SemanticState<'_>,
    ) -> Option<Target<'node>> {
        let Expr::Field(access) = node.unparenthesized() else {
            return None;
        };
        let field = access.field();
        let mut qualifier = access.object().unparenthesized();
        while let Expr::GenericInstantiation(instantiation) = qualifier {
            qualifier = instantiation.target().unparenthesized();
        }
        let qualified = match qualifier {
            Expr::Ident(name) if !state.environment.is_bound(name.name()) => {
                QualifiedName::from(name.name())
            }
            Expr::Field(access) => match access.object().unparenthesized() {
                Expr::Ident(prefix) if !state.environment.is_bound(prefix.name()) => {
                    QualifiedName::prefixed(prefix.name(), access.field().name())
                }
                _ => return None,
            },
            _ => return None,
        };
        let owner = state.model.unaliased(state.model.resolve_name(qualified)?);
        state.signatures.constructor(&owner, field.name())?;
        Some(constructor_target(owner, field.name(), node.span()))
    }

    fn reference_target<'node>(
        &self,
        node: Expr<'node>,
        state: &SemanticState<'_>,
        access: Option<bool>,
    ) -> Option<Target<'node>>
    where
        'ctx: 'node,
    {
        if let Some(constructor) = self.constructor_reference(node, state) {
            return Some(constructor);
        }
        let mut node = node;
        while let Expr::GenericInstantiation(instantiation) = node.unparenthesized() {
            node = instantiation.target();
        }
        let write = access.unwrap_or_else(|| self.writes.contains(&node.span()));
        match node.unparenthesized() {
            Expr::Ident(id)
                if !state.environment.is_bound(id.name())
                    || state.environment.is_bound_in_root_scope(id.name()) =>
            {
                let extension = self.extension_owners.iter().find(|owner| {
                    owner.span.start <= id.span().start && id.span().end <= owner.span.end
                });
                if let Some(owner) = extension
                    && let Some((_, is_static)) =
                        owner.members.iter().find(|(name, _)| name == id.name())
                {
                    let owner = owner.identity.clone()?;
                    let selector = if *is_static {
                        self.ctx
                            .identities?
                            .static_member_selector(&owner, id.name(), write)?
                            .into()
                    } else {
                        MemberSelector::new(
                            if write {
                                SelectorKind::Write
                            } else {
                                SelectorKind::Read
                            },
                            false,
                            id.name(),
                        )
                        .into()
                    };
                    return Some(Target {
                        owner,
                        selector,
                        name: id.name().into(),
                        span: id.span(),
                    });
                }
                if let Some(receiver) = &state.enclosing_type
                    && let Some(target) = self.member(receiver, id.name(), id.span(), state, write)
                {
                    return Some(target);
                }
                if extension.is_none()
                    && let Some(ResolvedType::Interface { identity, .. }) = &state.enclosing_type
                    && state
                        .signatures
                        .declared_member_facts(identity, id.name())
                        .iter()
                        .any(|fact| fact.is_static)
                {
                    return Some(Target {
                        owner: identity.clone(),
                        selector: self
                            .ctx
                            .identities?
                            .static_member_selector(identity, id.name(), write)?
                            .into(),
                        name: id.name().into(),
                        span: id.span(),
                    });
                }
                let value = state.model.resolve_value(id.name());
                let owner = value
                    .clone()
                    .or_else(|| state.model.resolve_name(id.name()))
                    .or_else(|| self.extension(id.name().into()))?;
                Some(Target {
                    owner,
                    selector: MemberSelector::new(
                        if write && value.is_some() {
                            SelectorKind::Write
                        } else {
                            SelectorKind::Declaration
                        },
                        false,
                        "",
                    )
                    .into(),
                    name: id.name().into(),
                    span: id.span(),
                })
            }
            Expr::Field(access) => {
                let object = access.object();
                let field = access.field();
                let span = if self.increments.contains(&node.span()) {
                    node.span()
                } else {
                    field.span()
                };
                if matches!(object.unparenthesized(), Expr::Super(_)) {
                    let owner = state.signatures.resolved_super_member_owner(
                        state.enclosing_type.as_ref()?,
                        field.name(),
                        write,
                        &state.model,
                    )?;
                    return Some(Target {
                        owner,
                        selector: MemberSelector::new(
                            if write {
                                SelectorKind::Write
                            } else {
                                SelectorKind::Read
                            },
                            false,
                            field.name(),
                        )
                        .into(),
                        name: field.name().into(),
                        span,
                    });
                }
                if let Some(owner) = self.extension_reference(object, state)
                    && let Some(target) = self.extension_member(&owner, field.name(), span, write)
                {
                    return Some(target);
                }
                if let Some(owner) = self.extension_override(object, state) {
                    return Some(Target {
                        owner,
                        selector: MemberSelector::new(
                            if write {
                                SelectorKind::Write
                            } else {
                                SelectorKind::Read
                            },
                            false,
                            field.name(),
                        )
                        .into(),
                        name: field.name().into(),
                        span,
                    });
                }
                let mut qualifier = object;
                while let Expr::GenericInstantiation(instantiation) = qualifier.unparenthesized() {
                    qualifier = instantiation.target();
                }
                if let Expr::Ident(prefix) = qualifier.unparenthesized()
                    && !state.environment.is_bound(prefix.name())
                {
                    let qualified = QualifiedName::prefixed(prefix.name(), field.name());
                    let value = state.model.resolve_value(qualified);
                    if let Some(owner) = value
                        .clone()
                        .or_else(|| state.model.resolve_name(qualified))
                        .or_else(|| self.extension(qualified))
                    {
                        return Some(Target {
                            owner,
                            selector: MemberSelector::new(
                                if write && value.is_some() {
                                    SelectorKind::Write
                                } else {
                                    SelectorKind::Declaration
                                },
                                false,
                                "",
                            )
                            .into(),
                            name: field.name().into(),
                            span,
                        });
                    }
                    if let Some(owner) = state.model.resolve_name(prefix.name()) {
                        let owner = state.model.unaliased(owner);
                        let selector = self
                            .ctx
                            .identities?
                            .static_member_selector(&owner, field.name(), write)?
                            .into();
                        return Some(Target {
                            owner,
                            selector,
                            name: field.name().into(),
                            span,
                        });
                    }
                }
                let receiver =
                    state.infer_access_receiver(object, field.name(), access.is_null_safe());
                let mut target = self.member(&receiver, field.name(), node.span(), state, write)?;
                target.span = span;
                Some(target)
            }
            _ => None,
        }
    }

    fn arguments(&mut self, target: &Target, args: ArgList<'_>) {
        for parameter in parameter_targets(target, args, self.ctx.identities) {
            self.report(&parameter);
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

impl<'a> SemanticRuleVisitor<'a> for Uses<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'_>,
    ) {
        match operation {
            CascadeOp::Field(_) | CascadeOp::Call(_) => {
                let name = match operation {
                    CascadeOp::Field(field) => field.name(),
                    CascadeOp::Call(call) => call.name(),
                    _ => unreachable!(),
                };
                if let Some(target) = self.member(receiver, name.name(), name.span(), state, false)
                {
                    self.report(&target);
                    if let CascadeOp::Call(call) = operation {
                        self.arguments(&target, call.args());
                    }
                }
            }
            CascadeOp::Invoke(invoke) => {
                let args = invoke.args();
                if !matches!(receiver, ResolvedType::Function { .. })
                    && let Some(target) = self.member(
                        receiver,
                        "call",
                        Span::new(section.start, args.span().end),
                        state,
                        false,
                    )
                {
                    self.report(&target);
                    // Implicit call invocations report the callable member,
                    // not its deprecated parameters, unlike explicit `.call`.
                }
            }
            CascadeOp::Index(index) => {
                if let Some(owner) =
                    state
                        .signatures
                        .resolved_member_owner(receiver, "[]", &state.model)
                {
                    self.report(&Target {
                        owner,
                        selector: MemberSelector::new(SelectorKind::Operator(1), false, "[]")
                            .into(),
                        name: "[]".into(),
                        span: self.cascade_index_span(section.start, index.index()),
                    });
                }
            }
            CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) | CascadeOp::NullAssert(_) => {}
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'_>,
    ) {
        let CascadeAssignment {
            target,
            operator,
            indexed,
            ..
        } = assignment;
        if indexed {
            let span = self.cascade_index_span(section.start, target);
            for (name, arity) in [("[]=", 2), ("[]", 1)] {
                if name == "[]" && operator == AssignOp::Eq {
                    continue;
                }
                if let Some(owner) =
                    state
                        .signatures
                        .resolved_member_owner(receiver, name, &state.model)
                {
                    self.report(&Target {
                        owner,
                        selector: MemberSelector::new(SelectorKind::Operator(arity), false, name)
                            .into(),
                        name: name.into(),
                        span,
                    });
                }
            }
        } else if let Expr::Ident(name) = target.unparenthesized() {
            self.cascade_targets.insert(name.span());
            for write in [true, false] {
                if !write && operator == AssignOp::Eq {
                    continue;
                }
                if let Some(target) = self.member(receiver, name.name(), name.span(), state, write)
                {
                    self.report(&target);
                }
            }
        }
    }
    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        _matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if let Pattern::Object(pattern) = node {
            let receiver = state
                .model
                .resolve_type_in(pattern.type_(), &state.type_parameters);
            for field in pattern.fields() {
                if let Some(target) = self.member(
                    &receiver,
                    field.name().name(),
                    field.name().span(),
                    state,
                    false,
                ) {
                    self.report(&target);
                }
            }
            return;
        }
        let Pattern::Const(pattern) = node else {
            return;
        };
        let ConstPatternValue::Name(names) = pattern.value() else {
            return;
        };
        let Some(name) = names.clone().last() else {
            return;
        };
        let qualifier_len = names.len() - 1;
        let qualifier = names.take(qualifier_len);
        let (owner, selector) = if qualifier_len == 0 {
            let Some(owner) = state.model.resolve_value(name.name()) else {
                return;
            };
            (owner, MemberSelector::DECLARATION)
        } else {
            let Some(qualified) = QualifiedName::from_identifiers(qualifier) else {
                return;
            };
            let Some(owner) = state.model.resolve_name(qualified) else {
                return;
            };
            (
                owner,
                MemberSelector::new(SelectorKind::Read, true, name.name()),
            )
        };
        self.report(&Target {
            owner,
            selector: selector.into(),
            name: name.name().into(),
            span: name.span(),
        });
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        if matches!(node, Expr::Paren(_)) {
            // The shared walker visits the inner expression separately.
            return;
        }
        if self.cascade_targets.contains(&node.span()) {
            return;
        }
        if let Expr::Ident(identifier) = node
            && let Some(index) = self.local(identifier.name(), identifier.span(), state)
        {
            if let Some(message) = &self.locals[index].message {
                push_deprecation(
                    self.ctx,
                    &self.excluded,
                    &mut self.reported,
                    &mut self.diagnostics,
                    identifier.name(),
                    identifier.span(),
                    message,
                );
            }
            return;
        }
        if let Some(target) = self.reference_target(node, state, None) {
            self.report(&target);
        }
        if self.reads.contains(&node.span())
            && let Some(target) = self.reference_target(node, state, Some(false))
        {
            self.report(&target);
        }
        let operator = match node {
            Expr::Binary(binary) => {
                let op = binary.op();
                let left = binary.left();
                let right = binary.right();
                if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq)
                    && (matches!(state.infer(left), ResolvedType::Null)
                        || matches!(state.infer(right), ResolvedType::Null))
                {
                    None
                } else {
                    binary_operator(op).map(|name| (left, name, 1))
                }
            }
            Expr::Assign(assignment) => assignment
                .op()
                .binary()
                .and_then(binary_operator)
                .map(|name| (assignment.target(), name, 1)),
            Expr::Unary(unary) => match unary.op() {
                UnaryOp::Minus => Some((unary.operand(), "-", 0)),
                UnaryOp::Tilde => Some((unary.operand(), "~", 0)),
                UnaryOp::PlusPlus => Some((unary.operand(), "+", 1)),
                UnaryOp::MinusMinus => Some((unary.operand(), "-", 1)),
                UnaryOp::Bang => None,
            },
            Expr::PostfixIncDec(postfix) => Some((
                postfix.operand(),
                if postfix.op() == PostfixIncDec::Increment {
                    "+"
                } else {
                    "-"
                },
                1,
            )),
            Expr::Index(index) => Some((
                index.object(),
                if self.writes.contains(&node.span()) {
                    "[]="
                } else {
                    "[]"
                },
                if self.writes.contains(&node.span()) {
                    2
                } else {
                    1
                },
            )),
            _ => None,
        };
        let index_read = if let Expr::Index(index) = node
            && self.reads.contains(&node.span())
            && self.writes.contains(&node.span())
        {
            Some((index.object(), "[]", 1))
        } else {
            None
        };
        for (receiver, name, arity) in [operator, index_read].into_iter().flatten() {
            let lookup = if name == "-" && arity == 0 {
                "unary-"
            } else {
                name
            };
            let receiver = match node {
                Expr::Index(index) => {
                    state.infer_access_receiver(receiver, lookup, index.is_null_safe())
                }
                Expr::Assign(_) | Expr::PostfixIncDec(_) => {
                    state.infer_access_receiver(receiver, lookup, false)
                }
                Expr::Unary(unary)
                    if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
                {
                    state.infer_access_receiver(receiver, lookup, false)
                }
                _ => state.infer(receiver),
            };
            if let Some(owner) = state
                .signatures
                .resolved_member_owner(&receiver, lookup, &state.model)
                .or_else(|| {
                    state
                        .signatures
                        .resolved_extension_owner(&receiver, lookup, &state.model)
                })
            {
                self.report(&Target {
                    owner,
                    selector: MemberSelector::new(SelectorKind::Operator(arity), false, name)
                        .into(),
                    name: lookup.into(),
                    span: node.span(),
                });
            }
        }
        match node {
            Expr::Call(call) => {
                let callee = call.callee();
                let args = call.args();
                if let Expr::Ident(identifier) = callee
                    && let Some(index) = self.local(identifier.name(), identifier.span(), state)
                {
                    let mut positional = args.positional().enumerate();
                    for parameter in &self.locals[index].parameters {
                        let span = match parameter.position {
                            Some(position) => positional
                                .find(|(index, _)| *index == position)
                                .map(|(_, argument)| argument.span()),
                            None => args
                                .named()
                                .find(|argument| argument.name().name() == parameter.name)
                                .map(|argument| argument.name().span()),
                        };
                        if let Some(span) = span {
                            push_deprecation(
                                self.ctx,
                                &self.excluded,
                                &mut self.reported,
                                &mut self.diagnostics,
                                &parameter.name,
                                span,
                                &parameter.message,
                            );
                        }
                    }
                    return;
                }
                let mut target_callee = callee;
                while let Expr::GenericInstantiation(instantiation) = target_callee {
                    target_callee = instantiation.target();
                }
                if !matches!(target_callee, Expr::Paren(_))
                    && let Some(mut target) = self.reference_target(callee, state, None)
                {
                    if target.selector.member == MemberSelector::DECLARATION {
                        let owner = state.model.unaliased(target.owner.clone());
                        if state.signatures.constructor(&owner, "new").is_some() {
                            target.owner = owner;
                            target.selector =
                                MemberSelector::new(SelectorKind::Constructor, false, "new").into();
                            // Call type arguments live outside the callee span.
                            let end = self
                                .ctx
                                .tree()
                                .tokens_in(
                                    self.ctx.source,
                                    Span::new(callee.span().start, args.span().start),
                                )
                                .rfind(|token| !token.is_trivia())
                                .map_or(callee.span().end, |token| token.offset() + token.len());
                            target.span = Span::new(callee.span().start, end);
                            self.report(&target);
                        }
                    }
                    self.arguments(&target, args);
                }
                let receiver = state.infer(callee);
                if !matches!(receiver, ResolvedType::Function { .. })
                    && let Some(mut target) =
                        self.member(&receiver, "call", node.span(), state, false)
                {
                    target.name = "call".into();
                    self.report(&target);
                }
            }
            Expr::New(construction) => {
                if let Some((identity, name)) = resolve_new_constructor(
                    construction.dart_type(),
                    construction.constructor_name(),
                    &state.model,
                ) {
                    let span = crate::lint::flutter::constructor_span(node);
                    let target = constructor_target(state.model.unaliased(identity), name, span);
                    self.report(&target);
                    self.arguments(&target, construction.args());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{
        IdentityIndex, IdentitySource, ParsedFile, RuleAnalysis, SignatureIndex, TypeIndex,
    };
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;
    use std::sync::Arc;

    fn analyze(source: &str, path: &Path) -> Vec<Diagnostic> {
        let falcon_dart_parser::ParseResult { errors, tree } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path,
                falcon_analyze::LibrarySource {
                    program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let model = SemanticModel::new(path, &parsed, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        DeprecatedMemberUseFromSamePackage.analyze(program, &ctx)
    }

    #[test]
    fn deprecated_named_constructors_cover_the_generic_qualifier() {
        let source = r"
import 'bad.dart' as p;
class Old<T> {
  @Deprecated('old')
  Old();
  @Deprecated('old')
  Old.named();
}
void use() {
  Old<int>.named();
  new Old<String>.named();
  Old<double>.named;
  Old<bool> /* before arguments */ ();
  p.Old();
  p.Old<num>();
  p.Old.named();
  p.Old<int>.named();
}
";
        let mut diagnostics = analyze(source, fixture_path());
        diagnostics.sort_unstable_by_key(|diagnostic| diagnostic.span.start);
        let expected = [
            "Old<int>.named",
            "Old<String>.named",
            "Old<double>.named",
            "Old<bool>",
            "p.Old",
            "p.Old<num>",
            "p.Old.named",
            "p.Old<int>.named",
        ];
        assert_eq!(diagnostics.len(), expected.len());
        for (diagnostic, expected) in diagnostics.iter().zip(expected) {
            assert_eq!(diagnostic.span.start, source.find(expected).unwrap());
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }

    #[test]
    fn deprecated_implicit_super_reports_the_complete_constructor() {
        let source = r"
class Base {
  @Deprecated('old')
  const Base();
}
class Child extends Base {
  /// Creates.
  const Child();
  /// Before metadata.
  @pragma('test')
  /// After metadata.
  Child.named();
}
class Primary() extends Base;
";
        let mut diagnostics = analyze(source, fixture_path());
        diagnostics.sort_unstable_by_key(|diagnostic| diagnostic.span.start);
        let expected = [
            "/// Creates.\n  const Child();",
            "@pragma('test')\n  /// After metadata.\n  Child.named();",
        ];
        assert_eq!(diagnostics.len(), expected.len());
        for (diagnostic, expected) in diagnostics.iter().zip(expected) {
            assert_eq!(diagnostic.span.start, source.find(expected).unwrap());
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }

    #[test]
    fn cascade_callable_invocations_distinguish_implicit_parameter_uses() {
        let source = r"
class Callable {
  @Deprecated('callable')
  void call({@Deprecated('named') int value = 0}) {}
}
class Holder {
  Callable callable() => Callable();
  Callable operator [](int index) => Callable();
}
void use(Holder holder) {
  holder..callable()(value: 1);
  holder..[0](value: 2);
  holder.callable().call(value: 3);
}
";
        let mut diagnostics = analyze(source, fixture_path());
        diagnostics.sort_unstable_by_key(|diagnostic| diagnostic.span.start);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            actual,
            ["..callable()(value: 1)", "..[0](value: 2)", "call", "value"]
        );
    }

    fn fixture_path() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/corpus/deprecated-member-use-from-same-package/bad.dart"
        ))
    }

    #[test]
    fn typed_selectors_keep_access_static_and_parameter_metadata_distinct() {
        let source = r"
class C {
  @Deprecated('read') int get readOnly => 0;
  set readOnly(int value) {}
  int get writeOnly => 0;
  @Deprecated('write') set writeOnly(int value) {}
  @Deprecated('field') int field = 0;
  @Deprecated('static read') static int get shared => 0;
  static set shared(int value) {}
  static int get writeShared => 0;
  @Deprecated('static write') static set writeShared(int value) {}
  static void method([@Deprecated('parameter') int? value]) {}
}
void use(C c) {
  c.readOnly;
  c.readOnly = 1;
  c.writeOnly;
  c.writeOnly = 1;
  c.field;
  c.field = 1;
  C.shared;
  C.shared = 1;
  C.writeShared;
  C.writeShared = 1;
  C.method(1);
}
";
        let diagnostics = analyze(source, fixture_path());
        let mut messages: Vec<_> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        messages.sort_unstable();
        let mut expected = [
            "'readOnly' is deprecated and shouldn't be used. read.",
            "'writeOnly' is deprecated and shouldn't be used. write.",
            "'field' is deprecated and shouldn't be used. field.",
            "'field' is deprecated and shouldn't be used. field.",
            "'shared' is deprecated and shouldn't be used. static read.",
            "'writeShared' is deprecated and shouldn't be used. static write.",
            "'value' is deprecated and shouldn't be used. parameter.",
        ];
        expected.sort_unstable();
        assert_eq!(messages, expected);
    }

    #[test]
    fn typed_extension_selectors_preserve_write_and_static_dispatch() {
        let source = r"
class C {}
extension E on C {
  @Deprecated('extension read') int get value => 0;
  @Deprecated('extension write') set value(int next) {}
  @Deprecated('extension static') static void helper() {}
}
void use(C c) {
  c.value;
  c.value = 1;
  E(c).value;
  E(c).value = 1;
  E.helper();
}
";
        let diagnostics = analyze(source, fixture_path());
        let mut messages: Vec<_> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        messages.sort_unstable();
        let mut expected = [
            "'value' is deprecated and shouldn't be used. extension read.",
            "'value' is deprecated and shouldn't be used. extension read.",
            "'value' is deprecated and shouldn't be used. extension write.",
            "'value' is deprecated and shouldn't be used. extension write.",
            "'helper' is deprecated and shouldn't be used. extension static.",
        ];
        expected.sort_unstable();
        assert_eq!(messages, expected);
    }

    #[test]
    fn implicit_this_write_does_not_fall_through_to_deprecated_top_level() {
        let source = r"
@Deprecated('top-level') int x = 0;
class C {
  int x = 0;
  void writeMember() { x = 1; }
}
void writeTopLevel() { x = 1; }
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "'x' is deprecated and shouldn't be used. top-level."
        );
        let start = source.rfind("x = 1").unwrap();
        assert_eq!(
            (diagnostics[0].span.start, diagnostics[0].span.end),
            (start, start + 1)
        );
    }

    #[test]
    fn documented_operator_uses_operator_selector() {
        let source = r"
class C {
  @Deprecated('operator') C operator +(C other) => this;
}
/// See [C.+].
void documentedOperator() {}
void positiveControl(C c) { c + c; }
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.message == "'+' is deprecated and shouldn't be used. operator."
        }));
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
    fn parenthesized_calls_do_not_reference_positional_parameter_metadata() {
        let source = r"
void f<T>(int a, [@deprecated int? b]) {}
void use() {
  f<int>(1, 2);
  (f)(1, 2);
  (f<int>)(1, 2);
  ((f<int>))(1, 2);
  (f)<int>(1, 2);
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }

    #[test]
    fn grouped_references_are_visited_without_invocation_parameter_metadata() {
        let source = r"
@deprecated void old<T>([@deprecated int? argument]) {}
class Value {
  @deprecated int get oldField => 0;
}
void use(Value value) {
  (old<int>)(1);
  ((value.oldField));
  @deprecated var oldLocal = 0;
  ((oldLocal));
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.starts_with("'argument'")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn enum_constants_do_not_reference_deprecated_constructors_or_parameters() {
        let source = r"
mixin M {}
enum Mixed with M {
  a(1), b.x(), c.named(oldParameter: null);
  @deprecated const Mixed(int i);
  @deprecated const Mixed.x();
  const Mixed.named({@deprecated Object? oldParameter});
}
enum Plain {
  a(1), b.x(), c.named(oldParameter: null);
  @deprecated const Plain(int i);
  @deprecated const Plain.x();
  const Plain.named({@deprecated Object? oldParameter});
}
";
        assert!(analyze(source, fixture_path()).is_empty());
    }

    #[test]
    fn direct_constructor_calls_retain_deprecated_constructor_and_parameter_metadata() {
        let source = r"
class Plain {
  @deprecated Plain.old();
  Plain.current({@deprecated Object? oldParameter});
}
void use() {
  Plain.old();
  Plain.current(oldParameter: null);
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    }

    #[test]
    fn local_deprecation_tracks_binding_identity_and_metadata_frontiers() {
        let source = r"
void use() {
  @deprecated void oldFunction() {}
  oldFunction();
  @deprecated int Function() oldCallback = () => 1;
  oldCallback();
  @deprecated var oldValue = 1;
  print(oldValue);
  { var oldValue = 2; print(oldValue); }
  @Deprecated('x') final (a, b) = (1, 2);
  print(a);
  for (@deprecated var oldElement in [1]) { print(oldElement); }
  { void oldFunction() {} oldFunction(); }
  oldFunction();
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
    }

    #[test]
    fn implicit_extension_unary_operator_uses_declaration_metadata() {
        let source = r"
class Value {}
extension Operations on Value {
  @deprecated Value operator -() => this;
}
void use(Value value) { -value; }
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            diagnostics[0].message,
            "'unary-' is deprecated and shouldn't be used."
        );
    }

    #[test]
    fn nullable_extensions_preserve_receiver_nullability_except_null_safe_access() {
        let source = r"
class Value {}
extension Nullable on Value? {
  @deprecated void nullable() {}
  @deprecated void shared() {}
}
extension NonNullable on Value {
  void shared() {}
  @deprecated void strict() {}
}
void use(Value? value) {
  value.nullable();
  value?.nullable();
  value.shared();
  value?.shared();
  value?.strict();
  value..shared();
  value?..shared();
  value?..strict();
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 6, "{diagnostics:?}");
    }

    #[test]
    fn null_safe_index_reads_and_writes_resolve_deprecated_operators() {
        for declaration in [
            r"class Collection {
  @deprecated Value operator [](Key key) => Value();
  @deprecated void operator []=(Key key, Value value) {}
}",
            r"class Collection {}
extension Operations on Collection {
  @deprecated Value operator [](Key key) => Value();
  @deprecated void operator []=(Key key, Value value) {}
}",
        ] {
            let source = format!(
                r"
class Key {{}}
class Value {{}}
{declaration}
void use(Collection? collection, Key key, Value value) {{
  collection?[key];
  collection?[key] = value;
}}
"
            );
            let diagnostics = analyze(&source, fixture_path());
            assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.starts_with("'[]'")),
                "{diagnostics:?}"
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.starts_with("'[]='")),
                "{diagnostics:?}"
            );
        }
    }

    #[test]
    fn null_shortened_member_receivers_preserve_declared_nullable_types_and_grouping() {
        let source = r"
class Value {}
class Key {}
class Container {
  Value get value => Value();
  Value? get maybe => null;
  Value make() => Value();
  Value? makeMaybe() => null;
  Value operator [](Key key) => Value();
}
extension NonNullable on Value {
  @deprecated void old() {}
}
extension Nullable on Value? {
  void old() {}
}
void use(Container? container, Key key) {
  container?.value.old();
  container?.maybe.old();
  container?.make().old();
  container?.makeMaybe().old();
  (container?.value).old();
  (container?.make()).old();
  container?.maybe?.old();
  container?[key].old();
  (container?[key]).old();
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.message.starts_with("'old'")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn null_shortened_compound_targets_preserve_declared_nullable_operator_receivers() {
        let source = r"
class Key {}
class Counter {
  @deprecated Counter operator +(int other) => this;
}
extension NullableCounter on Counter? {
  Counter? operator +(int other) => this;
}
class Holder {
  Counter count = Counter();
  Counter? maybe;
}
class Collection {
  Counter operator [](Key key) => Counter();
  void operator []=(Key key, Counter value) {}
}
class NullableCollection {
  Counter? operator [](Key key) => null;
  void operator []=(Key key, Counter? value) {}
}
void use(Holder? holder, Collection? collection,
    NullableCollection? nullableCollection, Key key) {
  holder?.count += 1;
  holder?.count++;
  ++holder?.count;
  holder?.maybe += 1;
  holder?.maybe++;
  collection?[key] += 1;
  collection?[key]++;
  nullableCollection?[key] += 1;
  nullableCollection?[key]++;
  (holder?.count) + 1;
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.message.starts_with("'+'")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn null_shortened_index_receivers_resolve_declared_member_types() {
        let source = r"
class Key {}
class Value {}
class Collection {
  @deprecated Value operator [](Key key) => Value();
  @deprecated void operator []=(Key key, Value value) {}
}
class Container {
  Collection get items => Collection();
}
void use(Container? container, Key key, Value value) {
  container?.items[key];
  container?.items[key] = value;
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    }

    #[test]
    fn local_function_metadata_applies_before_its_declaration() {
        let source = r"
void helper() {}
void forward() {
  old();
  @deprecated void old() {}
}
void use(int value) {
  helper();
  switch (value) {
    case 0:
      helper();
      @deprecated void helper() {}
      helper();
      break;
    case 1:
      helper();
      void helper() {}
      helper();
      break;
    default:
      helper();
  }
  helper();
}
";
        let diagnostics = analyze(source, fixture_path());
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    }

    #[test]
    fn cascade_index_spans_follow_each_index_in_the_local_token_store() {
        let source = r#"
class Box {
  Box get items => this;
  Box add(List<int> values) => this;
  @deprecated Box operator [](int index) => this;
}
void use(Box box) {
  box..items[0][1];
  box..items?[2];
  box..add([3])[4];
  print('${(box..items[5])}');
}
"#;
        let mut diagnostics = analyze(source, fixture_path());
        diagnostics.sort_unstable_by_key(|diagnostic| (diagnostic.span.start, diagnostic.span.end));
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            actual,
            [
                "..items[0]",
                "..items[0][1]",
                "..items?[2]",
                "..add([3])[4]",
                "..items[5]",
            ]
        );
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
            "@deprecated library; @deprecated void old() {} void use() { old(); }",
            "@deprecated void old() { old(); } @deprecated void use() { old(); }",
        ] {
            assert!(analyze(source, fixture_path()).is_empty(), "{source}");
        }
    }
}
