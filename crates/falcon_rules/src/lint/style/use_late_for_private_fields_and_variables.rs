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
//! disqualifies the referenced declaration. Qualified and cascade accesses are
//! attributed to their resolved declaration, not every implementation of an
//! interface member. Uses in every resolved library part are checked together;
//! unresolved parts prevent proving a candidate safe.
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

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
    TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    Visitor, walk_expr, walk_field_decl, walk_local_var, walk_top_level_decl,
};
use falcon_syntax::cst::*;

use crate::lint::declarations::declaration_key;
use crate::lint::doc_comments::{DocElement, DocLookup, DocResolver, DocScopes, references};
use crate::lint::expressions::is_parenthesized;
use crate::lint::semantic_type_operations::nullability;
use falcon_analyze::semantic_scope::{
    CascadeAssignment, SemanticRuleVisitor, SemanticState, visit_program,
};

#[derive(Default)]
pub struct UseLateForPrivateFieldsAndVariables;

impl Rule for UseLateForPrivateFieldsAndVariables {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let has_part_directives =
            !program.part_directives().is_empty() || program.part_of_directive().is_some();
        match ctx.library {
            Some(library) if library.has_unresolved_parts() => {
                return RulePreparation::Diagnostics(Vec::new());
            }
            None if has_part_directives => return RulePreparation::Diagnostics(Vec::new()),
            _ => {}
        }
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let candidates = collect_candidates(program, &state, ctx);
        if candidates.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let names: HashSet<&str> = candidates.iter().map(|candidate| candidate.name).collect();

        let mut scopes = DeclarationScopes::of(program, 0);
        if let Some(library) = ctx.library {
            for (file, sibling) in library.siblings().iter().enumerate() {
                scopes.extend(DeclarationScopes::of(*sibling, file + 1));
            }
        }
        let owners: HashMap<DeclarationIdentity, OwnerKey> = scopes
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
        check_doc_uses(program, ctx, &state, &names, &owners, &mut disqualified);
        if let Some(library) = ctx.library {
            for (file, (sibling, parsed)) in library
                .siblings()
                .iter()
                .zip(library.sibling_parsed())
                .enumerate()
            {
                let sibling_ctx = AnalyzeContext {
                    types: ctx.types,
                    identities: ctx.identities,
                    signatures: ctx.signatures,
                    library: ctx.library,
                    environment: ctx.environment,
                    package_language_version: ctx.package_language_version,
                    ..AnalyzeContext::new(ctx.file_path, ctx.config, parsed)
                };
                if let Some(state) = SemanticState::new(*sibling, &sibling_ctx) {
                    check_uses(
                        file + 1,
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

        RulePreparation::Semantic(Box::new(LateCandidates {
            program,
            ctx,
            candidates,
            scopes,
            disqualified,
            safe: SafeUses {
                types: ctx.types,
                names: Cow::Owned(names),
                owners: Cow::Owned(owners),
                resolved_owners: HashMap::new(),
                spans: HashSet::new(),
            },
        }))
    }
}

struct LateCandidates<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    candidates: Vec<Candidate<'a>>,
    scopes: DeclarationScopes<'a>,
    disqualified: HashSet<NullableAccess<'a>>,
    safe: SafeUses<'a, 'a>,
}

impl<'a> SemanticRuleVisitor<'a> for LateCandidates<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        self.safe.visit_expr(node, state);
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
    ) {
        self.safe
            .visit_cascade_operation(operation, receiver, section, state);
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
    ) {
        self.safe
            .visit_cascade_assignment(assignment, receiver, section, state);
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        check_mentions(
            self.program,
            0,
            &self.safe.names,
            &self.scopes,
            &self.safe,
            &mut self.disqualified,
        );
        self.candidates
            .iter()
            .filter(|candidate| {
                !self
                    .disqualified
                    .contains(&NullableAccess::Unresolved(candidate.name))
                    && !self
                        .disqualified
                        .contains(&NullableAccess::Resolved(candidate.name, candidate.owner))
            })
            .map(|candidate| {
                self.ctx.diagnostic(
                    "use-late-for-private-fields-and-variables",
                    "Use 'late' for private members with a non-nullable type.",
                    candidate.span,
                )
            })
            .collect()
    }
}

/// A tree-local declaration ID qualified by its file's position in the library.
type OwnerKey = (usize, NodeId);

/// A resolved declaration, or a name whose owner cannot be determined.
#[derive(Hash, PartialEq, Eq)]
enum NullableAccess<'a> {
    Unresolved(&'a str),
    Resolved(&'a str, Option<OwnerKey>),
}

fn check_uses<'a, 'n>(
    file: usize,
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    names: &HashSet<&'n str>,
    scopes: &DeclarationScopes<'_>,
    owners: &HashMap<DeclarationIdentity, OwnerKey>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let program = ctx.parsed.program();
    check_doc_uses(program, ctx, &state, names, owners, disqualified);
    let mut safe = SafeUses {
        types: ctx.types,
        names: Cow::Borrowed(names),
        owners: Cow::Borrowed(owners),
        resolved_owners: HashMap::new(),
        spans: HashSet::new(),
    };
    visit_program(&mut safe, program, state);
    check_mentions(program, file, names, scopes, &safe, disqualified);
}

fn check_doc_uses<'a, 'n>(
    program: Program<'a>,
    ctx: &AnalyzeContext<'_>,
    state: &SemanticState<'a>,
    names: &HashSet<&'n str>,
    owners: &HashMap<DeclarationIdentity, OwnerKey>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let doc_scopes = DocScopes::new(program, &state.model, ctx.source);
    let resolver = DocResolver::new(program, &state.model, &state.signatures, &doc_scopes, false);
    for block in ctx.tree().doc_blocks(ctx.source) {
        let Some(scope) = doc_scopes.scope_for(&block) else {
            continue;
        };
        for reference in references(&block).0 {
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
}

fn check_mentions<'n>(
    program: Program<'_>,
    file: usize,
    names: &HashSet<&'n str>,
    scopes: &DeclarationScopes<'_>,
    safe: &SafeUses<'_, '_>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let mut mentions = Mentions {
        names,
        types: &scopes.types,
        file,
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

struct Candidate<'a> {
    name: &'a str,
    span: Span,
    /// The declaring type ([`declaration_key`]); `None` for top-level variables.
    owner: Option<OwnerKey>,
}

fn collect_candidates<'a>(
    program: Program<'a>,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext,
) -> Vec<Candidate<'a>> {
    let mut candidates = Vec::new();
    for declaration in program.declarations() {
        let (type_params, members, all_fields, static_only) = match declaration {
            TopLevelDecl::Variable(variable) => {
                if !variable.is_late()
                    && !variable.is_external()
                    && variable.keyword() != VarKeyword::Const
                {
                    let scope = TypeParameterScope::default();
                    for declarator in variable.declarators() {
                        if is_private(declarator.name().name())
                            && potentially_nullable(
                                variable.var_type(),
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
                if has_const_constructor(class.members()) {
                    continue;
                }
                (class.type_params(), class.members(), false, false)
            }
            TopLevelDecl::MixinClass(class) => {
                if has_const_constructor(class.members()) {
                    continue;
                }
                (class.type_params(), class.members(), false, false)
            }
            TopLevelDecl::Mixin(mixin) => (mixin.type_params(), mixin.members(), false, false),
            TopLevelDecl::Extension(extension) => {
                let private = extension.name().is_none_or(|name| is_private(name.name()));
                (extension.type_params(), extension.members(), private, false)
            }
            TopLevelDecl::ExtensionType(extension) => {
                (extension.type_params(), extension.members(), false, true)
            }
            _ => continue,
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_params, &state.model);
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.is_late()
                || field.is_external()
                || field.is_abstract()
                || field.keyword() == VarKeyword::Const
                || (static_only && !field.is_static())
            {
                continue;
            }
            for declarator in field.declarators() {
                if (all_fields || is_private(declarator.name().name()))
                    && potentially_nullable(field.field_type(), declarator, &scope, state, ctx)
                {
                    candidates.push(candidate(
                        declarator,
                        Some((0, declaration_key(declaration))),
                    ));
                }
            }
        }
    }
    candidates
}

fn candidate(declarator: VarDeclarator<'_>, owner: Option<OwnerKey>) -> Candidate<'_> {
    Candidate {
        name: declarator.name().name(),
        span: declarator.span(),
        owner,
    }
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

fn has_const_constructor<'a>(mut members: AstChildren<'a, ClassMember<'a>>) -> bool {
    members.any(
        |member| matches!(member, ClassMember::Constructor(constructor) if constructor.is_const()),
    )
}

/// Whether the declared type is proven not to be non-nullable (the analyzer's
/// `!isNonNullable`), including `dynamic` and potentially nullable type
/// parameters. An unresolved type is never a candidate.
fn potentially_nullable<'a>(
    declared: Option<DartType<'a>>,
    declarator: VarDeclarator<'a>,
    scope: &TypeParameterScope,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext,
) -> bool {
    let ty = match (
        declared,
        declarator.initializer().map(Expr::unparenthesized),
    ) {
        (Some(ty), _) => state.model.resolve_type_in(ty, scope),
        // Without a type or initializer the variable is `dynamic`; a `null`
        // initializer also infers `dynamic`.
        (None, None | Some(Expr::NullLit(_))) => ResolvedType::Dynamic,
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
struct SafeUses<'s, 'a> {
    types: Option<&'s falcon_analyze::TypeIndex>,
    names: Cow<'s, HashSet<&'a str>>,
    owners: Cow<'s, HashMap<DeclarationIdentity, OwnerKey>>,
    resolved_owners: HashMap<usize, Option<OwnerKey>>,
    spans: HashSet<usize>,
}

impl SafeUses<'_, '_> {
    fn record_owner(
        &mut self,
        identifier: Identifier<'_>,
        receiver: &ResolvedType,
        state: &SemanticState<'_>,
    ) {
        if !self.names.contains(identifier.name()) {
            return;
        }
        let owner = state
            .signatures
            .resolved_readable_member_owner(receiver, identifier.name(), &state.model)
            .or_else(|| {
                state.signatures.resolved_writable_member_owner(
                    receiver,
                    identifier.name(),
                    &state.model,
                )
            });
        if let Some(key) = owner.as_ref().and_then(|owner| self.owners.get(owner)) {
            self.resolved_owners
                .insert(identifier.span().start, Some(*key));
        }
    }

    fn record(&mut self, expression: Expr<'_>) {
        let identifier = match expression.unparenthesized() {
            Expr::Ident(identifier) => identifier,
            Expr::Field(field) => field.field(),
            _ => return,
        };
        if self.names.contains(identifier.name()) {
            self.spans.insert(identifier.span().start);
        }
    }

    /// The analyzer requires the identifier itself to be the `!` operand,
    /// not a parenthesized expression or a property's name.
    fn record_null_assert(&mut self, operand: Expr<'_>) {
        if let Expr::Ident(identifier) = operand
            && self.names.contains(identifier.name())
            && !is_parenthesized(operand)
        {
            self.spans.insert(identifier.span().start);
        }
    }

    fn record_assignment<'a>(
        &mut self,
        target: Expr<'_>,
        op: AssignOp,
        value: Expr<'a>,
        state: &SemanticState<'a>,
    ) {
        if op == AssignOp::Eq
            && nullability(&state.infer(value), &state.signatures, self.types) == Truth::No
        {
            self.record(target);
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for SafeUses<'_, '_> {
    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        let name = match operation {
            CascadeOp::Field(field) => field.name(),
            CascadeOp::Call(call) => call.name(),
            _ => return,
        };
        self.record_owner(name, receiver, state);
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if !assignment.indexed
            && let Expr::Ident(identifier) = assignment.target.unparenthesized()
        {
            self.record_owner(identifier, receiver, state);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Field(field) = node
            && self.names.contains(field.field().name())
        {
            self.record_owner(field.field(), &state.infer(field.object()), state);
        } else if let Expr::Ident(identifier) = node
            && self.names.contains(identifier.name())
            && state
                .environment
                .binding_depth(identifier.name())
                .is_some_and(|depth| depth > 0)
        {
            // A local binding is not the same element as a private field.
            self.spans.insert(identifier.span().start);
        }
        match node {
            Expr::NullAssert(assertion) => self.record_null_assert(assertion.operand()),
            Expr::Assign(assignment) => {
                self.record_assignment(
                    assignment.target(),
                    assignment.op(),
                    assignment.value(),
                    state,
                );
            }
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    let mut ops = section.ops();
                    if let Some(CascadeOp::Assign(assignment)) = ops.next()
                        && ops.next().is_none()
                    {
                        self.record_assignment(
                            assignment.target(),
                            assignment.op(),
                            assignment.value(),
                            state,
                        );
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
    types: HashMap<&'p str, OwnerKey>,
    /// Names declared by each type declaration, by [`declaration_key`].
    members: HashMap<OwnerKey, HashSet<&'p str>>,
    top_level: HashSet<&'p str>,
}

impl<'p> DeclarationScopes<'p> {
    fn of(program: Program<'p>, file: usize) -> Self {
        let mut scopes = Self {
            types: HashMap::new(),
            members: HashMap::new(),
            top_level: HashSet::new(),
        };
        for declaration in program.declarations() {
            let (name, members, extra): (
                Option<Identifier<'p>>,
                AstChildren<'p, ClassMember<'p>>,
                Vec<&str>,
            ) = match declaration {
                TopLevelDecl::Variable(variable) => {
                    scopes.top_level.extend(
                        variable
                            .declarators()
                            .map(|declarator| declarator.name().name()),
                    );
                    continue;
                }
                TopLevelDecl::Function(function) => {
                    scopes.top_level.insert(function.name().name());
                    continue;
                }
                TopLevelDecl::ClassTypeAlias(alias) => {
                    scopes.top_level.insert(alias.name().name());
                    continue;
                }
                TopLevelDecl::TypeAlias(alias) => {
                    scopes.top_level.insert(alias.name().name());
                    continue;
                }
                TopLevelDecl::Error(_) => continue,
                TopLevelDecl::Class(class) => (Some(class.name()), class.members(), Vec::new()),
                TopLevelDecl::MixinClass(class) => {
                    (Some(class.name()), class.members(), Vec::new())
                }
                TopLevelDecl::Mixin(mixin) => (Some(mixin.name()), mixin.members(), Vec::new()),
                TopLevelDecl::Extension(extension) => {
                    (extension.name(), extension.members(), Vec::new())
                }
                TopLevelDecl::ExtensionType(extension) => (
                    Some(extension.name()),
                    extension.members(),
                    extension
                        .representation()
                        .map(|representation| representation.name().name())
                        .into_iter()
                        .collect(),
                ),
                TopLevelDecl::Enum(enumeration) => (
                    Some(enumeration.name()),
                    enumeration.members(),
                    enumeration
                        .variants()
                        .map(|variant| variant.name().name())
                        .collect(),
                ),
            };
            let key = (file, declaration_key(declaration));
            if let Some(name) = name {
                scopes.top_level.insert(name.name());
                scopes.types.insert(name.name(), key);
            }
            let mut names: HashSet<&str> = extra.into_iter().collect();
            for member in members {
                match member {
                    ClassMember::Field(field) => names.extend(
                        field
                            .declarators()
                            .map(|declarator| declarator.name().name()),
                    ),
                    ClassMember::Method(method) => {
                        names.insert(method.name().name());
                    }
                    ClassMember::Getter(getter) => {
                        names.insert(getter.name().name());
                    }
                    ClassMember::Setter(setter) => {
                        names.insert(setter.name().name());
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
    fn owner(&self, mention: &Mention<'_>) -> Option<Option<OwnerKey>> {
        let declares = |key: OwnerKey| {
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
    Type(OwnerKey),
    /// Any other member access (`object.name`, `super.name`, cascade sections,
    /// dot shorthands): the receiver's declaring type is not resolved here.
    Member,
}

struct Mention<'a> {
    name: &'a str,
    start: usize,
    /// The enclosing type declaration ([`declaration_key`]).
    enclosing: Option<OwnerKey>,
    access: Access,
}

/// Every occurrence of a candidate name, declarations included. Like the
/// analyzer, the bodies of enums and of classes with a `const` constructor are
/// not visited.
struct Mentions<'a, 'n> {
    names: &'a HashSet<&'n str>,
    types: &'a HashMap<&'a str, OwnerKey>,
    file: usize,
    enclosing: Option<OwnerKey>,
    /// Starts of identifiers naming a member in a cascade section.
    cascade_members: HashSet<usize>,
    declarations: HashSet<usize>,
    found: Vec<Mention<'n>>,
}

impl Mentions<'_, '_> {
    fn record(&mut self, identifier: Identifier<'_>, access: Access) {
        if let Some(name) = self.names.get(identifier.name()) {
            let access = if self.cascade_members.contains(&identifier.span().start) {
                Access::Member
            } else {
                access
            };
            self.found.push(Mention {
                name,
                start: identifier.span().start,
                enclosing: self.enclosing,
                access,
            });
        }
    }
}

impl<'a> Visitor<'a> for Mentions<'_, '_> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::Variable(variable) = node {
            self.declarations.extend(
                variable
                    .declarators()
                    .map(|declarator| declarator.name().span().start),
            );
        }
        let enclosing = match node {
            TopLevelDecl::Enum(_) => return,
            TopLevelDecl::Class(class) if has_const_constructor(class.members()) => return,
            TopLevelDecl::MixinClass(class) if has_const_constructor(class.members()) => return,
            TopLevelDecl::Class(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Mixin(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_) => Some((self.file, declaration_key(node))),
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

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        self.declarations.extend(
            node.declarators()
                .map(|declarator| declarator.name().span().start),
        );
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        self.declarations.extend(
            node.declarators()
                .map(|declarator| declarator.name().span().start),
        );
        walk_local_var(self, node);
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) {
        self.record(node, Access::Bare);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        match node {
            Expr::Field(field) => {
                let access = match field.object().unparenthesized() {
                    Expr::This(_) => Access::This,
                    Expr::Ident(owner) => self
                        .types
                        .get(owner.name())
                        .map_or(Access::Member, |key| Access::Type(*key)),
                    _ => Access::Member,
                };
                self.record(field.field(), access);
            }
            Expr::DotShorthand(shorthand) => self.record(shorthand.name(), Access::Member),
            Expr::Cascade(cascade) => {
                for op in cascade.sections().flat_map(|section| section.ops()) {
                    let member = match op {
                        CascadeOp::Field(field) => Some(field.name()),
                        CascadeOp::Call(call) => Some(call.name()),
                        CascadeOp::Assign(assignment) => receiver_root(assignment.target()),
                        CascadeOp::Invoke(_)
                        | CascadeOp::NullAssert(_)
                        | CascadeOp::Index(_)
                        | CascadeOp::IndexAssign(_) => None,
                    };
                    if let Some(member) = member {
                        self.cascade_members.insert(member.span().start);
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

/// The identifier a selector chain starts from: `a` in `a.b[0].c()`.
fn receiver_root(mut expression: Expr<'_>) -> Option<Identifier<'_>> {
    loop {
        match expression.unparenthesized() {
            Expr::Ident(identifier) => return Some(identifier),
            Expr::Field(field) => expression = field.object(),
            Expr::Index(index) => expression = index.object(),
            Expr::Call(call) => expression = call.callee(),
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::ParsedFile;
    use falcon_dart_parser::parse;
    use std::sync::Arc;

    #[test]
    fn declaration_scopes_distinguish_library_part_trees() {
        let first_source = "class A { int? _x; }";
        let second_source = "class B { int? _x; }";
        let first = parse(first_source);
        let second = parse(second_source);
        assert!(first.errors.is_empty(), "{:?}", first.errors);
        assert!(second.errors.is_empty(), "{:?}", second.errors);
        let first = Arc::new(ParsedFile {
            source: first_source.into(),
            tree: first.tree,
        });
        let second = Arc::new(ParsedFile {
            source: second_source.into(),
            tree: second.tree,
        });
        let first_program = first.program();
        let second_program = second.program();
        let first_declaration = first_program.declarations().next().unwrap();
        let second_declaration = second_program.declarations().next().unwrap();
        let mut scopes = DeclarationScopes::of(first_program, 0);
        scopes.extend(DeclarationScopes::of(second_program, 1));

        assert_eq!(scopes.members.len(), 2);
        assert_eq!(scopes.types["A"], (0, declaration_key(first_declaration)));
        assert_eq!(scopes.types["B"], (1, declaration_key(second_declaration)));
        for owner in scopes.types.values() {
            assert!(scopes.members[owner].contains("_x"));
        }
    }
}
