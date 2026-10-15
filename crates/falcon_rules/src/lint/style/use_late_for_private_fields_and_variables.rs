//! Flags a private nullable field or top-level variable that is only ever
//! assigned non-null values and read through `!`, so it could be `late` with a
//! non-nullable type.
//!
//! When every read of a private variable is a null assertion and every write
//! stores a non-nullable value, the nullable type only exists to defer
//! initialization; `late` expresses that and moves the null check into the
//! runtime. Candidates are private (underscore) top-level variables and private
//! fields (instance or static), plus every field of a private extension; instance
//! fields of extension types, fields of classes with a `const` constructor, and
//! all enum members are skipped, as are `late`, `const`, `external`, and
//! `abstract` declarations. Any other use — a plain read, a `this._x`
//! initializing formal, a `: _x = ...` initializer, a compound or `??=`
//! assignment, or an assignment whose value is not proven non-nullable —
//! disqualifies the referenced declaration. Uses in every resolved library
//! part are checked together; unresolved parts prevent proving a candidate safe.
//!
//! ```dart
//! int? _i; // flagged
//! void f() {
//!   _i = 1;
//!   print(_i!);
//! }
//! ```
//!
//! Adopted from the official `use_late_for_private_fields_and_variables` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_field_decl, walk_local_var, walk_top_level_decl,
};

use crate::lint::declarations::declaration_key;
use crate::lint::doc_comments::{
    DocElement, DocLookup, DocResolver, DocScopes, doc_blocks, references,
};
use crate::lint::expressions::is_parenthesized;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::semantic_type_operations::nullability;

#[derive(Default)]
pub struct UseLateForPrivateFieldsAndVariables;

impl Rule for UseLateForPrivateFieldsAndVariables {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let has_part_directives =
            !program.part_directives.is_empty() || program.part_of_directive.is_some();
        match ctx.library {
            Some(library) if library.has_unresolved_parts() => return Vec::new(),
            None if has_part_directives => return Vec::new(),
            _ => {}
        }
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let candidates = collect_candidates(program, &state, ctx);
        if candidates.is_empty() {
            return Vec::new();
        }
        let names: HashSet<&str> = candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect();

        let mut scopes = DeclarationScopes::of(program);
        if let Some(library) = ctx.library {
            for sibling in library.siblings() {
                scopes.extend(DeclarationScopes::of(sibling));
            }
        }
        let owners: HashMap<DeclarationIdentity, usize> = scopes
            .types
            .iter()
            .filter_map(|(name, key)| {
                state
                    .model
                    .resolve_name(*name)
                    .map(|identity| (identity, *key))
            })
            .collect();
        let mut disqualified = HashSet::new();
        check_uses(
            program,
            ctx,
            state,
            &names,
            &scopes,
            &owners,
            &mut disqualified,
        );
        if let Some(library) = ctx.library {
            for (sibling, source) in library.siblings().iter().zip(library.sibling_sources()) {
                let sibling_ctx = AnalyzeContext {
                    types: ctx.types,
                    identities: ctx.identities,
                    signatures: ctx.signatures,
                    library: ctx.library,
                    environment: ctx.environment,
                    package_language_version: ctx.package_language_version,
                    ..AnalyzeContext::new(ctx.file_path, source, ctx.config)
                };
                if let Some(state) = SemanticState::new(sibling, &sibling_ctx) {
                    check_uses(
                        sibling,
                        &sibling_ctx,
                        state,
                        &names,
                        &scopes,
                        &owners,
                        &mut disqualified,
                    );
                }
            }
        }

        candidates
            .iter()
            .filter(|candidate| {
                let name = candidate.name.as_str();
                !disqualified.contains(&NullableAccess::Unresolved(name))
                    && !disqualified.contains(&NullableAccess::Resolved(name, candidate.owner))
            })
            .map(|candidate| {
                ctx.diagnostic(
                    "use-late-for-private-fields-and-variables",
                    "Use 'late' for private members with a non-nullable type.",
                    candidate.span,
                )
            })
            .collect()
    }
}

/// A resolved declaration, or a name whose owner cannot be determined.
#[derive(Hash, PartialEq, Eq)]
enum NullableAccess<'a> {
    Unresolved(&'a str),
    Resolved(&'a str, Option<usize>),
}

fn check_uses<'n>(
    program: &Program,
    ctx: &AnalyzeContext,
    state: SemanticState<'_>,
    names: &'n HashSet<&'n str>,
    scopes: &DeclarationScopes<'_>,
    owners: &HashMap<DeclarationIdentity, usize>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let doc_scopes = DocScopes::new(program, &state.model, ctx.source);
    let resolver = DocResolver::new(program, &state.model, &state.signatures, &doc_scopes, false);
    for block in doc_blocks(ctx.tokens(), ctx.source) {
        let Some(scope) = doc_scopes.scope_for(&block) else {
            continue;
        };
        for reference in references(ctx.source, &block).0 {
            let Some(parsed) = reference.parse() else {
                continue;
            };
            for (name, lookup) in parsed.names.iter().zip(resolver.resolve(&parsed, scope)) {
                let Some(name) = names.get(name.text) else {
                    continue;
                };
                match lookup {
                    DocLookup::Found(DocElement::Declaration { .. }) => {
                        disqualified.insert(NullableAccess::Resolved(name, None));
                    }
                    DocLookup::Found(DocElement::Member { owner, .. }) => {
                        if let Some(owner) = owners.get(&owner) {
                            disqualified.insert(NullableAccess::Resolved(name, Some(*owner)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let mut safe = SafeUses {
        ctx,
        names,
        owners,
        resolved_owners: HashMap::new(),
        spans: HashSet::new(),
    };
    visit_program(&mut safe, program, state);
    let mut mentions = Mentions {
        names,
        types: &scopes.types,
        enclosing: None,
        cascade_members: HashSet::new(),
        declarations: HashSet::new(),
        found: Vec::new(),
    };
    mentions.visit_program(program);
    for mention in &mentions.found {
        if mentions.declarations.contains(&mention.start) || safe.spans.contains(&mention.start) {
            continue;
        }
        match safe
            .resolved_owners
            .get(&mention.start)
            .copied()
            .or_else(|| scopes.owner(mention))
        {
            Some(owner) => {
                disqualified.insert(NullableAccess::Resolved(mention.name, owner));
            }
            None => {
                disqualified.insert(NullableAccess::Unresolved(mention.name));
            }
        }
    }
}

struct Candidate {
    name: String,
    span: Span,
    /// The declaring type ([`declaration_key`]); `None` for top-level variables.
    owner: Option<usize>,
}

fn collect_candidates(
    program: &Program,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for declaration in &program.declarations {
        let (type_params, members, all_fields, static_only) = match declaration {
            TopLevelDecl::Variable(variable) => {
                if !variable.is_late
                    && !variable.is_external
                    && variable.keyword != VarKeyword::Const
                {
                    let scope = TypeParameterScope::default();
                    for declarator in &variable.declarators {
                        if is_private(&declarator.name.name)
                            && potentially_nullable(
                                variable.var_type.as_ref(),
                                declarator,
                                &scope,
                                state,
                                ctx,
                            )
                        {
                            candidates.push(candidate(declarator, None));
                        }
                    }
                }
                continue;
            }
            TopLevelDecl::Class(class) => {
                if has_const_constructor(&class.members) {
                    continue;
                }
                (&class.type_params, &class.members, false, false)
            }
            TopLevelDecl::MixinClass(class) => {
                if has_const_constructor(&class.members) {
                    continue;
                }
                (&class.type_params, &class.members, false, false)
            }
            TopLevelDecl::Mixin(mixin) => (&mixin.type_params, &mixin.members, false, false),
            TopLevelDecl::Extension(extension) => {
                let private = extension
                    .name
                    .as_ref()
                    .is_none_or(|name| is_private(&name.name));
                (&extension.type_params, &extension.members, private, false)
            }
            TopLevelDecl::ExtensionType(extension) => {
                (&extension.type_params, &extension.members, false, true)
            }
            _ => continue,
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_params, &state.model);
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.is_late
                || field.is_external
                || field.is_abstract
                || field.keyword == VarKeyword::Const
                || (static_only && !field.is_static)
            {
                continue;
            }
            for declarator in &field.declarators {
                if (all_fields || is_private(&declarator.name.name))
                    && potentially_nullable(
                        field.field_type.as_ref(),
                        declarator,
                        &scope,
                        state,
                        ctx,
                    )
                {
                    candidates.push(candidate(declarator, Some(declaration_key(declaration))));
                }
            }
        }
    }
    candidates
}

fn candidate(declarator: &VarDeclarator, owner: Option<usize>) -> Candidate {
    Candidate {
        name: declarator.name.name.clone(),
        span: declarator.span,
        owner,
    }
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

fn has_const_constructor(members: &[ClassMember]) -> bool {
    members.iter().any(
        |member| matches!(member, ClassMember::Constructor(constructor) if constructor.is_const),
    )
}

/// Whether the declared type is proven not to be non-nullable (the analyzer's
/// `!isNonNullable`), including `dynamic` and potentially nullable type
/// parameters. An unresolved type is never a candidate.
fn potentially_nullable(
    declared: Option<&DartType>,
    declarator: &VarDeclarator,
    scope: &TypeParameterScope,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext,
) -> bool {
    let ty = match (
        declared,
        declarator.initializer.as_ref().map(Expr::unparenthesized),
    ) {
        (Some(ty), _) => state.model.resolve_type_in(ty, scope),
        // Without a type or initializer the variable is `dynamic`; a `null`
        // initializer also infers `dynamic`.
        (None, None | Some(Expr::NullLit { .. })) => ResolvedType::Dynamic,
        (None, Some(initializer)) => state.infer(initializer),
    };
    match &ty {
        ResolvedType::Dynamic => true,
        ResolvedType::TypeParameter {
            bound,
            nullable: false,
            ..
        } => {
            matches!(bound.as_ref(), ResolvedType::Dynamic)
                || nullability(bound, &state.signatures, ctx.types) == Truth::Yes
        }
        _ => nullability(&ty, &state.signatures, ctx.types) == Truth::Yes,
    }
}

/// Records the identifier spans of uses that keep a candidate lateable: a `!`
/// operand, or the target of a plain `=` of a proven non-nullable value.
struct SafeUses<'a> {
    ctx: &'a AnalyzeContext<'a>,
    names: &'a HashSet<&'a str>,
    owners: &'a HashMap<DeclarationIdentity, usize>,
    resolved_owners: HashMap<usize, Option<usize>>,
    spans: HashSet<usize>,
}

impl SafeUses<'_> {
    fn record(&mut self, expression: &Expr) {
        let identifier = match expression.unparenthesized() {
            Expr::Ident(identifier) => identifier,
            Expr::Field { field, .. } => field,
            _ => return,
        };
        if self.names.contains(identifier.name.as_str()) {
            self.spans.insert(identifier.span.start);
        }
    }

    /// The analyzer requires the identifier itself to be the `!` operand,
    /// not a parenthesized expression or a property's name.
    fn record_null_assert(&mut self, operand: &Expr) {
        if let Expr::Ident(identifier) = operand
            && self.names.contains(identifier.name.as_str())
            && !is_parenthesized(operand)
        {
            self.spans.insert(identifier.span.start);
        }
    }

    fn record_assignment(
        &mut self,
        target: &Expr,
        op: AssignOp,
        value: &Expr,
        state: &SemanticState<'_>,
    ) {
        if op == AssignOp::Eq
            && nullability(&state.infer(value), &state.signatures, self.ctx.types) == Truth::No
        {
            self.record(target);
        }
    }
}

impl SemanticRuleVisitor for SafeUses<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Field { object, field, .. } = node
            && self.names.contains(field.name.as_str())
        {
            let receiver = state.infer(object);
            let owner = state
                .signatures
                .resolved_readable_member_owner(&receiver, &field.name, &state.model)
                .or_else(|| {
                    state.signatures.resolved_writable_member_owner(
                        &receiver,
                        &field.name,
                        &state.model,
                    )
                });
            if let Some(key) = owner.as_ref().and_then(|owner| self.owners.get(owner)) {
                self.resolved_owners.insert(field.span.start, Some(*key));
            }
        } else if let Expr::Ident(identifier) = node
            && self.names.contains(identifier.name.as_str())
            && state
                .environment
                .binding_depth(&identifier.name)
                .is_some_and(|depth| depth > 0)
        {
            // A local binding is not the same element as a private field.
            self.spans.insert(identifier.span.start);
        }
        match node {
            Expr::NullAssert { operand, .. } => self.record_null_assert(operand),
            Expr::Assign {
                target, op, value, ..
            } => self.record_assignment(target, *op, value, state),
            Expr::Cascade { sections, .. } => {
                for section in sections {
                    if let [CascadeOp::Assign(target, op, value)] = section.ops.as_slice() {
                        self.record_assignment(target, *op, value, state);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The member names each type declaration of a file declares, and its
/// top-level names, for resolving mentions to their declaring type. Private
/// names never resolve to other libraries, so this file (plus its parts, which
/// are matched by name) holds every declaration a mention can denote.
struct DeclarationScopes<'p> {
    /// Type declarations by name.
    types: HashMap<&'p str, usize>,
    /// Names declared by each type declaration, by [`declaration_key`].
    members: HashMap<usize, HashSet<&'p str>>,
    top_level: HashSet<&'p str>,
}

impl<'p> DeclarationScopes<'p> {
    fn of(program: &'p Program) -> Self {
        let mut scopes = Self {
            types: HashMap::new(),
            members: HashMap::new(),
            top_level: HashSet::new(),
        };
        for declaration in &program.declarations {
            let (name, members, extra): (Option<&Identifier>, &[ClassMember], Vec<&str>) =
                match declaration {
                    TopLevelDecl::Variable(variable) => {
                        scopes.top_level.extend(
                            variable
                                .declarators
                                .iter()
                                .map(|declarator| declarator.name.name.as_str()),
                        );
                        continue;
                    }
                    TopLevelDecl::Function(function) => {
                        scopes.top_level.insert(&function.name.name);
                        continue;
                    }
                    TopLevelDecl::ClassTypeAlias(alias) => {
                        scopes.top_level.insert(&alias.name.name);
                        continue;
                    }
                    TopLevelDecl::TypeAlias(alias) => {
                        scopes.top_level.insert(&alias.name.name);
                        continue;
                    }
                    TopLevelDecl::Error(_) => continue,
                    TopLevelDecl::Class(class) => (Some(&class.name), &class.members, Vec::new()),
                    TopLevelDecl::MixinClass(class) => {
                        (Some(&class.name), &class.members, Vec::new())
                    }
                    TopLevelDecl::Mixin(mixin) => (Some(&mixin.name), &mixin.members, Vec::new()),
                    TopLevelDecl::Extension(extension) => {
                        (extension.name.as_ref(), &extension.members, Vec::new())
                    }
                    TopLevelDecl::ExtensionType(extension) => (
                        Some(&extension.name),
                        &extension.members,
                        extension
                            .representation()
                            .map(|representation| representation.name.name.as_str())
                            .into_iter()
                            .collect(),
                    ),
                    TopLevelDecl::Enum(enumeration) => (
                        Some(&enumeration.name),
                        &enumeration.members,
                        enumeration
                            .variants
                            .iter()
                            .map(|variant| variant.name.name.as_str())
                            .collect(),
                    ),
                };
            let key = declaration_key(declaration);
            if let Some(name) = name {
                scopes.top_level.insert(&name.name);
                scopes.types.insert(&name.name, key);
            }
            let mut names: HashSet<&str> = extra.into_iter().collect();
            for member in members {
                match member {
                    ClassMember::Field(field) => names.extend(
                        field
                            .declarators
                            .iter()
                            .map(|declarator| declarator.name.name.as_str()),
                    ),
                    ClassMember::Method(method) => {
                        names.insert(&method.name.name);
                    }
                    ClassMember::Getter(getter) => {
                        names.insert(&getter.name.name);
                    }
                    ClassMember::Setter(setter) => {
                        names.insert(&setter.name.name);
                    }
                    ClassMember::Constructor(_)
                    | ClassMember::PrimaryConstructorBody(_)
                    | ClassMember::Operator(_)
                    | ClassMember::Error(_) => {}
                }
            }
            scopes.members.insert(key, names);
        }
        scopes
    }

    fn extend(&mut self, other: Self) {
        self.types.extend(other.types);
        self.members.extend(other.members);
        self.top_level.extend(other.top_level);
    }
    /// The declaring type (`Some(None)` for top level) that `mention` denotes,
    /// or `None` when it cannot be resolved here, which disqualifies every
    /// declaration of the name.
    fn owner(&self, mention: &Mention<'_>) -> Option<Option<usize>> {
        let declares = |key: usize| {
            self.members
                .get(&key)
                .is_some_and(|names| names.contains(mention.name))
        };
        match mention.access {
            // Lexical lookup: the enclosing type's own members, then the
            // library scope; an unresolved name in a type body may be an
            // inherited member (`this._x`).
            Access::Bare => match mention.enclosing {
                Some(key) if declares(key) => Some(Some(key)),
                _ if self.top_level.contains(mention.name) => Some(None),
                Some(_) => None,
                None => Some(None),
            },
            Access::This => mention.enclosing.filter(|key| declares(*key)).map(Some),
            Access::Type(key) => declares(key).then_some(Some(key)),
            Access::Member => None,
        }
    }
}

/// How a mention of a candidate name is reached.
#[derive(Clone, Copy)]
enum Access {
    /// An unqualified identifier.
    Bare,
    /// `this.name`.
    This,
    /// `Type.name` for a type declared in this file.
    Type(usize),
    /// Any other member access (`object.name`, `super.name`, cascade sections,
    /// dot shorthands): the receiver's declaring type is not resolved here.
    Member,
}

struct Mention<'a> {
    name: &'a str,
    start: usize,
    /// The enclosing type declaration ([`declaration_key`]).
    enclosing: Option<usize>,
    access: Access,
}

/// Every occurrence of a candidate name, declarations included. Like the
/// analyzer, the bodies of enums and of classes with a `const` constructor are
/// not visited.
struct Mentions<'a, 'n> {
    names: &'a HashSet<&'n str>,
    types: &'a HashMap<&'a str, usize>,
    enclosing: Option<usize>,
    /// Starts of identifiers naming a member in a cascade section.
    cascade_members: HashSet<usize>,
    declarations: HashSet<usize>,
    found: Vec<Mention<'n>>,
}

impl Mentions<'_, '_> {
    fn record(&mut self, identifier: &Identifier, access: Access) {
        if let Some(name) = self.names.get(identifier.name.as_str()) {
            let access = if self.cascade_members.contains(&identifier.span.start) {
                Access::Member
            } else {
                access
            };
            self.found.push(Mention {
                name,
                start: identifier.span.start,
                enclosing: self.enclosing,
                access,
            });
        }
    }
}

impl Visitor for Mentions<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.declarations.extend(
                variable
                    .declarators
                    .iter()
                    .map(|declarator| declarator.name.span.start),
            );
        }
        let enclosing = match node {
            TopLevelDecl::Enum(_) => return,
            TopLevelDecl::Class(class) if has_const_constructor(&class.members) => return,
            TopLevelDecl::MixinClass(class) if has_const_constructor(&class.members) => return,
            TopLevelDecl::Class(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Mixin(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_) => Some(declaration_key(node)),
            TopLevelDecl::Variable(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => None,
        };
        let saved = std::mem::replace(&mut self.enclosing, enclosing);
        walk_top_level_decl(self, node);
        self.enclosing = saved;
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.declarations.extend(
            node.declarators
                .iter()
                .map(|declarator| declarator.name.span.start),
        );
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.declarations.extend(
            node.declarators
                .iter()
                .map(|declarator| declarator.name.span.start),
        );
        walk_local_var(self, node);
    }

    fn visit_identifier(&mut self, node: &Identifier) {
        self.record(node, Access::Bare);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Field { object, field, .. } => {
                let access = match object.unparenthesized() {
                    Expr::This { .. } => Access::This,
                    Expr::Ident(owner) => self
                        .types
                        .get(owner.name.as_str())
                        .map_or(Access::Member, |key| Access::Type(*key)),
                    _ => Access::Member,
                };
                self.record(field, access);
            }
            Expr::DotShorthand { name, .. } => self.record(name, Access::Member),
            Expr::Cascade { sections, .. } => {
                for op in sections.iter().flat_map(|section| &section.ops) {
                    let member = match op {
                        CascadeOp::Field { name, .. } | CascadeOp::Call(name, ..) => Some(name),
                        CascadeOp::Assign(target, ..) => receiver_root(target),
                        CascadeOp::Invoke(..)
                        | CascadeOp::Index { .. }
                        | CascadeOp::IndexAssign(..) => None,
                    };
                    if let Some(member) = member {
                        self.cascade_members.insert(member.span.start);
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

/// The identifier a selector chain starts from: `a` in `a.b[0].c()`.
fn receiver_root(mut expression: &Expr) -> Option<&Identifier> {
    loop {
        match expression.unparenthesized() {
            Expr::Ident(identifier) => return Some(identifier),
            Expr::Field { object, .. } | Expr::Index { object, .. } => expression = object,
            Expr::Call { callee, .. } => expression = callee,
            _ => return None,
        }
    }
}
