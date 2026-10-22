//! Report public declarations that no entry point of an executable library
//! reaches.
//!
//! A library — its defining unit plus every `part` file — is executable when it
//! declares a top-level function named `main`, or a top-level function
//! annotated `@pragma('vm:entry-point')` whose `pragma` provably resolves to
//! `dart:core`. Starting from those entry points (and from top-level functions
//! annotated with a widget-preview `@Preview`), the rule follows references:
//! identifiers, invocations, constructor names and tear-offs, annotations,
//! user-defined operators, object-pattern fields, doc comment references,
//! supertypes named in a class's `extends`/`with`/`implements` clause or an
//! extension type's `implements` clause, implicit super-constructor calls,
//! typedef and extension-type names, type literals, type arguments, and the
//! types of `external` members. Type parameters shadow same-named library
//! declarations. A plain type annotation is not a reference, and
//! constructor invocations inside constant patterns do not count. Instance
//! `toJson` methods are reachable with their container, because `dart:convert`
//! calls them dynamically.
//! Documentation references use the shared Dartdoc parser, excluding Markdown
//! link display text and link definitions, inline and block code, and Dartdoc
//! directives. Reference-style destination labels remain references.
//! A block documentation comment replaces immediately preceding line docs,
//! following the shared documentation grouping used by other rules.
//!
//! The candidates are the library's top-level declarations and the public
//! constructors (except enum constructors), fields, methods, getters, setters,
//! and operators of public classes, enums, mixins, extensions, and extension
//! types. A member that overrides a supertype member is not a candidate, nor
//! are the test-harness methods `test_*`, `solo_test_*`, `setUp`, `tearDown`,
//! `setUpClass`, and `tearDownClass`. Unreached public candidates are reported
//! unless annotated `@visibleForTesting` or a widget preview; a member other
//! than a field is not reported again when its enclosing declaration already
//! is (the analyzer reports unreachable fields of an unreachable class too).
//! Libraries that declare a top-level name twice are not analyzed.
//!
//! Facts falcon cannot prove are resolved toward reachability, so uncertainty
//! causes misses rather than false positives: a member whose container has a
//! supertype with an incomplete indexed hierarchy may override it and is not
//! a candidate. Known receiver types resolve members and operators before
//! considering extensions. An unknown receiver reaches same-named members
//! only in extensions or classes named outside their own declaration (a class
//! never named cannot be its static type); an identifier counts unless it
//! resolves to a local. A library with a part that does not resolve to an
//! analyzed file is not analyzed.
//!
//! Adopted from the official `unreachable_from_main` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, QualifiedName, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    Visitor, walk_annotation, walk_class_member, walk_dart_type, walk_expr, walk_pattern,
    walk_stmt, walk_top_level_decl,
};
use falcon_syntax::cst::*;

use crate::lint::declarations::operator_token_span;
use crate::lint::string_lit::is_ident_body;
use falcon_analyze::resolve::lexical_usage::locally_bound_identifiers;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

/// Members every class inherits from `Object`; declaring one is an override.
const OBJECT_MEMBERS: &[&str] = &["==", "hashCode", "toString", "noSuchMethod", "runtimeType"];

/// Bound on typedef-alias chains followed while resolving a named type.
const MAX_ALIAS_DEPTH: usize = 8;

#[derive(Default)]
pub struct UnreachableFromMain;

impl Rule for UnreachableFromMain {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(units) = library_units(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let tables = Tables::build(&units, ctx);
        if !tables.executable || tables.conflicting {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Reachability {
            units,
            tables,
            ctx,
            diagnostics: None,
            primary_types: ExpressionTypes::default(),
        }))
    }
}

struct Reachability<'a> {
    units: Vec<Unit<'a>>,
    tables: Tables<'a>,
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Option<Vec<Diagnostic>>,
    primary_types: ExpressionTypes<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Reachability<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        self.primary_types.visit_expr(node, state);
    }

    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        self.diagnostics = Some(self.diagnostics(Some(state)));
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
            .take()
            .unwrap_or_else(|| self.diagnostics(None))
    }
}

impl<'a> Reachability<'a> {
    fn diagnostics(&self, primary_state: Option<&SemanticState<'a>>) -> Vec<Diagnostic> {
        let ctx = self.ctx;
        let tables = &self.tables;

        let mut deps: Vec<Vec<DeclId>> = vec![Vec::new(); tables.decls.len()];
        for (index, unit) in self.units.iter().enumerate() {
            let bound = locally_bound_identifiers(unit.program);
            let mut sibling_types = ExpressionTypes::default();
            // A sibling needs its own parsed backing and per-file semantic caches.
            let sibling_ctx = (index != 0).then(|| ctx.for_file(unit.path, unit.parsed));
            let sibling_state = sibling_ctx
                .as_ref()
                .and_then(|ctx| SemanticState::new(unit.program, ctx))
                .map(|state| visit_program(&mut sibling_types, unit.program, state));
            let (state, types) = if index == 0 {
                (primary_state, &self.primary_types)
            } else {
                (sibling_state.as_ref(), &sibling_types)
            };
            let mut walker = RefWalker::new(tables, index, unit.parsed, &bound, &mut deps);
            walker.semantic = state.map(|state| (state, types));
            for declaration in unit.program.declarations() {
                walker.visit_top_level_decl(declaration);
            }
            walker.doc_references();
        }

        let mut reached = vec![false; tables.decls.len()];
        let mut pending = tables.roots.clone();
        for &root in &pending {
            reached[root] = true;
        }
        while let Some(declaration) = pending.pop() {
            for &dependency in &deps[declaration] {
                if !reached[dependency] {
                    reached[dependency] = true;
                    pending.push(dependency);
                }
            }
        }

        let mut diagnostics: Vec<Diagnostic> = tables
            .decls
            .iter()
            .enumerate()
            .filter(|(id, decl)| {
                decl.unit == 0
                    && decl.reportable
                    && !reached[*id]
                    && (decl.field
                        || decl
                            .container
                            .is_none_or(|container| reached[tables.containers[container].decl]))
            })
            .map(|(_, decl)| {
                ctx.diagnostic(format!(
                    "Unreachable member `{}` in an executable library.",
                    decl.label
                ), decl.name_span)
            })
            .collect();
        diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
        diagnostics
    }
}

/// One compilation unit of the analyzed library. Unit 0 is the file under
/// analysis; the rest are its library siblings.
struct Unit<'a> {
    program: Program<'a>,
    parsed: &'a std::sync::Arc<falcon_analyze::ParsedFile>,
    path: &'a std::path::Path,
}

/// The analyzed file plus every sibling unit of its library, each with the
/// source text it was parsed from. `None` when the library is incomplete.
fn library_units<'a>(program: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> Option<Vec<Unit<'a>>> {
    let current = Unit {
        program,
        parsed: ctx.parsed(),
        path: ctx.file_path(),
    };
    if ctx
        .library()
        .is_some_and(|library| library.has_unresolved_parts())
    {
        return None;
    }
    let Some(library) = ctx
        .library()
        .filter(|library| !library.siblings().is_empty())
    else {
        let standalone =
            program.part_of_directive().is_none() && program.part_directives().is_empty();
        return standalone.then(|| vec![current]);
    };
    let siblings = library
        .siblings()
        .iter()
        .zip(library.sibling_parsed())
        .zip(library.sibling_paths())
        .map(|((&program, parsed), &path)| Unit {
            program,
            parsed,
            path,
        });
    Some(std::iter::once(current).chain(siblings).collect())
}

fn top_level_name(declaration: TopLevelDecl<'_>) -> Option<Identifier<'_>> {
    match declaration {
        TopLevelDecl::Class(x) => Some(x.name()),
        TopLevelDecl::ClassTypeAlias(x) => Some(x.name()),
        TopLevelDecl::Mixin(x) => Some(x.name()),
        TopLevelDecl::MixinClass(x) => Some(x.name()),
        TopLevelDecl::Enum(x) => Some(x.name()),
        TopLevelDecl::Extension(x) => x.name(),
        TopLevelDecl::ExtensionType(x) => Some(x.name()),
        TopLevelDecl::Function(x) => Some(x.name()),
        TopLevelDecl::Variable(x) => x.declarators().next().map(|declarator| declarator.name()),
        TopLevelDecl::TypeAlias(x) => Some(x.name()),
        TopLevelDecl::Error(_) => None,
    }
}

type DeclId = usize;
type ContainerId = usize;

/// A declaration that reachability is computed for.
struct Decl {
    /// The name used in the diagnostic message.
    label: String,
    unit: usize,
    name_span: Span,
    /// The enclosing class-like declaration of a member.
    container: Option<ContainerId>,
    /// The class-like scope references in this declaration's doc comment
    /// resolve in: the container of a member, or the container itself.
    scope: Option<ContainerId>,
    /// A public candidate that is not exempt from reporting.
    reportable: bool,
    /// Start of the leading doc comment, or of the declaration without one.
    doc_start: usize,
    end: usize,
    /// A field: the analyzer reports an unreachable field even when its
    /// enclosing declaration is reported too.
    field: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    /// A class or mixin class declaration (with a body).
    Class,
    /// A mixin-application class `class C = S with M;`.
    ClassTypeAlias,
    Mixin,
    Enum,
    Extension,
    ExtensionType,
}

/// A class-like top-level declaration.
struct Container<'a> {
    decl: DeclId,
    kind: ContainerKind,
    superclass: Option<DartType<'a>>,
    /// Types whose members this declaration inherits or must implement.
    supertypes: Vec<DartType<'a>>,
    /// Every member name in the declaration's lexical scope.
    declared: HashSet<&'a str>,
    /// Instance member names, for override detection.
    instance: HashSet<&'a str>,
    constructors: HashSet<&'a str>,
    /// Candidate non-constructor members by name.
    members: HashMap<&'a str, Vec<DeclId>>,
    /// Candidate constructors by name (`new` for the unnamed one).
    candidate_constructors: HashMap<&'a str, DeclId>,
    members_source: AstChildren<'a, ClassMember<'a>>,
    public: bool,
    unit: usize,
}

impl<'a> Container<'a> {
    fn new(
        decl: DeclId,
        kind: ContainerKind,
        members_source: AstChildren<'a, ClassMember<'a>>,
        public: bool,
        unit: usize,
    ) -> Self {
        Self {
            decl,
            kind,
            superclass: None,
            supertypes: Vec::new(),
            declared: HashSet::new(),
            instance: HashSet::new(),
            constructors: HashSet::new(),
            members: HashMap::new(),
            candidate_constructors: HashMap::new(),
            members_source,
            public,
            unit,
        }
    }
}

struct Tables<'a> {
    decls: Vec<Decl>,
    containers: Vec<Container<'a>>,
    /// Top-level declarations by name.
    top: HashMap<&'a str, Vec<DeclId>>,
    container_named: HashMap<&'a str, ContainerId>,
    container_identities: HashMap<falcon_analyze::DeclarationIdentity, ContainerId>,
    extension_on_types: Vec<(ContainerId, falcon_analyze::ResolvedType)>,
    alias_named: HashMap<&'a str, DartType<'a>>,
    /// Top-level variable initializers, for annotations through a constant.
    initializers: HashMap<&'a str, Expr<'a>>,
    /// Candidate non-constructor members of every container, by name.
    members_named: HashMap<&'a str, Vec<DeclId>>,
    /// Declaration ids by `(unit, span start)` of their declaring node.
    at: HashMap<(usize, usize), DeclId>,
    container_at: HashMap<(usize, usize), ContainerId>,
    roots: Vec<DeclId>,
    executable: bool,
    /// Names declared by more than one top-level declaration (other than a
    /// getter and setter pair); such invalid code is not analyzed.
    defined: HashSet<&'a str>,
    conflicting: bool,
    /// Per container: whether its name occurs anywhere in the library outside
    /// its own declaration, as an identifier, type, comment, or string.
    named_elsewhere: Vec<bool>,
}

impl<'a> Tables<'a> {
    fn build(units: &[Unit<'a>], ctx: &AnalyzeContext) -> Self {
        let mut tables = Tables {
            decls: Vec::new(),
            containers: Vec::new(),
            top: HashMap::new(),
            container_named: HashMap::new(),
            container_identities: HashMap::new(),
            extension_on_types: Vec::new(),
            alias_named: HashMap::new(),
            initializers: HashMap::new(),
            members_named: HashMap::new(),
            at: HashMap::new(),
            container_at: HashMap::new(),
            roots: Vec::new(),
            executable: false,
            defined: HashSet::new(),
            conflicting: false,
            named_elsewhere: Vec::new(),
        };
        for (unit, source) in units.iter().enumerate() {
            for declaration in source.program.declarations() {
                tables.add_top_level(
                    unit,
                    &source.parsed.source,
                    &source.parsed.tree,
                    declaration,
                );
            }
        }
        let model = ctx.identities().map(|identities| {
            falcon_analyze::SemanticModel::new(
                ctx.file_path(),
                ctx.parsed(),
                identities,
                ctx.types(),
            )
        });
        let semantic = model.as_ref().zip(ctx.signatures());
        if let Some((model, signatures)) = semantic {
            tables.container_identities.reserve(tables.containers.len());
            for (container, declaration) in tables.containers.iter().enumerate() {
                let label = &tables.decls[declaration.decl].label;
                let identity = if declaration.kind == ContainerKind::Extension {
                    model.resolve_extension(label)
                } else {
                    model.resolve_name(label)
                };
                if let Some(identity) = identity {
                    tables
                        .container_identities
                        .entry(identity)
                        .or_insert(container);
                }
                if declaration.kind == ContainerKind::Extension
                    && let Some(on_type) = signatures.visible_extension_on_type(label, model)
                {
                    tables.extension_on_types.push((container, on_type.clone()));
                }
            }
        }
        for container in 0..tables.containers.len() {
            tables.add_members(
                container,
                &units[tables.containers[container].unit].parsed.source,
                &units[tables.containers[container].unit].parsed.tree,
                semantic,
            );
        }
        tables.find_entry_points(units, ctx);
        tables.find_named_elsewhere(units);
        tables
    }

    fn add_decl(&mut self, unit: usize, key: usize, decl: Decl) -> DeclId {
        let id = self.decls.len();
        self.decls.push(Decl { unit, ..decl });
        self.at.insert((unit, key), id);
        id
    }

    fn add_top_level(
        &mut self,
        unit: usize,
        source: &str,
        tree: &SyntaxTree,
        declaration: TopLevelDecl<'a>,
    ) {
        let annotations = match declaration {
            TopLevelDecl::Class(x) => x.annotations(),
            TopLevelDecl::ClassTypeAlias(x) => x.annotations(),
            TopLevelDecl::Mixin(x) => x.annotations(),
            TopLevelDecl::MixinClass(x) => x.annotations(),
            TopLevelDecl::Enum(x) => x.annotations(),
            TopLevelDecl::Extension(x) => x.annotations(),
            TopLevelDecl::ExtensionType(x) => x.annotations(),
            TopLevelDecl::Function(x) => x.annotations(),
            TopLevelDecl::Variable(x) => x.annotations(),
            TopLevelDecl::TypeAlias(x) => x.annotations(),
            TopLevelDecl::Error(_) => return,
        };
        let span = declaration.span();
        let doc_start = doc_start(
            source,
            tree,
            annotated_start(annotations.clone(), span.start),
        );
        let exempt = annotations.clone().any(is_visible_for_testing);

        if let TopLevelDecl::Variable(variable) = declaration {
            for declarator in variable.declarators() {
                let id = self.add_decl(
                    unit,
                    declarator.span().start,
                    Decl {
                        label: declarator.name().name().to_string(),
                        unit,
                        name_span: declarator.name().span(),
                        container: None,
                        scope: None,
                        reportable: is_public(declarator.name().name()) && !exempt,
                        doc_start,
                        end: declarator.span().end,
                        field: false,
                    },
                );
                self.top
                    .entry(declarator.name().name())
                    .or_default()
                    .push(id);
                self.conflicting |= !self.defined.insert(declarator.name().name());
                if let Some(initializer) = declarator.initializer() {
                    self.initializers
                        .insert(declarator.name().name(), initializer);
                }
            }
            return;
        }

        let Some(name) = top_level_name(declaration) else {
            // An unnamed extension: never referenced by name, never reported,
            // but its members are reached through implicit invocations.
            if let TopLevelDecl::Extension(extension) = declaration {
                let id = self.add_decl(
                    unit,
                    span.start,
                    Decl {
                        label: String::new(),
                        unit,
                        name_span: span,
                        container: None,
                        scope: None,
                        reportable: false,
                        doc_start,
                        end: span.end,
                        field: false,
                    },
                );
                self.add_container(
                    span.start,
                    Container::new(
                        id,
                        ContainerKind::Extension,
                        extension.members(),
                        false,
                        unit,
                    ),
                );
            }
            return;
        };
        let id = self.add_decl(
            unit,
            span.start,
            Decl {
                label: name.name().to_string(),
                unit,
                name_span: name.span(),
                container: None,
                scope: None,
                reportable: is_public(name.name()) && !exempt,
                doc_start,
                end: span.end,
                field: false,
            },
        );
        self.top.entry(name.name()).or_default().push(id);
        let accessor = matches!(declaration, TopLevelDecl::Function(function)
            if !matches!(function.kind(), FunctionKind::Function(_)));
        if !accessor {
            self.conflicting |= !self.defined.insert(name.name());
        }

        let public = is_public(name.name());
        let (kind, superclass, supertypes, members): (_, _, Vec<DartType<'a>>, _) =
            match declaration {
                TopLevelDecl::Class(x) => (
                    ContainerKind::Class,
                    x.extends(),
                    x.extends()
                        .into_iter()
                        .chain(x.with_clause())
                        .chain(x.implements())
                        .collect(),
                    x.members(),
                ),
                TopLevelDecl::MixinClass(x) => (
                    ContainerKind::Class,
                    x.extends(),
                    x.extends()
                        .into_iter()
                        .chain(x.with_clause())
                        .chain(x.implements())
                        .collect(),
                    x.members(),
                ),
                TopLevelDecl::ClassTypeAlias(x) => (
                    ContainerKind::ClassTypeAlias,
                    Some(x.superclass()),
                    std::iter::once(x.superclass())
                        .chain(x.with_clause())
                        .chain(x.implements())
                        .collect(),
                    AstChildren::empty(),
                ),
                TopLevelDecl::Mixin(x) => (
                    ContainerKind::Mixin,
                    None,
                    x.on_clause().chain(x.implements()).collect(),
                    x.members(),
                ),
                TopLevelDecl::Enum(x) => (
                    ContainerKind::Enum,
                    None,
                    x.with_clause().chain(x.implements()).collect(),
                    x.members(),
                ),
                TopLevelDecl::Extension(x) => {
                    (ContainerKind::Extension, None, Vec::new(), x.members())
                }
                TopLevelDecl::ExtensionType(x) => (
                    ContainerKind::ExtensionType,
                    None,
                    x.implements().collect(),
                    x.members(),
                ),
                TopLevelDecl::TypeAlias(x) => {
                    self.alias_named.insert(name.name(), x.aliased());
                    return;
                }
                TopLevelDecl::Function(_) | TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => {
                    return;
                }
            };
        let container = self.add_container(
            span.start,
            Container {
                superclass,
                supertypes,
                ..Container::new(id, kind, members, public, unit)
            },
        );
        self.container_named.entry(name.name()).or_insert(container);
        self.decls[id].scope = Some(container);
        let lexical = &mut self.containers[container];
        match declaration {
            TopLevelDecl::Enum(x) => {
                lexical
                    .declared
                    .extend(x.variants().map(|variant| variant.name().name()));
            }
            TopLevelDecl::ExtensionType(x) => {
                if let Some(representation) = x.representation() {
                    let field = representation.name().name();
                    lexical.declared.insert(field);
                    lexical.instance.insert(field);
                }
                lexical.constructors.insert(
                    x.primary_constructor()
                        .constructor_name()
                        .map_or("new", |name| name.name()),
                );
            }
            _ => {}
        }
    }

    /// Register a container, indexing the names its members declare.
    fn add_container(&mut self, key: usize, mut container: Container<'a>) -> ContainerId {
        for member in container.members_source.clone() {
            let (is_static, single, fields) = match member {
                ClassMember::Constructor(constructor) => {
                    container.constructors.insert(constructor_key(constructor));
                    continue;
                }
                ClassMember::Field(x) => (x.is_static(), None, Some(x.declarators())),
                ClassMember::Method(x) => (x.is_static(), Some(x.name().name()), None),
                ClassMember::Getter(x) => (x.is_static(), Some(x.name().name()), None),
                ClassMember::Setter(x) => (x.is_static(), Some(x.name().name()), None),
                ClassMember::Operator(x) => (false, Some(x.lookup_name()), None),
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => continue,
            };
            for name in single.into_iter().chain(
                fields
                    .into_iter()
                    .flatten()
                    .map(|declarator| declarator.name().name()),
            ) {
                if !is_static {
                    container.instance.insert(name);
                }
                container.declared.insert(name);
            }
        }
        let id = self.containers.len();
        self.container_at.insert((container.unit, key), id);
        self.containers.push(container);
        id
    }

    /// Register the candidate members of a public container. Runs after every
    /// container is known, so override detection sees the whole library.
    fn add_members(
        &mut self,
        container: ContainerId,
        source: &str,
        tree: &SyntaxTree,
        semantic: Option<(
            &falcon_analyze::SemanticModel<'_>,
            &falcon_analyze::SignatureIndex,
        )>,
    ) {
        let Container {
            public,
            unit,
            kind,
            ref members_source,
            decl,
            ..
        } = self.containers[container];
        if !public {
            return;
        }
        let owner = self.decls[decl].label.clone();
        let members_source = members_source.clone();
        for member in members_source {
            let span = member.span();
            let (annotations, is_static) = match member {
                ClassMember::Field(x) => (x.annotations(), x.is_static()),
                ClassMember::Constructor(x) => (x.annotations(), true),
                ClassMember::Method(x) => (x.annotations(), x.is_static()),
                ClassMember::Getter(x) => (x.annotations(), x.is_static()),
                ClassMember::Setter(x) => (x.annotations(), x.is_static()),
                ClassMember::Operator(x) => (x.annotations(), false),
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => continue,
            };
            let doc_start = doc_start(
                source,
                tree,
                annotated_start(annotations.clone(), span.start),
            );
            let visible_for_testing = annotations.clone().any(is_visible_for_testing);
            let preview = annotations.clone().any(|a| self.is_preview(a));
            let mut entries: Vec<(usize, &'a str, String, Span, Span)> = Vec::new();
            match member {
                ClassMember::Constructor(constructor) => {
                    let key = constructor_key(constructor);
                    if kind == ContainerKind::Enum || !is_public(key) {
                        continue;
                    }
                    let name_span = constructor
                        .constructor_name()
                        .map_or(constructor.name().span(), |name| name.span());
                    let label = format!("{owner}.{key}");
                    entries.push((span.start, key, label, name_span, span));
                }
                ClassMember::Field(field) => {
                    for declarator in field.declarators() {
                        let name = declarator.name().name();
                        if is_public(name)
                            && (is_static || !self.may_override(container, name, semantic))
                        {
                            entries.push((
                                declarator.span().start,
                                name,
                                name.to_string(),
                                declarator.name().span(),
                                Span::new(span.start, declarator.span().end),
                            ));
                        }
                    }
                }
                ClassMember::Operator(operator) => {
                    let key = operator.lookup_name();
                    if !self.may_override(container, key, semantic) {
                        let name_span = operator_token_span(operator);
                        entries.push((span.start, key, operator.op().to_string(), name_span, span));
                    }
                }
                ClassMember::Method(_) | ClassMember::Getter(_) | ClassMember::Setter(_) => {
                    let name = match member {
                        ClassMember::Method(x) => x.name(),
                        ClassMember::Getter(x) => x.name(),
                        ClassMember::Setter(x) => x.name(),
                        _ => unreachable!(),
                    };
                    let key = name.name();
                    if is_public(key)
                        && !is_test_method(key)
                        && (is_static || !self.may_override(container, key, semantic))
                    {
                        entries.push((span.start, key, key.to_string(), name.span(), span));
                    }
                }
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
            }
            // Widget previews apply to constructors and static methods.
            let preview_exempt = preview
                && match member {
                    ClassMember::Constructor(_) => true,
                    ClassMember::Method(method) => method.is_static(),
                    _ => false,
                };
            let is_constructor = matches!(member, ClassMember::Constructor(_));
            for (at, key, label, name_span, range) in entries {
                let id = self.add_decl(
                    unit,
                    at,
                    Decl {
                        label,
                        unit,
                        name_span,
                        container: Some(container),
                        scope: Some(container),
                        reportable: !visible_for_testing && !preview_exempt,
                        doc_start: doc_start.min(range.start),
                        end: range.end,
                        field: matches!(member, ClassMember::Field(_)),
                    },
                );
                if is_constructor {
                    self.containers[container]
                        .candidate_constructors
                        .insert(key, id);
                } else {
                    self.containers[container]
                        .members
                        .entry(key)
                        .or_default()
                        .push(id);
                    self.members_named.entry(key).or_default().push(id);
                }
            }
        }
    }

    /// Whether an instance member named `name` of `container` may override a
    /// supertype member. Unknown supertypes answer yes.
    fn may_override(
        &self,
        container: ContainerId,
        name: &str,
        semantic: Option<(
            &falcon_analyze::SemanticModel<'_>,
            &falcon_analyze::SignatureIndex,
        )>,
    ) -> bool {
        if OBJECT_MEMBERS.contains(&name) {
            return true;
        }
        let declaration = &self.containers[container];
        match declaration.kind {
            ContainerKind::Extension | ContainerKind::ExtensionType => return false,
            ContainerKind::Enum if name == "index" => return true,
            _ => {}
        }
        let mut visited = HashSet::from([container]);
        let mut pending = declaration.supertypes.clone();
        while let Some(supertype) = pending.pop() {
            if is_object(supertype) && !self.top.contains_key("Object") {
                continue;
            }
            let Some(parent) = self.library_container(supertype) else {
                if let Some((model, signatures)) = semantic
                    && let falcon_analyze::ResolvedType::Interface { identity, .. } =
                        model.resolve_type(supertype)
                {
                    match signatures.supertype_matches(&identity, &|identity| {
                        !signatures.declared_member_facts(identity, name).is_empty()
                            || (matches!(identity, falcon_analyze::DeclarationIdentity::Sdk { .. })
                                && *identity != falcon_analyze::DeclarationIdentity::OBJECT
                                && signatures.type_kind(identity).is_none())
                    }) {
                        falcon_analyze::Truth::Yes => return true,
                        falcon_analyze::Truth::No => continue,
                        falcon_analyze::Truth::Unknown => {}
                    }
                }
                return true;
            };
            if !visited.insert(parent) {
                continue;
            }
            let parent = &self.containers[parent];
            if parent.instance.contains(name) {
                return true;
            }
            pending.extend(parent.supertypes.iter().copied());
        }
        false
    }

    /// The library container a written type denotes, following typedefs.
    fn library_container(&self, ty: DartType<'a>) -> Option<ContainerId> {
        let mut ty = ty;
        for _ in 0..MAX_ALIAS_DEPTH {
            let DartType::Named(named) = ty else {
                return None;
            };
            let mut segments = named.segments();
            let segment = segments.next()?;
            if segments.next().is_some() {
                return None;
            }
            if let Some(&container) = self.container_named.get(segment.name()) {
                return Some(container);
            }
            ty = *self.alias_named.get(segment.name())?;
        }
        None
    }

    /// The library container a top-level name denotes, following typedefs.
    fn container_for(&self, name: &str) -> Option<ContainerId> {
        if let Some(&container) = self.container_named.get(name) {
            return Some(container);
        }
        self.library_container(*self.alias_named.get(name)?)
    }

    fn find_named_elsewhere(&mut self, units: &[Unit<'_>]) {
        let mut by_name: HashMap<&str, Vec<ContainerId>> = HashMap::new();
        for (id, container) in self.containers.iter().enumerate() {
            let label = self.decls[container.decl].label.as_str();
            if !label.is_empty() {
                by_name.entry(label).or_default().push(id);
            }
        }
        let mut named = vec![false; self.containers.len()];
        // Deliberately include names in comments and strings: this conservative
        // escape check must not conclude that a container is never named elsewhere.
        for (unit, source) in units.iter().enumerate() {
            let bytes = source.parsed.source.as_bytes();
            let mut index = 0;
            while index < bytes.len() {
                if !is_ident_body(bytes[index]) {
                    index += 1;
                    continue;
                }
                let start = index;
                while index < bytes.len() && is_ident_body(bytes[index]) {
                    index += 1;
                }
                let Some(containers) = source
                    .parsed
                    .source
                    .get(start..index)
                    .and_then(|w| by_name.get(w))
                else {
                    continue;
                };
                for &container in containers {
                    let decl = &self.decls[self.containers[container].decl];
                    if decl.unit != unit || start < decl.doc_start || start >= decl.end {
                        named[container] = true;
                    }
                }
            }
        }
        self.named_elsewhere = named;
    }

    fn find_entry_points(&mut self, units: &[Unit<'a>], ctx: &AnalyzeContext) {
        let library = ctx.identities().and_then(|identities| {
            Some((
                identities,
                identities.library(identities.file(ctx.file_path())?),
            ))
        });
        for (unit, source) in units.iter().enumerate() {
            for declaration in source.program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(&id) = self.at.get(&(unit, function.span().start)) else {
                    continue;
                };
                let is_main = function.name().name() == "main";
                let pragmas: Vec<QualifiedName<'_>> = function
                    .annotations()
                    .filter_map(|annotation| self.entry_point_pragma(annotation))
                    .collect();
                let preview = function.annotations().any(|a| self.is_preview(a));
                if preview {
                    self.decls[id].reportable = false;
                }
                if !is_main && pragmas.is_empty() && !preview {
                    continue;
                }
                self.roots.push(id);
                let proven_pragma = library.is_some_and(|(identities, library)| {
                    pragmas.iter().any(|&name| {
                        identities
                            .resolve_sdk_member(library, name)
                            .is_some_and(|identity| identity.is_sdk("dart:core", "pragma"))
                    })
                });
                self.executable |= is_main || proven_pragma;
            }
        }
    }

    /// The written `pragma` name of an `@pragma('vm:entry-point')` annotation,
    /// directly or through a top-level constant.
    fn entry_point_pragma(&self, annotation: Annotation<'a>) -> Option<QualifiedName<'a>> {
        let mut segments = annotation.name();
        let single = segments.next()?;
        let (callee, args) = if segments.next().is_none() && annotation.args().is_none() {
            invocation(*self.initializers.get(single.name())?)?
        } else {
            (
                QualifiedName::from_identifiers(annotation.name())?,
                annotation.args()?,
            )
        };
        let is_entry_point = callee.name == "pragma"
            && matches!(args.positional().first().map(Expr::unparenthesized), Some(Expr::StringLit(literal))
                if literal.interpolations().next().is_none() && literal.value() == "vm:entry-point");
        is_entry_point.then_some(callee)
    }

    /// Whether an annotation is a widget-preview `@Preview(...)`, directly or
    /// through a top-level constant. A library-declared `Preview` is not the
    /// Flutter one.
    fn is_preview(&self, annotation: Annotation<'a>) -> bool {
        if self.top.contains_key("Preview") {
            return false;
        }
        let mut segments = annotation.name();
        let Some(single) = segments.next() else {
            return false;
        };
        let callee = if segments.next().is_none() && annotation.args().is_none() {
            self.initializers
                .get(single.name())
                .and_then(|&initializer| invocation(initializer))
                .map(|(callee, _)| callee)
        } else {
            QualifiedName::from_identifiers(annotation.name())
        };
        callee.is_some_and(|callee| callee.name == "Preview")
    }
}

/// The invoked (possibly prefixed) name and arguments of `name(...)`,
/// `prefix.name(...)`, or `const name(...)`.
fn invocation(expr: Expr<'_>) -> Option<(QualifiedName<'_>, ArgList<'_>)> {
    match expr.unparenthesized() {
        Expr::Call(call) => {
            let name = match call.callee().unparenthesized() {
                Expr::Ident(name) => QualifiedName::from(name.name()),
                Expr::Field(field) => match field.object().unparenthesized() {
                    Expr::Ident(prefix) => {
                        QualifiedName::prefixed(prefix.name(), field.field().name())
                    }
                    _ => return None,
                },
                _ => return None,
            };
            Some((name, call.args()))
        }
        Expr::New(new) if new.constructor_name().is_none() => {
            let DartType::Named(named) = new.dart_type() else {
                return None;
            };
            Some((
                QualifiedName::from_identifiers(named.segments())?,
                new.args(),
            ))
        }
        _ => None,
    }
}

fn is_visible_for_testing(annotation: Annotation<'_>) -> bool {
    annotation
        .name()
        .last()
        .is_some_and(|name| name.name() == "visibleForTesting")
}

fn is_public(name: &str) -> bool {
    !name.starts_with('_')
}

fn is_test_method(name: &str) -> bool {
    name.starts_with("test_")
        || name.starts_with("solo_test_")
        || matches!(name, "setUp" | "tearDown" | "setUpClass" | "tearDownClass")
}

fn is_object(ty: DartType<'_>) -> bool {
    let DartType::Named(named) = ty else {
        return false;
    };
    let mut segments = named.segments();
    segments.next().is_some_and(|only| only.name() == "Object") && segments.next().is_none()
}

fn constructor_key<'a>(constructor: ConstructorDecl<'a>) -> &'a str {
    constructor
        .constructor_name()
        .map_or("new", |name| name.name())
}

fn annotated_start<'a>(annotations: AstChildren<'a, Annotation<'a>>, start: usize) -> usize {
    annotations
        .map(|annotation| annotation.span().start)
        .fold(start, usize::min)
}

/// Start of the doc comment block immediately preceding `start`, or `start`.
/// The empty block comment `/**/` is not a doc comment.
// Unlike documentation_start/doc_blocks, attachment here includes all adjacent
// documentation groups and stops at ordinary comments.
fn doc_start(source: &str, tree: &SyntaxTree, start: usize) -> usize {
    let mut start = start.min(source.len());
    for token in
        crate::lint::tokens::preceding_trivia(tree.tokens_in(source, Span::new(0, start)), start)
    {
        match token.kind() {
            falcon_syntax::TokenKind::Whitespace | falcon_syntax::TokenKind::Newline => {}
            falcon_syntax::TokenKind::DocComment => {
                let offset = token.offset();
                if token.text().starts_with("///") {
                    let line_start = source[..offset]
                        .rfind('\n')
                        .map_or(0, |newline| newline + 1);
                    if !source[line_start..offset].trim().is_empty() {
                        break;
                    }
                    start = line_start;
                } else {
                    start = offset;
                }
            }
            _ => break,
        }
    }
    start
}

fn attached_doc_references<'a>(
    source: &'a str,
    tree: &'a SyntaxTree,
) -> impl Iterator<Item = crate::lint::doc_comments::Reference<'a>> + 'a {
    tree.doc_blocks(source).flat_map(move |block| {
        let start = doc_start(source, tree, block.next_token);
        crate::lint::doc_comments::references(&block)
            .0
            .into_iter()
            .filter(move |reference| reference.offset >= start)
    })
}

/// Resolution of an unqualified name in expression context.
enum Lexical {
    /// A member declared by the enclosing class-like declaration.
    Member(ContainerId),
    TopLevel,
    /// Imported, inherited, or otherwise unknown.
    Unknown,
}

#[derive(Default)]
struct ExpressionTypes<'a>(
    HashMap<Span, falcon_analyze::ResolvedType>,
    HashMap<(Span, &'a str), falcon_analyze::DeclarationIdentity>,
);

impl<'a> SemanticRuleVisitor<'a> for ExpressionTypes<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Field(field) = node
            && let Some(reference) = state.referenced_declaration(node, false)
                .or_else(|| state.referenced_declaration(node, true))
        {
            self.1.insert((field.object().span(), field.field().name()), reference.owner);
        }
        let receiver = match node {
            Expr::Field(field) => field.object(),
            Expr::Index(index) => index.object(),
            Expr::Cascade(cascade) => cascade.object(),
            Expr::Binary(binary) => binary.left(),
            Expr::Unary(unary) => unary.operand(),
            Expr::PostfixIncDec(postfix) => postfix.operand(),
            Expr::Assign(assign) => assign.target(),
            _ => return,
        };
        self.0
            .entry(receiver.span())
            .or_insert_with(|| state.infer(receiver));
    }
}

/// Records, for each declaration being walked, the declarations it references.
struct RefWalker<'t, 'a> {
    tables: &'t Tables<'a>,
    unit: usize,
    source: &'t str,
    tree: &'t SyntaxTree,
    /// Identifier expressions resolving to locals.
    bound: &'t HashSet<usize>,
    deps: &'t mut Vec<Vec<DeclId>>,
    /// Declarations whose subtree is being walked; references count for all.
    stack: Vec<DeclId>,
    container: Option<ContainerId>,
    type_argument_depth: usize,
    /// Names of lexically visible type parameters, as spans in this unit.
    type_parameters: Vec<Span>,
    external_depth: usize,
    constant_pattern_depth: usize,
    semantic: Option<(
        &'t SemanticState<'a>,
        &'t ExpressionTypes<'a>,
    )>,
}

impl<'t, 'a> RefWalker<'t, 'a> {
    fn new(
        tables: &'t Tables<'a>,
        unit: usize,
        parsed: &'t falcon_analyze::ParsedFile,
        bound: &'t HashSet<usize>,
        deps: &'t mut Vec<Vec<DeclId>>,
    ) -> Self {
        Self {
            tables,
            unit,
            source: &parsed.source,
            tree: &parsed.tree,
            bound,
            deps,
            stack: Vec::new(),
            container: None,
            type_argument_depth: 0,
            type_parameters: Vec::new(),
            external_depth: 0,
            constant_pattern_depth: 0,
            semantic: None,
        }
    }

    fn push_type_parameters(&mut self, parameters: AstChildren<'a, TypeParam<'a>>) -> usize {
        let depth = self.type_parameters.len();
        self.type_parameters
            .extend(parameters.map(|parameter| parameter.name().span()));
        depth
    }

    fn id_at(&self, start: usize) -> Option<DeclId> {
        self.tables.at.get(&(self.unit, start)).copied()
    }

    fn mark(&mut self, target: DeclId) {
        for &source in &self.stack {
            self.deps[source].push(target);
        }
    }

    fn mark_top(&mut self, name: &str) {
        if self
            .type_parameters
            .iter()
            .any(|span| self.source.get(span.start..span.end) == Some(name))
        {
            return;
        }
        let tables = self.tables;
        for &id in tables.top.get(name).into_iter().flatten() {
            self.mark(id);
        }
    }

    fn mark_container(&mut self, container: ContainerId) {
        self.mark(self.tables.containers[container].decl);
    }

    fn mark_members(&mut self, container: ContainerId, name: &str) {
        let tables = self.tables;
        for &id in tables.containers[container]
            .members
            .get(name)
            .into_iter()
            .flatten()
        {
            self.mark(id);
        }
        self.mark_container(container);
    }

    fn mark_constructor(&mut self, container: ContainerId, name: &str) {
        if let Some(&id) = self.tables.containers[container]
            .candidate_constructors
            .get(name)
        {
            self.mark(id);
        }
        self.mark_container(container);
    }

    /// `Container.name`: a static member or a constructor.
    fn mark_static(&mut self, container: ContainerId, name: &str) {
        self.mark_members(container, name);
        self.mark_constructor(container, name);
    }

    /// A member access on a receiver of unknown type. The member's container
    /// is reached too when the receiver may have its type: always for an
    /// extension, otherwise only when the container's name occurs outside its
    /// own declaration (no expression can have a type that is never named).
    fn mark_any_member(&mut self, name: &str) {
        let tables = self.tables;
        for &id in tables.members_named.get(name).into_iter().flatten() {
            if let Some(container) = tables.decls[id].container
                && (tables.containers[container].kind == ContainerKind::Extension
                    || tables.named_elsewhere[container])
            {
                self.mark(id);
                self.mark_container(container);
            }
        }
    }

    /// A member access on an instance of `container`: the member it declares
    /// or inherits from library supertypes, else any same-named member.
    fn mark_member_of(&mut self, container: ContainerId, name: &str) {
        if !self.mark_in_hierarchy(container, name) {
            self.mark_any_member(name);
        }
    }

    /// Mark the members named `name` that `container` declares or inherits
    /// from library supertypes; whether any declaration was found.
    fn mark_in_hierarchy(&mut self, container: ContainerId, name: &str) -> bool {
        let tables = self.tables;
        let mut visited = HashSet::new();
        let mut pending = vec![container];
        let mut found = false;
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            let declaration = &tables.containers[current];
            if declaration.declared.contains(name) {
                self.mark_members(current, name);
                found = true;
                continue;
            }
            pending.extend(
                declaration
                    .supertypes
                    .iter()
                    .filter_map(|&supertype| tables.library_container(supertype)),
            );
        }
        found
    }

    fn lexical(&self, name: &str) -> Lexical {
        if let Some(container) = self.container
            && self.tables.containers[container].declared.contains(name)
        {
            return Lexical::Member(container);
        }
        if self.tables.top.contains_key(name) {
            Lexical::TopLevel
        } else {
            Lexical::Unknown
        }
    }

    /// An unqualified identifier that is not a local.
    fn reference(&mut self, name: &str) {
        match self.lexical(name) {
            Lexical::Member(container) => self.mark_members(container, name),
            Lexical::TopLevel => {
                self.mark_top(name);
                // An inherited member of the same name may be what the
                // identifier denotes; counting it too only adds reachability.
                if let Some(container) = self.container {
                    self.mark_in_hierarchy(container, name);
                }
            }
            Lexical::Unknown if self.container.is_some() => self.mark_any_member(name),
            Lexical::Unknown => {}
        }
    }

    /// The library container an unqualified, non-local identifier names.
    fn named_container(&self, identifier: Identifier<'a>) -> Option<ContainerId> {
        if self.bound.contains(&identifier.span().start) {
            return None;
        }
        match self.lexical(identifier.name()) {
            Lexical::TopLevel => self.tables.container_for(identifier.name()),
            Lexical::Member(_) | Lexical::Unknown => None,
        }
    }

    /// A constructor invocation or tear-off of `Type[.name]`.
    fn constructor_reference(&mut self, type_name: &str, constructor: &str) {
        if self.constant_pattern_depth > 0 {
            return;
        }
        let Some(container) = self.tables.container_for(type_name) else {
            return;
        };
        self.mark_top(type_name);
        self.mark_constructor(container, constructor);
    }

    /// The constructor a written `Type`, `Type.name`, or `prefix.Type` names.
    fn constructor_target(
        &self,
        mut segments: impl Iterator<Item = Identifier<'a>>,
        constructor: Option<Identifier<'a>>,
    ) -> Option<(&'a str, &'a str)> {
        let type_name = segments.next()?;
        match (segments.next(), segments.next()) {
            (None, _) => Some((
                type_name.name(),
                constructor.map_or("new", |name| name.name()),
            )),
            (Some(name), None)
                if constructor.is_none()
                    && self.tables.container_for(type_name.name()).is_some() =>
            {
                Some((type_name.name(), name.name()))
            }
            _ => None,
        }
    }

    /// The class-like declaration an expression is an instance of, when it is
    /// syntactically evident.
    fn receiver_container(&self, receiver: Expr<'a>) -> Option<ContainerId> {
        match receiver.unparenthesized() {
            Expr::This(_) => self.container.filter(|&container| {
                self.tables.containers[container].kind != ContainerKind::Extension
            }),
            Expr::Call(call) => match call.callee().unparenthesized() {
                Expr::Ident(type_name) => self.named_container(type_name),
                Expr::Field(field) => match field.object().unparenthesized() {
                    Expr::Ident(type_name) => {
                        self.named_container(type_name).filter(|&container| {
                            self.tables.containers[container]
                                .constructors
                                .contains(field.field().name())
                        })
                    }
                    _ => None,
                },
                _ => None,
            },
            Expr::New(new) => {
                let DartType::Named(named) = new.dart_type() else {
                    return None;
                };
                self.constructor_target(named.segments(), new.constructor_name())
                    .and_then(|(type_name, _)| self.tables.container_for(type_name))
            }
            _ => None,
        }
    }

    /// `receiver.name` where the receiver is not a type name.
    fn instance_member(&mut self, receiver: Expr<'a>, name: &str) {
        if self.resolved_instance_member(receiver, name) {
            return;
        }
        match self.receiver_container(receiver) {
            Some(container) => self.mark_member_of(container, name),
            None => self.mark_any_member(name),
        }
    }

    fn resolved_container(
        &self,
        identity: &falcon_analyze::DeclarationIdentity,
    ) -> Option<ContainerId> {
        self.tables.container_identities.get(identity).copied()
    }

    fn resolved_instance_member(&mut self, receiver: Expr<'a>, name: &str) -> bool {
        let Some((state, types)) = self.semantic else {
            return false;
        };
        let Some(ty) = types.0.get(&receiver.span()) else {
            return false;
        };
        if matches!(
            ty,
            falcon_analyze::ResolvedType::Unknown | falcon_analyze::ResolvedType::Dynamic
        ) {
            return false;
        }
        if let falcon_analyze::ResolvedType::Interface { identity, .. } = ty
            && let Some(container) = self.resolved_container(identity)
            && self.mark_in_hierarchy(container, name)
        {
            return true;
        }
        if let Some(owner) = types.1.get(&(receiver.span(), name)).cloned()
            .or_else(|| state.referenced_member_owner(ty, name, false))
            .or_else(|| state.referenced_member_owner(ty, name, true))
        {
            if let Some(container) = self.resolved_container(&owner) {
                self.mark_members(container, name);
            }
            return true;
        }
        let Some(on_types) = state
            .signatures
            .applicable_extension_on_types(ty, name, &state.model)
        else {
            return false;
        };
        let tables = self.tables;
        for (container, on_type) in &tables.extension_on_types {
            if on_types.contains(&on_type) {
                self.mark_members(*container, name);
            }
        }
        true
    }

    /// `object.name`: a static member or constructor when `object` names a
    /// library type, an instance member otherwise.
    fn member_access(&mut self, object: Expr<'a>, name: &str) {
        if let Expr::Ident(type_name) = object.unparenthesized()
            && let Some(container) = self.named_container(type_name)
        {
            self.mark_static(container, name);
            return;
        }
        self.instance_member(object, name);
    }

    fn superclass_constructor(&mut self, container: ContainerId, name: &str) {
        let tables = self.tables;
        if let Some(superclass) = tables.containers[container]
            .superclass
            .and_then(|superclass| tables.library_container(superclass))
        {
            self.mark_constructor(superclass, name);
        }
    }

    fn visit_type_arguments(&mut self, arguments: impl IntoIterator<Item = DartType<'a>>) {
        self.type_argument_depth += 1;
        for argument in arguments {
            self.visit_dart_type(argument);
        }
        self.type_argument_depth -= 1;
    }

    fn visit_external_type(&mut self, ty: Option<DartType<'a>>, is_external: bool) {
        if let Some(ty) = ty
            && is_external
        {
            self.external_depth += 1;
            self.visit_dart_type(ty);
            self.external_depth -= 1;
        }
    }

    /// Walk variable declarators: annotations and the declared type belong to
    /// every declarator, each initializer only to its own.
    fn visit_declarators(
        &mut self,
        annotations: AstChildren<'a, Annotation<'a>>,
        ty: Option<DartType<'a>>,
        is_external: bool,
        declarators: AstChildren<'a, VarDeclarator<'a>>,
    ) {
        let ids: Vec<DeclId> = declarators
            .clone()
            .filter_map(|declarator| self.id_at(declarator.span().start))
            .collect();
        let depth = self.stack.len();
        self.stack.extend(ids);
        for annotation in annotations {
            self.visit_annotation(annotation);
        }
        if let Some(ty) = ty {
            self.visit_dart_type(ty);
        }
        self.visit_external_type(ty, is_external);
        self.stack.truncate(depth);
        for declarator in declarators {
            let id = self.id_at(declarator.span().start);
            self.stack.extend(id);
            if let Some(initializer) = declarator.initializer() {
                self.visit_expr(initializer);
            }
            self.stack.truncate(depth);
        }
    }

    /// Container-level references: named supertypes, the implicit super
    /// constructor call, reflective-test instantiation, and `toJson`.
    fn container_references(&mut self, declaration: TopLevelDecl<'a>, container: ContainerId) {
        let tables = self.tables;
        let members = tables.containers[container].members_source.clone();
        let annotations = match declaration {
            TopLevelDecl::Class(x) => Some(x.annotations()),
            TopLevelDecl::MixinClass(x) => Some(x.annotations()),
            _ => None,
        };
        if matches!(
            declaration,
            TopLevelDecl::Class(_) | TopLevelDecl::MixinClass(_) | TopLevelDecl::ExtensionType(_)
        ) {
            for &supertype in &tables.containers[container].supertypes {
                if let DartType::Named(named) = supertype {
                    let mut segments = named.segments();
                    if let Some(name) = segments.next()
                        && segments.next().is_none()
                    {
                        self.mark_top(name.name());
                    }
                }
            }
        }
        let kind = tables.containers[container].kind;
        if kind == ContainerKind::Class {
            if !members
                .clone()
                .any(|member| matches!(member, ClassMember::Constructor(_)))
            {
                self.superclass_constructor(container, "new");
            }
            if annotations.into_iter().flatten().any(|annotation| {
                annotation
                    .name()
                    .last()
                    .is_some_and(|name| name.name() == "reflectiveTest")
            }) {
                self.mark_constructor(container, "new");
            }
        }
        for member in members {
            if let ClassMember::Method(method) = member
                && method.name().name() == "toJson"
                && !method.is_static()
                && let Some(id) = self.id_at(member.span().start)
            {
                self.mark(id);
            }
        }
    }

    fn constructor_references(&mut self, constructor: ConstructorDecl<'a>) {
        let Some(container) = self.container else {
            return;
        };
        let mut has_super_call = false;
        for initializer in constructor.initializers() {
            match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    has_super_call = true;
                    let name = call.call_name().map_or("new", |name| name.name());
                    self.superclass_constructor(container, name);
                }
                ConstructorInitializer::ThisCall(call) => {
                    let name = call.call_name().map_or("new", |name| name.name());
                    self.mark_constructor(container, name);
                }
                ConstructorInitializer::FieldInit(field) => {
                    self.mark_members(container, field.field().name());
                }
                ConstructorInitializer::Assert(_) => {}
            }
        }
        if !has_super_call && self.tables.containers[container].kind == ContainerKind::Class {
            self.superclass_constructor(container, "new");
        }
        if let Some(redirect) = constructor.redirect()
            && let DartType::Named(named) = redirect.type_()
            && let Some((type_name, name)) =
                self.constructor_target(named.segments(), redirect.constructor_name())
        {
            self.constructor_reference(type_name, name);
        }
    }

    /// Attribute every doc comment reference of this unit to the declarations
    /// whose doc comment or body contains it.
    fn doc_references(&mut self) {
        let tables = self.tables;
        let unit_decls: Vec<DeclId> = (0..tables.decls.len())
            .filter(|&id| tables.decls[id].unit == self.unit)
            .collect();
        for reference in attached_doc_references(self.source, self.tree) {
            let Some(parsed) = reference.parse() else {
                continue;
            };
            self.stack.extend(unit_decls.iter().copied().filter(|&id| {
                let decl = &tables.decls[id];
                decl.doc_start <= reference.offset && reference.offset < decl.end
            }));
            let owners = &self.stack;
            let Some(&innermost) = owners.iter().max_by_key(|&&id| tables.decls[id].doc_start)
            else {
                continue;
            };
            self.container = tables.decls[innermost].scope;
            let mut names = parsed.names.iter().map(|name| name.text);
            match (names.next(), names.next(), names.next()) {
                (Some(name), None, _) if parsed.new_keyword => self.constructor_reference(name, "new"),
                (Some(name), None, _) => self.reference(name),
                (Some(type_name), Some(member), None) => {
                    self.reference(type_name);
                    match tables.container_for(type_name) {
                        Some(container) if matches!(self.lexical(type_name), Lexical::TopLevel) => {
                            self.mark_static(container, member)
                        }
                        _ => self.mark_any_member(member),
                    }
                }
                (_, _, Some(member)) => self.mark_any_member(member),
                _ => {}
            }
            self.stack.clear();
        }
        self.container = None;
    }
}


fn relational_operator(op: RelationalPatternOp) -> Option<&'static str> {
    match op {
        RelationalPatternOp::Lt => Some("<"),
        RelationalPatternOp::Gt => Some(">"),
        RelationalPatternOp::LtEq => Some("<="),
        RelationalPatternOp::GtEq => Some(">="),
        RelationalPatternOp::Eq | RelationalPatternOp::NotEq => None,
    }
}

impl<'a> Visitor<'a> for RefWalker<'_, 'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::Variable(variable) = node {
            self.visit_declarators(
                variable.annotations(),
                variable.var_type(),
                variable.is_external(),
                variable.declarators(),
            );
            return;
        }
        let parameters = node.type_params();
        let parameter_depth = self.push_type_parameters(parameters);
        let start = node.span().start;
        let id = self.id_at(start);
        let container = self.tables.container_at.get(&(self.unit, start)).copied();
        self.stack.extend(id);
        self.container = container;
        if let Some(container) = container {
            self.container_references(node, container);
        }
        walk_top_level_decl(self, node);
        self.type_parameters.truncate(parameter_depth);
        self.container = None;
        self.stack.clear();
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        let depth = self.stack.len();
        let parameter_depth = match node {
            ClassMember::Method(method) => self.push_type_parameters(method.type_params()),
            _ => self.type_parameters.len(),
        };
        match node {
            ClassMember::Field(field) => {
                self.visit_declarators(
                    field.annotations(),
                    field.field_type(),
                    field.is_external(),
                    field.declarators(),
                );
                return;
            }
            ClassMember::Constructor(constructor) => {
                self.stack.extend(self.id_at(constructor.span().start));
                self.constructor_references(constructor);
            }
            ClassMember::Method(method) => {
                self.stack.extend(self.id_at(method.span().start));
                self.visit_external_type(method.return_type(), method.is_external());
            }
            ClassMember::Getter(getter) => {
                self.stack.extend(self.id_at(getter.span().start));
                self.visit_external_type(getter.return_type(), getter.is_external());
            }
            ClassMember::Operator(operator) => {
                self.stack.extend(self.id_at(operator.span().start));
                self.visit_external_type(operator.return_type(), operator.is_external());
            }
            ClassMember::Setter(setter) => self.stack.extend(self.id_at(setter.span().start)),
            ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
        }
        walk_class_member(self, node);
        self.stack.truncate(depth);
        self.type_parameters.truncate(parameter_depth);
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        let invoked = node.args().is_some();
        let mut names = node.name();
        match (names.next(), names.next(), names.next()) {
            (Some(name), None, _) if invoked => {
                let constructor = node.constructor_name().map_or("new", |name| name.name());
                if self.tables.container_for(name.name()).is_some() {
                    self.constructor_reference(name.name(), constructor);
                } else {
                    self.mark_top(name.name());
                }
            }
            (Some(name), None, _) => self.mark_top(name.name()),
            (Some(type_name), Some(member), None) => {
                if let Some(container) = self.tables.container_for(type_name.name()) {
                    self.mark_top(type_name.name());
                    self.mark_static(container, member.name());
                }
            }
            _ => {}
        }
        self.type_argument_depth += 1;
        walk_annotation(self, node);
        self.type_argument_depth -= 1;
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) {
        let DartType::Named(named) = node else {
            let depth = match node {
                DartType::Function(function) => self.push_type_parameters(function.type_params()),
                _ => self.type_parameters.len(),
            };
            walk_dart_type(self, node);
            self.type_parameters.truncate(depth);
            return;
        };
        let mut segments = named.segments();
        if let (Some(segment), None) = (segments.next(), segments.next()) {
            let name = segment.name();
            let counts = self.type_argument_depth > 0
                || self.external_depth > 0
                || self.tables.alias_named.contains_key(name)
                || self
                    .tables
                    .container_named
                    .get(name)
                    .is_some_and(|&container| {
                        self.tables.containers[container].kind == ContainerKind::ExtensionType
                    });
            if counts {
                self.mark_top(name);
            }
        }
        self.visit_type_arguments(named.type_args());
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        let parameter_depth = match node {
            Stmt::LocalFunc(function) => self.push_type_parameters(function.type_params()),
            _ => self.type_parameters.len(),
        };
        if let Stmt::For(statement) = node
            && let Some(ForInit::ForIn(init)) = statement.header().init()
            && !init.declares_variable()
        {
            self.reference(init.name().name());
        }
        walk_stmt(self, node);
        self.type_parameters.truncate(parameter_depth);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        let parameter_depth = match node {
            Expr::FuncExpr(function) => self.push_type_parameters(function.type_params()),
            _ => self.type_parameters.len(),
        };
        match node {
            Expr::Ident(identifier) if !self.bound.contains(&identifier.span().start) => {
                self.reference(identifier.name());
            }
            Expr::Field(field) => self.member_access(field.object(), field.field().name()),
            Expr::Call(call) => {
                self.visit_type_arguments(call.type_args());
                if let Expr::Ident(type_name) = call.callee().unparenthesized()
                    && self.named_container(type_name).is_some()
                {
                    self.constructor_reference(type_name.name(), "new");
                }
            }
            Expr::New(new) => {
                if let DartType::Named(named) = new.dart_type()
                    && let Some((type_name, name)) =
                        self.constructor_target(named.segments(), new.constructor_name())
                {
                    self.constructor_reference(type_name, name);
                }
            }
            Expr::DotShorthand(shorthand) => {
                let name = shorthand.name().name();
                self.mark_any_member(name);
                let tables = self.tables;
                for (container, declaration) in tables.containers.iter().enumerate() {
                    if declaration.constructors.contains(name) {
                        self.mark_constructor(container, name);
                    }
                }
            }
            Expr::Binary(binary) => {
                if let Some(operator) = binary.op().operator_name().filter(|_| !matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq)) {
                    self.instance_member(binary.left(), operator);
                }
            }
            Expr::Unary(unary) => {
                let operator = match unary.op() {
                    UnaryOp::Minus => Some("unary-"),
                    UnaryOp::Tilde => Some("~"),
                    UnaryOp::PlusPlus => Some("+"),
                    UnaryOp::MinusMinus => Some("-"),
                    UnaryOp::Bang => None,
                };
                if let Some(operator) = operator {
                    self.instance_member(unary.operand(), operator);
                }
            }
            Expr::PostfixIncDec(postfix) => self.instance_member(
                postfix.operand(),
                match postfix.op() {
                    PostfixIncDec::Increment => "+",
                    PostfixIncDec::Decrement => "-",
                },
            ),
            Expr::Assign(assign) => {
                if let Some(operator) = assign.op().operator_name() {
                    self.instance_member(assign.target(), operator);
                }
                if let Expr::Index(index) = assign.target().unparenthesized() {
                    self.instance_member(index.object(), "[]=");
                }
            }
            Expr::Index(index) => self.instance_member(index.object(), "[]"),
            Expr::Cascade(cascade) => {
                let object = cascade.object();
                for operation in cascade.sections().flat_map(|section| section.ops()) {
                    match operation {
                        CascadeOp::NullAssert(_) => {}
                        CascadeOp::Field(field) => {
                            self.instance_member(object, field.name().name())
                        }
                        CascadeOp::Call(call) => {
                            self.visit_type_arguments(call.type_args());
                            self.instance_member(object, call.name().name());
                        }
                        // Like ordinary function-expression invocations, an implicit
                        // `call` names no element, so it references no member.
                        CascadeOp::Invoke(invoke) => self.visit_type_arguments(invoke.type_args()),
                        CascadeOp::Index(_) => self.mark_any_member("[]"),
                        CascadeOp::Assign(assign) => {
                            // A named cascade assignment writes the receiver member.
                            if let Expr::Ident(name) = assign.target().unparenthesized() {
                                self.instance_member(object, name.name());
                            }
                            if let Some(operator) = assign.op().operator_name() {
                                self.mark_any_member(operator);
                            }
                        }
                        CascadeOp::IndexAssign(assign) => {
                            self.instance_member(object, "[]=");
                            if assign.op() != AssignOp::Eq {
                                self.instance_member(object, "[]");
                            }
                            if let Some(operator) = assign.op().operator_name() {
                                self.mark_any_member(operator);
                            }
                        }
                    }
                }
            }
            Expr::List(list) => self.visit_type_arguments(list.type_arg()),
            Expr::Set(set) => self.visit_type_arguments(set.type_arg()),
            Expr::Map(map) => self.visit_type_arguments(map.type_args()),
            Expr::GenericInstantiation(generic) => self.visit_type_arguments(generic.type_args()),
            _ => {}
        }
        walk_expr(self, node);
        self.type_parameters.truncate(parameter_depth);
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) {
        match node {
            Pattern::Object(object) => {
                let tables = self.tables;
                let container = match object.type_() {
                    named @ DartType::Named(_) => tables.library_container(named),
                    _ => None,
                };
                for field in object.fields() {
                    match container {
                        Some(container) => self.mark_member_of(container, field.name().name()),
                        None => self.mark_any_member(field.name().name()),
                    }
                }
            }
            Pattern::Const(constant) => {
                if let ConstPatternValue::Name(mut segments) = constant.value() {
                    match (segments.next(), segments.next(), segments.next()) {
                        (Some(name), None, _) => self.reference(name.name()),
                        (Some(type_name), Some(member), None) => {
                            self.reference(type_name.name());
                            match self.named_container(type_name) {
                                Some(container) => self.mark_static(container, member.name()),
                                None => self.mark_any_member(member.name()),
                            }
                        }
                        (_, _, Some(member)) => {
                            self.mark_any_member(segments.last().unwrap_or(member).name());
                        }
                        _ => {}
                    }
                }
                self.constant_pattern_depth += 1;
                walk_pattern(self, node);
                self.constant_pattern_depth -= 1;
                return;
            }
            Pattern::Relational(relational) => {
                if let Some(operator) = relational_operator(relational.op()) {
                    self.mark_any_member(operator);
                }
            }
            Pattern::List(list) => self.visit_type_arguments(list.type_arg()),
            Pattern::Map(map) => self.visit_type_arguments(map.type_args()),
            _ => {}
        }
        walk_pattern(self, node);
    }
}

#[cfg(test)]
mod tests {
    use falcon_analyze::AnalyzeContext;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;

    use super::{UnreachableFromMain, attached_doc_references, doc_start};
    use crate::lint::doc_comments::references;

    fn parse(source: &str) -> std::sync::Arc<falcon_analyze::ParsedFile> {
        std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: falcon_dart_parser::parse(source).tree,
        })
    }

    fn lint(source: &str) -> Vec<String> {
        let parsed = parse(source);
        let program = parsed.program();
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(std::path::Path::new("main.dart"), &config, &parsed);
        UnreachableFromMain.analyze(UnreachableFromMain::NAME, program, &ctx)
            .into_iter()
            .map(|diagnostic| source[diagnostic.span.start..diagnostic.span.end].to_string())
            .collect()
    }

    #[test]
    fn doc_comment_references_parse_names_members_and_constructors() {
        let source = "/// See [A], [B.c], [new D] and [text].\nvoid f() {}\n/** [E] */\nvoid g() {}\n";
        let markdown_link = "/// [text](url)\n";
        let markdown_parsed = parse(markdown_link);
        assert!(
            markdown_parsed
                .tree
                .doc_blocks(markdown_link)
                .flat_map(|block| references(&block).0)
                .next()
                .is_none()
        );
        let parsed = parse(source);
        let references: Vec<(bool, Vec<&str>)> = parsed
            .tree
            .doc_blocks(source)
            .flat_map(|block| references(&block).0)
            .filter_map(|reference| reference.parse())
            .map(|reference| {
                (
                    reference.new_keyword,
                    reference.names.into_iter().map(|name| name.text).collect(),
                )
            })
            .collect();
        assert_eq!(
            references,
            [
                (false, vec!["A"]),
                (false, vec!["B", "c"]),
                (true, vec!["D"]),
                (false, vec!["text"]),
                (false, vec!["E"]),
            ]
        );
    }

    #[test]
    fn doc_start_covers_line_and_block_doc_comments() {
        let source = "int a = 0;\n/// One.\n/// Two.\nvoid f() {}\n/** Block. */\nvoid g() {}\n";
        let parsed = parse(source);
        assert_eq!(
            doc_start(source, &parsed.tree, source.find("void f").unwrap()),
            source.find("/// One").unwrap()
        );
        assert_eq!(
            doc_start(source, &parsed.tree, source.find("void g").unwrap()),
            source.find("/**").unwrap()
        );
        assert_eq!(
            doc_start(source, &parsed.tree, source.find("int a").unwrap()),
            0
        );
    }

    #[test]
    fn empty_block_comment_is_not_a_doc_comment() {
        let source = "/**/\nvoid main() {}\n";
        let parsed = parse(source);
        assert_eq!(
            doc_start(source, &parsed.tree, source.find("void").unwrap()),
            5
        );
        assert!(lint(source).is_empty());
        assert_eq!(lint("/**/\nvoid main() {}\nclass C {}\n"), ["C"]);
    }

    #[test]
    fn doc_start_preserves_adjacent_groups_and_plain_comment_boundary() {
        let source = "/// First.\n/** Second. */\n  /// Third.\nvoid f() {}\n\
                      /// Detached.\n// Boundary.\n  /// Attached.\nvoid g() {}\n";
        let parsed = parse(source);
        assert_eq!(
            doc_start(source, &parsed.tree, source.find("void f").unwrap()),
            0
        );
        assert_eq!(
            doc_start(source, &parsed.tree, source.find("void g").unwrap()),
            source.find("  /// Attached").unwrap()
        );
    }

    #[test]
    fn doc_references_only_read_retained_documentation_comments() {
        let source = "const text = '/// [StringOnly]';\n\
                      // [PlainOnly]\n/**/\n\
                      /// [Attached]\nvoid f() {}\n";
        let parsed = parse(source);
        let references: Vec<_> = parsed
            .tree
            .doc_blocks(source)
            .flat_map(|block| references(&block).0)
            .collect();
        assert_eq!(references.len(), 1);
        assert_eq!(
            references[0]
                .parse()
                .unwrap()
                .names
                .into_iter()
                .map(|name| name.text)
                .collect::<Vec<_>>(),
            ["Attached"]
        );
        assert_eq!(references[0].offset, source.find("[Attached]").unwrap() + 1);
    }

    #[test]
    fn attached_doc_references_reach_declarations() {
        assert!(lint("/// See [Helper].\nvoid main() {}\nclass Helper {}\n").is_empty());
        assert_eq!(
            lint("/// See [Helper].\nvoid unused() {}\nvoid main() {}\nclass Helper {}\n"),
            ["unused", "Helper"]
        );
    }

    #[test]
    fn inline_line_documentation_does_not_attach_to_next_declaration() {
        let source = "void first() {} /// [Helper]\nvoid main() {}\nclass Helper {}\n";
        let parsed = parse(source);
        let start = source.find("void main").unwrap();
        assert_eq!(doc_start(source, &parsed.tree, start), start);
        assert!(
            attached_doc_references(source, &parsed.tree)
                .next()
                .is_none()
        );
    }

    #[test]
    fn doc_references_include_nested_interpolation_comments() {
        let source = "void main() { print('''${() {\n\
                      /// [Helper]\nreturn 0;\n}()}'''); }\nclass Helper {}\n";
        let parsed = parse(source);
        let references: Vec<_> = parsed
            .tree
            .doc_blocks(source)
            .flat_map(|block| references(&block).0)
            .collect();
        assert_eq!(references.len(), 1);
        assert_eq!(
            references[0]
                .parse()
                .unwrap()
                .names
                .into_iter()
                .map(|name| name.text)
                .collect::<Vec<_>>(),
            ["Helper"]
        );
        assert_eq!(references[0].offset, source.find("[Helper]").unwrap() + 1);
        assert!(lint(source).is_empty());
    }

    #[test]
    fn members_of_unknown_supertypes_are_never_candidates() {
        let source = "void main() { W(); }\nclass W extends External { void build() {} }\n";
        assert!(lint(source).is_empty());
    }

    #[test]
    fn local_and_member_shadowing_do_not_reach_top_level() {
        let source = "void main() { var f = 1; print(f); C().m(); }\n\
                      class C { int f() => 0; void m() => f(); }\n\
                      int f = 0;\n";
        assert_eq!(lint(source), ["f"]);
    }

    #[test]
    fn sibling_semantics_use_their_own_parsed_file_without_shared_signatures() {
        use falcon_analyze::{group_libraries, library_unit};
        use std::path::Path;

        let path = Path::new("/project/main.dart");
        let part_path = Path::new("/project/part.dart");
        let parsed =
            parse("part 'part.dart';\nvoid main() => run();\nclass _C { void used() {} }\n");
        let part = parse(
            "part of 'main.dart';\nvoid run() {\n\
             final receiver = _C();\nreceiver.used();\n}\n",
        );
        let identities = crate::lint::test_support::identities(&[(path, &parsed), (part_path, &part)]);
        let grouping = group_libraries(&[
            (path.to_path_buf(), parsed.program()),
            (part_path.to_path_buf(), part.program()),
        ]);
        let library = library_unit(&grouping, &[&parsed, &part], 0);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&identities)
            .with_library(&library);
        assert!(
            UnreachableFromMain
                .analyze(UnreachableFromMain::NAME, parsed.program(), &ctx)
                .is_empty()
        );
    }

    #[test]
    fn library_without_entry_point_is_not_analyzed() {
        assert!(lint("class C {}\nvoid helper() {}\n").is_empty());
    }

    #[test]
    fn part_without_library_context_is_not_analyzed() {
        assert!(lint("part of 'lib.dart';\nvoid main() {}\nclass C {}\n").is_empty());
    }
}
