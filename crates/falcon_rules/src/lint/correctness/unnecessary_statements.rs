//! Flags expression statements whose expression has no clear effect.
//!
//! A statement such as `x;`, `1 + 2;`, `list.clear;` or `c ? a : b;` computes a
//! value and throws it away, which usually means the author forgot a call, an
//! assignment, or a `return`. Expressions with a clear effect are allowed:
//! assignments, invocations, instance creations, `await`, `throw`, `rethrow`,
//! cascades, `as` casts, postfix operators (`x++`, `x!`) and prefix `++`/`--`.
//! The right operand of `&&`, `||`, and `??`, and both branches of a
//! conditional, are checked in turn, since those operators are used for control
//! flow. A parenthesized expression is always reported, as are binary
//! operations, index reads, literals, and `is` tests.
//!
//! Identifiers and resolved property reads are reported unless they denote an
//! explicitly declared getter, which may have side effects. Dynamic reads and
//! unresolved identifiers are reported too. Applicable extension getters are
//! allowed, while property reads of unavailable external members are
//! conservatively left alone. The same checks apply to the initializer and
//! updaters of a `for (;;)` loop with an expression initializer. A cascade
//! section that reads a method or function-valued member (`..clear`) is reported.
//! Constructor and generic function tear-offs are references without an effect.
//! Reading a function's synthetic `call` member is also a tear-off, including
//! null-aware reads and cascade sections; invoking it still has an effect.
//!
//! ```dart
//! void f(List<int> list, int a, int b) {
//!   a;          // flagged
//!   list.clear; // flagged
//!   a + b;      // flagged
//!   list.clear(); // ok
//! }
//! ```
//!
//! Adopted from the official `unnecessary_statements` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, MemberKind, MemberResult, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

const RULE: &str = "unnecessary-statements";
const MESSAGE: &str = "Unnecessary statement.";

#[derive(Default)]
pub struct UnnecessaryStatements;

impl Rule for UnnecessaryStatements {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            declarations: Declarations::new(program),
            diagnostics: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

/// What reading a name as a statement means for this rule.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// A variable, field, tear-off, or type: no effect, report.
    None,
    /// An explicitly declared getter: allowed.
    Getter,
}

impl Effect {
    fn of_kind(kind: MemberKind) -> Option<Self> {
        match kind {
            MemberKind::Getter => Some(Self::Getter),
            MemberKind::Field { .. } | MemberKind::Method => Some(Self::None),
            MemberKind::Setter | MemberKind::Operator => None,
        }
    }

    /// Combines the getter/setter halves of one name: a getter wins, a lone
    /// setter says nothing about reading the name.
    fn merge(existing: Option<Self>, next: Self) -> Self {
        match existing {
            Some(Self::Getter) => Self::Getter,
            _ => next,
        }
    }
}

/// A member of a type declared in this file.
#[derive(Clone, Copy)]
struct Member {
    effect: Effect,
    is_static: bool,
}

/// Top-level and type-member declarations of the current file.
struct Declarations {
    top_level: HashMap<String, Effect>,
    types: HashMap<String, HashMap<String, Member>>,
}
impl Declarations {
    fn new(program: &Program) -> Self {
        let mut top_level = HashMap::new();
        let mut types = HashMap::new();
        for declaration in &program.declarations {
            let (name, members, values): (&Identifier, &[ClassMember], &[EnumVariant]) =
                match declaration {
                    TopLevelDecl::Variable(variable) => {
                        for declarator in &variable.declarators {
                            top_level.insert(declarator.name.name.clone(), Effect::None);
                        }
                        continue;
                    }
                    TopLevelDecl::Function(function) => {
                        let effect = match function.kind {
                            FunctionKind::Function(_) => Effect::None,
                            FunctionKind::Getter => Effect::Getter,
                            FunctionKind::Setter(_) => continue,
                        };
                        let existing = top_level.get(&function.name.name).copied();
                        top_level
                            .insert(function.name.name.clone(), Effect::merge(existing, effect));
                        continue;
                    }
                    TopLevelDecl::TypeAlias(alias) => {
                        top_level.insert(alias.name.name.clone(), Effect::None);
                        continue;
                    }
                    TopLevelDecl::ClassTypeAlias(alias) => {
                        top_level.insert(alias.name.name.clone(), Effect::None);
                        continue;
                    }
                    TopLevelDecl::Class(class) => (&class.name, &class.members, &[]),
                    TopLevelDecl::MixinClass(class) => (&class.name, &class.members, &[]),
                    TopLevelDecl::Mixin(mixin) => (&mixin.name, &mixin.members, &[]),
                    TopLevelDecl::Enum(enumeration) => (
                        &enumeration.name,
                        &enumeration.members,
                        &enumeration.variants,
                    ),
                    TopLevelDecl::ExtensionType(extension) => {
                        (&extension.name, &extension.members, &[])
                    }
                    TopLevelDecl::Extension(_) | TopLevelDecl::Error(_) => continue,
                };
            top_level.insert(name.name.clone(), Effect::None);
            let primary = match declaration {
                TopLevelDecl::Class(class) => Some(class.element_members()),
                TopLevelDecl::MixinClass(class) => Some(class.element_members()),
                TopLevelDecl::Enum(enumeration) => Some(enumeration.element_members()),
                TopLevelDecl::ExtensionType(extension) => Some(extension.element_members()),
                _ => None,
            };
            let additional_fields = primary.map_or(0, |primary| primary.declaring_params().count());
            let mut members = type_members(members, values, additional_fields);
            if let Some(primary) = primary {
                for param in primary.declaring_params() {
                    members.insert(
                        param.name.name.clone(),
                        Member {
                            effect: Effect::None,
                            is_static: false,
                        },
                    );
                }
            }
            types.insert(name.name.clone(), members);
        }
        Self { top_level, types }
    }
}

fn type_members(
    members: &[ClassMember],
    values: &[EnumVariant],
    additional_fields: usize,
) -> HashMap<String, Member> {
    let mut map = HashMap::with_capacity(members.len() + values.len() + additional_fields);
    for value in values {
        map.insert(
            value.name.name.clone(),
            Member {
                effect: Effect::None,
                is_static: true,
            },
        );
    }
    for member in members {
        let (name, effect, is_static) = match member {
            ClassMember::Field(field) => {
                for declarator in &field.declarators {
                    map.insert(
                        declarator.name.name.clone(),
                        Member {
                            effect: Effect::None,
                            is_static: field.is_static,
                        },
                    );
                }
                continue;
            }
            ClassMember::Method(method) => (&method.name, Effect::None, method.is_static),
            ClassMember::Getter(getter) => (&getter.name, Effect::Getter, getter.is_static),
            // Named constructors tear off a function without invoking it.
            ClassMember::Constructor(constructor) => match &constructor.constructor_name {
                Some(name) => (name, Effect::None, true),
                None => continue,
            },
            ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Setter(_)
            | ClassMember::Operator(_)
            | ClassMember::Error(_) => continue,
        };
        let existing = map.get(&name.name).map(|member: &Member| member.effect);
        map.insert(
            name.name.clone(),
            Member {
                effect: Effect::merge(existing, effect),
                is_static,
            },
        );
    }
    map
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: Declarations,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        match node {
            Stmt::Expr(statement) => self.check(&statement.expr, state),
            Stmt::For(statement)
                if matches!(statement.header.init, None | Some(ForInit::Exprs(_))) =>
            {
                if let Some(ForInit::Exprs(initializers)) = &statement.header.init {
                    for initializer in initializers {
                        self.check(initializer, state);
                    }
                }
                for update in &statement.header.updates {
                    self.check(update, state);
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Cascade {
            object, sections, ..
        } = node
        else {
            return;
        };
        let target = state.infer(object);
        for section in sections {
            let [CascadeOp::Field { name, .. }] = section.ops.as_slice() else {
                continue;
            };
            let facts =
                state
                    .signatures
                    .resolved_interface_member_facts(&target, &name.name, &state.model);
            let is_method = (name.name == "call"
                && matches!(target, ResolvedType::Function { .. }))
                || facts.is_some_and(|facts| {
                    facts
                        .iter()
                        .all(|fact| matches!(fact.kind, MemberKind::Method))
                })
                || (facts.is_none()
                    && self.index_member(&target, &name.name) == Some(MemberKind::Method));
            let is_function_value = state
                .signatures
                .resolved_field(&target, &name.name, &state.model)
                .or_else(|| {
                    state
                        .signatures
                        .resolved_extension_getter(&target, &name.name, &state.model)
                })
                .is_some_and(|(ty, _)| matches!(ty, ResolvedType::Function { .. }));
            if is_method || is_function_value {
                self.report(section.span);
            }
        }
    }
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics
            .push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }

    /// Reports `root` and, through `&&`/`||`/`??` right operands and
    /// conditional branches, the sub-expressions that have no clear effect.
    fn check(&mut self, root: &Expr, state: &SemanticState<'_>) {
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            let report = match expression {
                Expr::As { .. }
                | Expr::Assign { .. }
                | Expr::Await { .. }
                | Expr::Cascade { .. }
                | Expr::Call { .. }
                | Expr::New { .. }
                | Expr::PostfixIncDec { .. }
                | Expr::NullAssert { .. }
                | Expr::Throw { .. }
                | Expr::DotShorthand { .. }
                | Expr::Error { .. } => false,
                Expr::Binary {
                    op: BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce,
                    right,
                    ..
                } => {
                    pending.push(right);
                    false
                }
                Expr::Conditional {
                    then_expr,
                    else_expr,
                    ..
                } => {
                    pending.push(else_expr);
                    pending.push(then_expr);
                    false
                }
                Expr::Unary { op, .. } => !matches!(op, UnaryOp::PlusPlus | UnaryOp::MinusMinus),
                Expr::Ident(identifier) => {
                    self.identifier(&identifier.name, state) != Some(Effect::Getter)
                }
                Expr::Field { object, field, .. } => {
                    self.property(object, &field.name, state) == Some(Effect::None)
                }
                Expr::Binary { .. }
                | Expr::Paren { .. }
                | Expr::IntLit { .. }
                | Expr::DoubleLit { .. }
                | Expr::StringLit(_)
                | Expr::BoolLit { .. }
                | Expr::NullLit { .. }
                | Expr::SymbolLit { .. }
                | Expr::GenericInstantiation { .. }
                | Expr::This { .. }
                | Expr::Super { .. }
                | Expr::Is { .. }
                | Expr::Index { .. }
                | Expr::List { .. }
                | Expr::Map { .. }
                | Expr::Set { .. }
                | Expr::Record { .. }
                | Expr::FuncExpr { .. }
                | Expr::Switch { .. } => true,
            };
            if report {
                self.report(*expression.span());
            }
        }
    }

    /// What an unqualified name denotes, following Dart's lexical lookup:
    /// locals and the enclosing type's own members, then this file's
    /// top-level declarations, then inherited members.
    fn identifier(&self, name: &str, state: &SemanticState<'_>) -> Option<Effect> {
        if state.environment.is_bound(name) {
            return Some(Effect::None);
        }
        let enclosing = state.enclosing_type.as_ref();
        if let Some(member) = enclosing
            .and_then(|ty| self.own_members(ty))
            .and_then(|members| members.get(name))
        {
            return Some(member.effect);
        }
        if let Some(effect) = self.declarations.top_level.get(name) {
            return Some(*effect);
        }
        let enclosing = enclosing?;
        self.member_effect(enclosing, name, state)
    }

    /// What `object.name` denotes.
    fn property(&self, object: &Expr, name: &str, state: &SemanticState<'_>) -> Option<Effect> {
        // `C<T>.named` is a constructor reference, not a getter.
        if let Expr::GenericInstantiation { target, .. } = object
            && let Expr::Ident(identifier) = target.as_ref()
            && !state.environment.is_bound(&identifier.name)
            && (self.declarations.types.contains_key(&identifier.name)
                || state.model.resolve_name(&identifier.name).is_some())
        {
            return Some(Effect::None);
        }
        if let Expr::Ident(identifier) = object
            && !state.environment.is_bound(&identifier.name)
        {
            // A resolved type or import prefix can denote a static member.
            if self.declarations.top_level.get(&identifier.name) == Some(&Effect::None)
                && let Some(members) = self.declarations.types.get(&identifier.name)
            {
                if name == "new" {
                    return Some(Effect::None);
                }
                return members
                    .get(name)
                    .filter(|member| member.is_static)
                    .map(|member| member.effect);
            }
            if let Some(identity) = state.model.resolve_name(&identifier.name)
                && (name == "new" || state.signatures.constructor(&identity, name).is_some())
            {
                return Some(Effect::None);
            }
            if let Some(identity) = state.model.resolve_name(&identifier.name) {
                let receiver = ResolvedType::interface_identity(identity);
                if let Some(facts) =
                    state
                        .signatures
                        .resolved_member_facts(&receiver, name, &state.model)
                {
                    return facts
                        .iter()
                        .filter(|fact| fact.is_static)
                        .filter_map(|fact| Effect::of_kind(fact.kind))
                        .fold(None, |effect, next| Some(Effect::merge(effect, next)));
                }
            }
            let is_value = self.declarations.top_level.get(&identifier.name) == Some(&Effect::None)
                && !self.declarations.types.contains_key(&identifier.name);
            if !is_value && state.model.resolve_name(&identifier.name).is_none() {
                return Some(Effect::None);
            }
        }
        let receiver = state.infer(object);
        self.member_effect(&receiver, name, state)
    }

    /// The effect of reading instance member `name` on `receiver`.
    fn member_effect(
        &self,
        receiver: &ResolvedType,
        name: &str,
        state: &SemanticState<'_>,
    ) -> Option<Effect> {
        // Function types have a synthetic `call` method, not a getter or an
        // interface declaration that member lookup can discover.
        if name == "call" && matches!(receiver, ResolvedType::Function { .. }) {
            return Some(Effect::None);
        }
        if let Some(facts) =
            state
                .signatures
                .resolved_interface_member_facts(receiver, name, &state.model)
        {
            let mut effect = None;
            for fact in facts {
                if fact.is_static {
                    continue;
                }
                if let Some(next) = Effect::of_kind(fact.kind) {
                    effect = Some(Effect::merge(effect, next));
                }
            }
            return effect;
        }
        if matches!(receiver, ResolvedType::Dynamic) {
            return Some(Effect::None);
        }
        let effect = self.index_member(receiver, name).and_then(Effect::of_kind);
        if effect.is_some() {
            return effect;
        }
        // An applicable extension getter may have side effects.
        if state
            .signatures
            .has_possible_extension_getter(receiver, name, &state.model)
        {
            return Some(Effect::Getter);
        }
        match receiver {
            ResolvedType::Interface { identity, .. }
                if state.signatures.declaration(identity).is_some() =>
            {
                Some(Effect::None)
            }
            _ => None,
        }
    }

    /// The member kind recorded by the type index for an interface receiver.
    fn index_member(&self, receiver: &ResolvedType, name: &str) -> Option<MemberKind> {
        let ResolvedType::Interface { identity, .. } = receiver else {
            return None;
        };
        match self.ctx.types?.member_lookup_identity(identity, name) {
            MemberResult::Found(kind) => Some(kind),
            MemberResult::Absent | MemberResult::Unknown => None,
        }
    }

    /// Members declared in this file by the enclosing type.
    fn own_members(&self, enclosing: &ResolvedType) -> Option<&HashMap<String, Member>> {
        let ResolvedType::Interface { identity, .. } = enclosing else {
            return None;
        };
        self.declarations.types.get(identity.name())
    }
}
