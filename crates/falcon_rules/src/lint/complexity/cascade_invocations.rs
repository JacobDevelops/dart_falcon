//! Flags consecutive statements that invoke members on the same variable.
//!
//! ```dart
//! final list = <int>[];   // bad
//! list.add(1);
//! list.add(2);
//!
//! final list = <int>[]    // good
//!   ..add(1)
//!   ..add(2);
//! ```
//!
//! Within one block (or `switch` case body) the rule follows the official
//! lint's cascade model. A statement that calls an instance method on, reads
//! a property of, sets a property of, or cascades on a simple identifier `v`
//! can join the preceding statement when that one targeted the same `v` and
//! can receive: a declaration of the local `v` (not `const`), an assignment
//! to the local `v` (not `??=`), a property set on a local `v` (not `??=`),
//! or a method call, property read, or cascade on `v`. Assignments to fields
//! and top-level variables never receive (the analyzer sees their setter, not
//! the variable), nor do property sets through them. Null-aware accesses
//! (`v?.m()`) and calls of static methods never join.
//! Method calls require a resolved non-static method: dynamic invocations and
//! function `call` invocations do not qualify. Parenthesized receivers are not
//! simple identifiers, while property assignments still accept dynamic locals.
//! Receiver types come from shared resolution, including imported top-level
//! getters/variables and implicit method calls used as cascade initializers;
//! genuinely unresolved receivers stay silent rather than guessing a method.
//!
//! A statement whose member names, arguments, or right-hand side depend on
//! the receiver breaks the chain, since a cascade would evaluate them before
//! the earlier member access: a reference to the same local declaration (not
//! a shadowing closure parameter), and for a field or top-level-variable receiver
//! also a closure or a reference to any member declared in the same class as
//! the field (respectively any top-level declaration of the same library).
//! One diagnostic covers each run of
//! joinable statements, from the first statement that could be cascaded to
//! the last one. Where falcon cannot resolve a referenced member's declaring
//! class (for example through an external type), it assumes a dependency and
//! stays silent.
//!
//! Adopted from the official `cascade_invocations` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, LibraryId, MemberKind, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::visit::{Visitor, for_each_stmt_list, walk_expr_chain};
use falcon_syntax::cst::*;

use crate::lint::const_eval::mentions_type_parameter;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct CascadeInvocations;

impl Rule for CascadeInvocations {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Statements {
            program,
            ctx,
            locals: ctx.local_declarations(program),
            prefixes: program
                .imports()
                .filter_map(|import| import.as_name())
                .map(|prefix| prefix.name())
                .collect(),
            facts: HashMap::new(),
        }))
    }
}

/// The library a top-level declaration belongs to.
#[derive(Clone, PartialEq, Eq)]
enum Library {
    Source(LibraryId),
    Sdk(falcon_analyze::InternedString),
    Package(falcon_analyze::InternedString),
}

impl Library {
    fn of(identity: &DeclarationIdentity) -> Self {
        match identity {
            DeclarationIdentity::Library { library, .. } => Self::Source(*library),
            DeclarationIdentity::Sdk { library, .. } => Self::Sdk(library.clone()),
            DeclarationIdentity::Package { package, .. } => Self::Package(package.clone()),
        }
    }
}

/// Where a referenced declaration lives: the analyzer's `enclosingElement`.
#[derive(Clone, PartialEq, Eq)]
enum Owner {
    Type(DeclarationIdentity),
    Library(Library),
}

/// The element a statement's simple target identifier resolves to.
#[derive(Clone, PartialEq, Eq)]
enum Element<'a> {
    /// A local variable or parameter.
    Local(NodeId),
    /// A field or top-level variable, read through its getter.
    Property { name: &'a str, owner: Owner },
    /// A type name, as in `Type.property`.
    Type(&'a str),
}

/// What a statement's critical nodes reference, as far as dependencies on
/// its own receiver go.
#[derive(Default)]
struct Critical<'a> {
    /// Unqualified identifiers, with their local declaration when applicable.
    names: Vec<(&'a str, Option<NodeId>)>,
    /// Whether a closure occurs; it may read any property.
    closure: bool,
    /// Whether a member or top-level declaration of the receiver's owner is
    /// referenced, or one whose owner falcon cannot rule out.
    owner_referenced: bool,
}

impl Critical<'_> {
    /// Whether these nodes depend on `element`.
    fn depends_on(&self, element: &Element<'_>) -> bool {
        if self.names.iter().any(|(name, declaration)| match element {
            Element::Local(target) => declaration == &Some(*target),
            Element::Property { name: target, .. } | Element::Type(target) => {
                declaration.is_none() && name == target
            }
        }) {
            return true;
        }
        matches!(element, Element::Property { .. }) && (self.closure || self.owner_referenced)
    }
}

/// One statement's role in a potential cascade.
#[derive(Default)]
struct Cascadable<'a> {
    element: Option<Element<'a>>,
    critical: Critical<'a>,
    can_join: bool,
    can_receive: bool,
    can_be_cascaded: bool,
}

impl<'a> Cascadable<'a> {
    /// A call, property read, or cascade on `element`, which both joins and
    /// receives.
    fn member_access(element: Element<'a>, critical: Critical<'a>) -> Self {
        Self {
            element: Some(element),
            critical,
            can_join: true,
            can_receive: true,
            can_be_cascaded: true,
        }
    }

    /// Whether this statement can be cascaded onto `previous`.
    fn compatible_with(&self, previous: &Cascadable<'_>) -> bool {
        let Some(element) = &self.element else {
            return false;
        };
        previous.can_receive
            && self.can_join
            && (self.can_be_cascaded || previous.can_be_cascaded)
            && previous.element.as_ref() == Some(element)
            && !self.critical.depends_on(element)
            && !previous.critical.depends_on(element)
    }
}

/// Cascade facts of every expression and declaration statement, keyed by the
/// statement's start.
struct Statements<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    /// Local declarations keyed by their identifier uses.
    locals: &'a HashMap<usize, NodeId>,
    /// Import prefixes of this file.
    prefixes: HashSet<&'a str>,
    facts: HashMap<usize, Cascadable<'a>>,
}

impl<'a> SemanticRuleVisitor<'a> for Statements<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_stmt_list(self.program, &mut |list, _| {
            check_statements(list, &self.facts, self.ctx, &mut diags);
        });
        diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let scope = Scope {
            state,
            locals: self.locals,
            prefixes: &self.prefixes,
        };
        let facts = match node {
            Stmt::LocalVar(declaration) => scope.declaration(declaration),
            Stmt::Expr(statement) => scope.expression(statement.expr()),
            _ => return,
        };
        self.facts.insert(node.span().start, facts);
    }
}

/// Name resolution at one statement.
struct Scope<'s, 'a> {
    state: &'s SemanticState<'a>,
    locals: &'s HashMap<usize, NodeId>,
    prefixes: &'s HashSet<&'a str>,
}

impl<'a> Scope<'_, 'a> {
    fn is_local(&self, identifier: Identifier<'a>) -> bool {
        self.locals.contains_key(&identifier.span().start)
    }

    fn declaration(&self, declaration: LocalVarDecl<'a>) -> Cascadable<'a> {
        let mut declarators = declaration.declarators();
        let Some(declarator) = declarators.next() else {
            return Cascadable::default();
        };
        if declarators.next().is_some() {
            return Cascadable::default();
        }
        // `await` binds looser than it looks next to a cascade.
        if matches!(declarator.initializer(), Some(Expr::Await(_))) {
            return Cascadable::default();
        }
        let initializer = declarator.initializer();
        Cascadable {
            element: Some(Element::Local(declarator.name().syntax().id())),
            critical: self.critical(initializer.as_slice(), None),
            can_receive: declaration.keyword() != VarKeyword::Const,
            ..Cascadable::default()
        }
    }

    fn expression(&self, expr: Expr<'a>) -> Cascadable<'a> {
        match expr {
            Expr::Assign(assignment) => match assignment.target() {
                // `v = e` receives a following cascade on a local `v`; for a
                // field or top-level variable the analyzer sees the setter.
                Expr::Ident(identifier) => {
                    let Some(&declaration) = self.locals.get(&identifier.span().start) else {
                        return Cascadable::default();
                    };
                    Cascadable {
                        element: Some(Element::Local(declaration)),
                        critical: self.critical(&[assignment.value()], None),
                        can_receive: assignment.op() != AssignOp::NullCoalesceEq,
                        ..Cascadable::default()
                    }
                }
                // `v.p = e` sets a property on `v`; only a local `v` is a
                // variable element that can receive.
                Expr::Field(field) if !field.is_null_safe() => {
                    let Some(element) = self.simple_element(field.object()) else {
                        return Cascadable::default();
                    };
                    Cascadable {
                        critical: self.critical(&[assignment.value()], Some(&element)),
                        can_join: true,
                        can_receive: assignment.op() != AssignOp::NullCoalesceEq
                            && matches!(element, Element::Local(_)),
                        can_be_cascaded: true,
                        element: Some(element),
                    }
                }
                _ => Cascadable::default(),
            },
            Expr::Call(call) => match call.callee() {
                // Static methods (on a type) never cascade.
                Expr::Field(field) if !field.is_null_safe() => {
                    match self.simple_element(field.object()) {
                        Some(element @ (Element::Local(_) | Element::Property { .. }))
                            if self.invokes_method(
                                field.object(),
                                field.field().name(),
                                call.args(),
                            ) =>
                        {
                            let mut nodes = Vec::new();
                            for_each_argument(call.args(), |argument| nodes.push(argument));
                            let mut critical = self.critical(&nodes, Some(&element));
                            if let Element::Property { owner, .. } = &element {
                                critical.owner_referenced |= self.member_of_owner(
                                    Some(field.object()),
                                    field.field().name(),
                                    owner,
                                );
                            }
                            Cascadable::member_access(element, critical)
                        }
                        _ => Cascadable::default(),
                    }
                }
                _ => Cascadable::default(),
            },
            Expr::Cascade(cascade) => {
                let object = cascade.object();
                let Some(element) = self.simple_element(object) else {
                    return Cascadable::default();
                };
                let mut nodes = Vec::new();
                let mut member_names = Vec::new();
                for op in cascade.sections().flat_map(|section| section.ops()) {
                    match op {
                        CascadeOp::NullAssert(_) => {}
                        CascadeOp::Field(field) => member_names.push(field.name()),
                        CascadeOp::Index(index) => nodes.push(index.index()),
                        CascadeOp::Call(call) => {
                            member_names.push(call.name());
                            for_each_argument(call.args(), |argument| nodes.push(argument));
                        }
                        CascadeOp::Invoke(invoke) => {
                            for_each_argument(invoke.args(), |argument| nodes.push(argument));
                        }
                        CascadeOp::Assign(assignment) => {
                            nodes.push(assignment.target());
                            nodes.push(assignment.value());
                        }
                        CascadeOp::IndexAssign(assignment) => {
                            nodes.push(assignment.target());
                            nodes.push(assignment.value());
                        }
                    }
                }
                let mut critical = self.critical(&nodes, Some(&element));
                if let Element::Property { owner, .. } = &element {
                    critical.owner_referenced |= member_names
                        .iter()
                        .any(|name| self.member_of_owner(Some(object), name.name(), owner));
                }
                Cascadable::member_access(element, critical)
            }
            Expr::Field(field) if !field.is_null_safe() => {
                let object = field.object();
                let Some(element) = self.simple_element(object) else {
                    return Cascadable::default();
                };
                let mut critical = Critical::default();
                if let Element::Property { owner, .. } = &element {
                    critical.owner_referenced =
                        self.member_of_owner(Some(object), field.field().name(), owner);
                }
                Cascadable::member_access(element, critical)
            }
            _ => Cascadable::default(),
        }
    }

    /// The element a simple target identifier resolves to; `None` for
    /// anything else, including unresolved names and functions.
    fn simple_element(&self, expr: Expr<'a>) -> Option<Element<'a>> {
        let Expr::Ident(identifier) = expr else {
            return None;
        };
        let name = identifier.name();
        if let Some(&declaration) = self.locals.get(&identifier.span().start) {
            return Some(Element::Local(declaration));
        }
        match self.enclosing_member_owner(name) {
            Some(Some(owner)) => {
                // A field whose type mentions its class's type parameters is
                // referenced through a fresh substituted member each time, so
                // two statements never share its element.
                if self
                    .declared_member_type(name)
                    .is_some_and(|ty| mentions_type_parameter(&ty))
                {
                    return None;
                }
                return Some(Element::Property {
                    name,
                    owner: Owner::Type(owner),
                });
            }
            // Possibly inherited through an unresolvable supertype; only a
            // resolvable top-level declaration is taken at face value.
            Some(None)
                if self.state.model.resolve_value(name).is_none()
                    && self.state.model.resolve_name(name).is_none() =>
            {
                return None;
            }
            _ => {}
        }
        if self.prefixes.contains(name) {
            return Some(Element::Type(name));
        }
        if let Some(identity) = self.state.model.resolve_value(name) {
            return self
                .state
                .signatures
                .function(&identity)
                .is_none()
                .then(|| Element::Property {
                    name,
                    owner: Owner::Library(Library::of(&identity)),
                });
        }
        self.state
            .model
            .resolve_name(name)
            .map(|_| Element::Type(name))
    }

    /// The declaring type of the enclosing type's member `name`: `None` when
    /// there is no enclosing type or no such member, `Some(None)` when the
    /// hierarchy is unresolvable.
    fn enclosing_member_owner(&self, name: &str) -> Option<Option<DeclarationIdentity>> {
        match self.state.enclosing_type.as_ref()?.view() {
            ResolvedTypeView::Interface { identity, .. } => member_owner(identity, name, self.state),
            _ => None,
        }
    }

    /// Whether member `name` accessed on `object` (or unqualified, when
    /// `None`) may be declared by `owner`.
    fn member_of_owner(&self, object: Option<Expr<'a>>, name: &str, owner: &Owner) -> bool {
        let Owner::Type(owner) = owner else {
            // Members never live in a library.
            return false;
        };
        let receiver = match object {
            None => return self.enclosing_member_owner(name).flatten().as_ref() == Some(owner),
            Some(Expr::Super(_)) => {
                let Some(ResolvedTypeView::Interface { identity, .. }) = self.state.enclosing_type.as_ref().map(ResolvedType::view)
                else {
                    return true;
                };
                return match self.state.signatures.inherited_member_facts(identity, name) {
                    Some(facts) => facts.iter().any(|fact| fact.declaring_type == *owner),
                    None => true,
                };
            }
            Some(Expr::Ident(identifier))
                if self.prefixes.contains(identifier.name()) && !self.is_local(identifier) =>
            {
                return false;
            }
            Some(object) => self.static_type(object),
        };
        match receiver.as_ref().map(ResolvedType::view) {
            Some(ResolvedTypeView::Interface { identity, .. }) => {
                member_owner(identity, name, self.state).flatten().as_ref() == Some(owner)
            }
            Some(ResolvedTypeView::Dynamic) => false,
            // An unresolved receiver may be the owner itself.
            _ => true,
        }
    }

    /// Whether the invoked name resolves to a method rather than a callable
    /// field/getter. Dynamic receivers resolve only matching `Object` methods;
    /// `call` on a function has no method element.
    fn invokes_method(&self, object: Expr<'a>, name: &str, args: ArgList<'a>) -> bool {
        let inferred = self.static_type(object);
        let mut receiver = inferred.as_ref();
        while let Some(
            ResolvedTypeView::TypeParameter { bound, .. }
            | ResolvedTypeView::NonNullTypeParameter { bound, .. },
        ) = receiver.map(ResolvedType::view)
        {
            receiver = Some(bound);
        }
        match receiver.map(ResolvedType::view) {
            Some(ResolvedTypeView::Never | ResolvedTypeView::Unknown) | None => false,
            Some(ResolvedTypeView::Dynamic) => {
                let parameters = match name {
                    "toString" => 0,
                    "noSuchMethod" => 1,
                    _ => return false,
                };
                args.named().is_empty() && args.positional().count() == parameters
            }
            Some(ResolvedTypeView::Function { .. }) => name != "call",
            Some(ResolvedTypeView::Record {
                positional, named, ..
            }) => {
                !named.iter().any(|(field, _)| field == name)
                    && !name
                        .strip_prefix('$')
                        .and_then(|index| index.parse::<usize>().ok())
                        .is_some_and(|index| (1..=positional.len()).contains(&index))
            }
            Some(_) if receiver.is_some_and(|ty| ty.interface("dart:core", "Function")) => name != "call",
            Some(ResolvedTypeView::Interface { .. }) => receiver.is_some_and(|receiver| {
                self.state
                    .signatures
                    .resolved_member_facts(receiver, name, &self.state.model)
                    .is_none_or(|facts| {
                        !facts.iter().any(|fact| {
                            matches!(fact.kind, MemberKind::Field { .. } | MemberKind::Getter)
                        })
                    })
            }),
            _ => false,
        }
    }

    /// The type of the enclosing type's field or getter `name` as declared,
    /// in terms of its owner's type parameters.
    fn declared_member_type(&self, name: &str) -> Option<ResolvedType> {
        let enclosing = self.state.enclosing_type.as_ref()?;
        self.state
            .signatures
            .resolved_field(enclosing, name, &self.state.model)
            .map(|(ty, _)| ty)
            .filter(|ty| !matches!(ty.view(), ResolvedTypeView::Unknown))
            // An own field without a declared type has its initializer's type.
            .or_else(|| Some(self.state.environment.lookup(name)))
    }

    /// The type of a member access target; a type name stands for its static
    /// members' declaring type, and `super` for the enclosing type's
    /// inherited members.
    fn static_type(&self, object: Expr<'a>) -> Option<ResolvedType> {
        match object {
            Expr::Ident(identifier) if !self.is_local(identifier) => {
                let name = identifier.name();
                if matches!(self.enclosing_member_owner(name), Some(Some(_))) {
                    let enclosing = self.state.enclosing_type.as_ref()?;
                    return self
                        .state
                        .signatures
                        .resolved_field(enclosing, name, &self.state.model)
                        .map(|(ty, substitutions)| self.state.model.substitute(&ty, &substitutions))
                        .filter(|ty| !matches!(ty.view(), ResolvedTypeView::Unknown))
                        .or_else(|| Some(self.state.infer(object)));
                }
                if self.state.model.resolve_value(name).is_none()
                    && let Some(identity) = self.state.model.resolve_name(name)
                {
                    return Some(ResolvedType::new_interface(identity, Vec::new().into(), false, false));
                }
                Some(self.state.infer(object))
            }
            _ => Some(self.state.infer(object)),
        }
    }

    /// Summarizes critical `nodes` for a statement targeting `element`.
    fn critical(&self, nodes: &[Expr<'a>], element: Option<&Element<'a>>) -> Critical<'a> {
        let owner = match element {
            Some(Element::Property { owner, .. }) => Some(owner),
            _ => None,
        };
        let mut finder = Finder {
            scope: self,
            owner,
            critical: Critical::default(),
        };
        for &node in nodes {
            finder.visit_expr(node);
        }
        finder.critical
    }

    /// Whether the unqualified non-local `name` refers to a declaration owned
    /// by `owner`.
    fn unqualified_in_owner(&self, name: &str, owner: &Owner) -> bool {
        match self.enclosing_member_owner(name) {
            Some(Some(member_owner)) => *owner == Owner::Type(member_owner),
            // An inherited member of an unresolvable hierarchy, or a
            // top-level declaration: only the latter can be in a library.
            Some(None) => matches!(owner, Owner::Library(_)) && self.top_level_in(name, owner),
            None => self.top_level_in(name, owner),
        }
    }

    fn top_level_in(&self, name: &str, owner: &Owner) -> bool {
        let Owner::Library(library) = owner else {
            return false;
        };
        self.state
            .model
            .resolve_value(name)
            .is_some_and(|identity| Library::of(&identity) == *library)
    }
}

/// The declaring type of member `name` of `identity`: `None` when `identity`
/// has no such member, `Some(None)` when its hierarchy is unresolvable.
fn member_owner(
    identity: &DeclarationIdentity,
    name: &str,
    state: &SemanticState<'_>,
) -> Option<Option<DeclarationIdentity>> {
    if !state
        .signatures
        .declared_member_facts(identity, name)
        .is_empty()
    {
        return Some(Some(identity.clone()));
    }
    match state.signatures.inherited_member_facts(identity, name) {
        None => Some(None),
        Some(facts) => facts.first().map(|fact| Some(fact.declaring_type.clone())),
    }
}

fn for_each_argument<'a>(args: ArgList<'a>, mut f: impl FnMut(Expr<'a>)) {
    args.positional()
        .chain(args.named().map(|argument| argument.value()))
        .for_each(&mut f);
}

/// Summarizes critical nodes: identifiers, closures, and for property
/// receivers the references to declarations of the receiver's owner.
struct Finder<'f, 's, 'a> {
    scope: &'f Scope<'s, 'a>,
    owner: Option<&'f Owner>,
    critical: Critical<'a>,
}

impl<'a> Visitor<'a> for Finder<'_, '_, 'a> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        walk_expr_chain(self, node, |finder, node| {
            match node {
                Expr::Ident(identifier) => {
                    let declaration = finder.scope.locals.get(&identifier.span().start).copied();
                    finder.critical.names.push((identifier.name(), declaration));
                    if let Some(owner) = finder.owner
                        && declaration.is_none()
                    {
                        finder.critical.owner_referenced |=
                            finder.scope.unqualified_in_owner(identifier.name(), owner);
                    }
                }
                Expr::FuncExpr(_) => {
                    finder.critical.closure = true;
                    if finder.owner.is_some() {
                        return false;
                    }
                }
                Expr::Field(field) => {
                    if let Some(owner) = finder.owner {
                        finder.critical.owner_referenced |= finder.scope.member_of_owner(
                            Some(field.object()),
                            field.field().name(),
                            owner,
                        );
                    }
                }
                Expr::Cascade(cascade) => {
                    if let Some(owner) = finder.owner {
                        for op in cascade.sections().flat_map(|section| section.ops()) {
                            let name = match op {
                                CascadeOp::Field(field) => field.name(),
                                CascadeOp::Call(call) => call.name(),
                                _ => continue,
                            };
                            finder.critical.owner_referenced |=
                                finder.scope
                                    .member_of_owner(Some(cascade.object()), name.name(), owner);
                        }
                    }
                }
                _ => {}
            }
            true
        });
    }
}

fn check_statements(
    statements: AstChildren<'_, Stmt<'_>>,
    facts: &HashMap<usize, Cascadable<'_>>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    if statements.clone().count() < 2 {
        return;
    }
    let none = Cascadable::default();
    let mut previous = &none;
    let mut previous_end = None;
    let mut first_in_cascade: Option<usize> = None;
    for statement in statements {
        let current = facts.get(&statement.span().start).unwrap_or(&none);
        if current.compatible_with(previous) {
            first_in_cascade.get_or_insert(statement.span().start);
        } else if let (Some(start), Some(end)) = (first_in_cascade.take(), previous_end) {
            report(start, end, ctx, diags);
        }
        previous = current;
        previous_end = Some(statement.span().end);
    }
    if let (Some(start), Some(end)) = (first_in_cascade, previous_end) {
        report(start, end, ctx, diags);
    }
}

fn report(start: usize, end: usize, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    diags.push(ctx.diagnostic("Avoid repeating the receiver.", DiagSpan { start, end }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_analyze::{ParsedFile};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;
    use std::sync::Arc;

    fn diagnostics(source: &str, imported: &str) -> Vec<Diagnostic> {
        let path = Path::new("/project/lib/main.dart");
        let imported_path = Path::new("/project/lib/output.dart");
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let parsed_output = parse(imported);
        assert!(
            parsed_output.errors.is_empty(),
            "{:?}",
            parsed_output.errors
        );
        let parsed_output = Arc::new(ParsedFile {
            source: imported.into(),
            tree: parsed_output.tree,
        });
        let files = [(path, &parsed), (imported_path, &parsed_output)];
        let indexes = crate::lint::test_support::TestIndexes::new(&files);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        CascadeInvocations.analyze(CascadeInvocations::NAME, program, &ctx)
    }

    #[test]
    fn imported_getters_and_inferred_variables_resolve_receivers() {
        let source = "import 'output.dart'; void run() {
            output.write('a'); output.write('b');
            final file = testFile; final out = file.openWrite();
            out.write('a'); out.write('b');
        }";
        let imported = "class Sink { void write(Object value) {} }
            class File { Sink openWrite() => Sink(); }
            Sink get output => Sink();
            final testFile = laterFile;
            final laterFile = File();";
        assert_eq!(diagnostics(source, imported).len(), 2);
    }

    #[test]
    fn cascade_initializer_preserves_implicit_method_resolution() {
        let source = "class Parsed {
            String root = ''; void normalize() {} void trim() {}
        }
        class Context {
            Parsed parse(String path) => Parsed();
            void run() {
                final parsed = parse('a')..normalize();
                print(parsed);
                parsed.root = '';
                parsed.trim();
            }
        }";
        assert_eq!(diagnostics(source, "").len(), 1);
    }

    #[test]
    fn static_getter_resolves_before_class_literal_type() {
        let source = "import 'output.dart'; void run() {
            final messenger = Binding.instance.defaultBinaryMessenger;
            messenger.setMockMethodCallHandler('a');
            messenger.setMockStreamHandler('b');
        }";
        let imported = "class Messenger {
            void setMockMethodCallHandler(String value) {}
            void setMockStreamHandler(String value) {}
        }
        class BaseBinding {
            Messenger get defaultBinaryMessenger => Messenger();
        }
        class Binding extends BaseBinding {
            static Binding get instance => Binding();
        }";
        assert_eq!(diagnostics(source, imported).len(), 1);
    }

    #[test]
    fn unresolved_and_callable_receivers_remain_silent() {
        let source = "import 'output.dart'; void run() {
            final unknown = missing(); unknown.write('a'); unknown.write('b');
            final dynamicValue = dynamicOutput;
            dynamicValue.write('a'); dynamicValue.write('b');
            final sink = callableOutput;
            sink.write('a'); sink.write('b');
        }";
        let imported = "dynamic get dynamicOutput => null;
            class CallableSink { final void Function(String) write = (value) {}; }
            CallableSink get callableOutput => CallableSink();";
        assert!(diagnostics(source, imported).is_empty());
    }
}
