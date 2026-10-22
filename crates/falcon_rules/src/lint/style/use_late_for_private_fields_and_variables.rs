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
        match ctx.library() {
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

        let mut owners = HashMap::new();
        declaration_owners(program, 0, &state, &mut owners);
        if let Some(library) = ctx.library() {
            for (file, sibling) in library.siblings().iter().enumerate() {
                declaration_owners(*sibling, file + 1, &state, &mut owners);
            }
        }
        let mut disqualified = HashSet::new();
        check_doc_uses(program, ctx, &state, &names, &owners, &mut disqualified);
        if let Some(library) = ctx.library() {
            for (file, (sibling, parsed)) in library
                .siblings()
                .iter()
                .zip(library.sibling_parsed())
                .enumerate()
            {
                let sibling_ctx = ctx.for_file(library.sibling_paths()[file], parsed);
                if let Some(state) = SemanticState::new(*sibling, &sibling_ctx) {
                    check_uses(
                        &sibling_ctx,
                        state,
                        &names,
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
            disqualified,
            safe: SafeUses {
                types: ctx.types(),
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
    disqualified: HashSet<NullableAccess<'a>>,
    safe: SafeUses<'a, 'a>,
}

impl<'a> SemanticRuleVisitor<'a> for LateCandidates<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        self.safe.visit_expr(node, state);
    }

    fn visit_constructor(&mut self, node: ConstructorDecl<'a>, state: &SemanticState<'a>) {
        self.safe.visit_constructor(node, state);
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
            &self.safe.names,
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
                self.ctx.diagnostic("Use `late` for private members with a non-nullable type.", candidate.span)
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
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    names: &HashSet<&'n str>,
    owners: &HashMap<DeclarationIdentity, OwnerKey>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let program = ctx.parsed().program();
    check_doc_uses(program, ctx, &state, names, owners, disqualified);
    let mut safe = SafeUses {
        types: ctx.types(),
        names: Cow::Borrowed(names),
        owners: Cow::Borrowed(owners),
        resolved_owners: HashMap::new(),
        spans: HashSet::new(),
    };
    visit_program(&mut safe, program, state);
    check_mentions(program, names, &safe, disqualified);
}

fn check_doc_uses<'a, 'n>(
    program: Program<'a>,
    ctx: &AnalyzeContext<'_>,
    state: &SemanticState<'a>,
    names: &HashSet<&'n str>,
    owners: &HashMap<DeclarationIdentity, OwnerKey>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let doc_scopes = DocScopes::new(program, &state.model, ctx.source());
    let resolver = DocResolver::new(program, &state.model, &state.signatures, &doc_scopes, false);
    for block in ctx.tree().doc_blocks(ctx.source()) {
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
    names: &HashSet<&'n str>,
    safe: &SafeUses<'_, '_>,
    disqualified: &mut HashSet<NullableAccess<'n>>,
) {
    let mut mentions = Mentions {
        names,
        declarations: HashSet::new(),
        found: Vec::new(),
    };
    mentions.visit_program(program);
    for mention in &mentions.found {
        if mentions.declarations.contains(&mention.start) || safe.spans.contains(&mention.start) {
            continue;
        }
        match safe.resolved_owners.get(&mention.start).copied() {
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
    /// The declaring type's syntax identity; `None` for top-level variables.
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
                    candidates.push(candidate(declarator, Some((0, declaration.syntax().id()))));
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
                || nullability(bound, &state.signatures, ctx.types()) == Truth::Yes
        }
        _ => nullability(&ty, &state.signatures, ctx.types()) == Truth::Yes,
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
            .referenced_member_owner(receiver, identifier.name(), false)
            .or_else(|| state.referenced_member_owner(receiver, identifier.name(), true));
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
    fn visit_constructor(&mut self, node: ConstructorDecl<'a>, state: &SemanticState<'a>) {
        let Some(receiver) = state.enclosing_type.as_ref() else {
            return;
        };
        for parameter in node.params().params() {
            if parameter.initializing() == Some(InitializingFormal::This) {
                self.record_owner(parameter.name(), receiver, state);
            }
        }
        for initializer in node.initializers() {
            if let ConstructorInitializer::FieldInit(field) = initializer {
                self.record_owner(field.field(), receiver, state);
            }
        }
    }

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
        let identifier = match node {
            Expr::Ident(identifier) => Some(identifier),
            Expr::Field(field) => Some(field.field()),
            _ => None,
        };
        if let Some(identifier) = identifier
            && self.names.contains(identifier.name())
        {
            if matches!(node, Expr::Ident(_))
                && state.environment.binding_depth(identifier.name()).is_some_and(|depth| depth > 0)
            {
                // A local binding is not the same element as a private field.
                self.spans.insert(identifier.span().start);
            } else if let Some(reference) = state.referenced_declaration(node, false)
                .or_else(|| state.referenced_declaration(node, true))
            {
                if reference.selector.name.is_empty() {
                    self.resolved_owners.insert(identifier.span().start, None);
                } else if let Some(key) = self.owners.get(&reference.owner) {
                    self.resolved_owners.insert(identifier.span().start, Some(*key));
                }
            }
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

/// Maps semantic owners to tree-local candidate identities, including parts.
fn declaration_owners(
    program: Program<'_>,
    file: usize,
    state: &SemanticState<'_>,
    owners: &mut HashMap<DeclarationIdentity, OwnerKey>,
) {
    for declaration in program.declarations() {
        let name = match declaration {
            TopLevelDecl::Class(class) => Some(class.name()),
            TopLevelDecl::MixinClass(class) => Some(class.name()),
            TopLevelDecl::Mixin(mixin) => Some(mixin.name()),
            TopLevelDecl::Extension(extension) => extension.name(),
            TopLevelDecl::ExtensionType(extension) => Some(extension.name()),
            TopLevelDecl::Enum(enumeration) => Some(enumeration.name()),
            _ => None,
        };
        if let Some(name) = name
            && let Some(identity) = state.model.resolve_name(name.name())
                .or_else(|| state.model.resolve_extension(name.name()))
        {
            owners.insert(identity, (file, declaration.syntax().id()));
        }
    }
}

struct Mention<'a> {
    name: &'a str,
    start: usize,
}

/// Syntax occurrences not accounted for by semantic expression callbacks remain
/// conservative (for example, initializing formals and dot shorthands).
struct Mentions<'a, 'n> {
    names: &'a HashSet<&'n str>,
    declarations: HashSet<usize>,
    found: Vec<Mention<'n>>,
}

impl Mentions<'_, '_> {
    fn record(&mut self, identifier: Identifier<'_>) {
        if let Some(name) = self.names.get(identifier.name()) {
            self.found.push(Mention {
                name,
                start: identifier.span().start,
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
        match node {
            TopLevelDecl::Enum(_) => return,
            TopLevelDecl::Class(class) if has_const_constructor(class.members()) => return,
            TopLevelDecl::MixinClass(class) if has_const_constructor(class.members()) => return,
            _ => {}
        }
        walk_top_level_decl(self, node);
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
        self.record(node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        match node {
            Expr::Field(field) => self.record(field.field()),
            Expr::DotShorthand(shorthand) => self.record(shorthand.name()),
            _ => {}
        }
        walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::UseLateForPrivateFieldsAndVariables;
    use crate::lint::test_support::TestIndexes;
    use falcon_analyze::{ParsedFile, RuleAnalysis};
    use falcon_config::FalconConfig;
    use falcon_diagnostics::Span;
    use std::path::Path;
    use std::sync::Arc;

    fn assert_only_field(source: &str, owner: &str) {
        let falcon_dart_parser::ParseResult { errors, tree } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let path = Path::new("main.dart");
        let config = FalconConfig::default();
        let indexes = TestIndexes::single(path, &parsed, true);
        let context = indexes.context(path, &config, &parsed);
        let diagnostics = UseLateForPrivateFieldsAndVariables.analyze(
            "use-late-for-private-fields-and-variables",
            parsed.program(),
            &context,
        );
        let owner_start = source.find(owner).unwrap();
        let field_start = owner_start + source[owner_start..].find("_value").unwrap();
        assert_eq!(
            diagnostics.iter().map(|diagnostic| diagnostic.span).collect::<Vec<_>>(),
            [Span {
                start: field_start,
                end: field_start + "_value".len(),
            }],
        );
    }

    #[test]
    fn constructor_initialization_disqualifies_only_the_declaring_field() {
        assert_only_field(
            r"
class First {
  int? _value;
  First(this._value);
}
class Second {
  int? _value;
  void setValue() {
    _value = 1;
    print(_value!);
  }
}
class Third {
  int? _value;
  Third() : _value = null;
}
",
            "class Second",
        );
    }

    #[test]
    fn library_variables_precede_inherited_and_extension_on_type_members() {
        assert_only_field(
            r"
int? _value;
class Parent {
  int? _value;
}
class Child extends Parent {
  void readLibraryVariable() {
    print(_value);
  }
}
class Declared {
  int? _value;
  void readDeclaredField() {
    print(_value);
  }
}
extension Reader on Parent {
  void readLibraryVariable() {
    print(_value);
  }
}
",
            "class Parent",
        );
    }
}


