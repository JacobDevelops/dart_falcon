//! Avoid `Type.toString()` in production code, where type names may be minified.
//!
//! Web and AOT builds may minify or strip type names, so a string built from a
//! `Type` can differ between debug and release:
//!
//! ```dart
//! class TypeName implements Type {}
//! final name = TypeName().toString(); // BAD: uses `Object.toString`
//! if (value is Widget) {} // GOOD: compare types, not their names
//! ```
//!
//! Adopted from the official `avoid_type_to_string` lint, including its
//! quirks. A `toString` invocation (cascade sections included) or a `toString`
//! tear-off passed as an argument is reported when it resolves to
//! `Object.toString` and its target type is a non-nullable subtype of
//! `dart:core` `Type`. The target type is the receiver's static type when that
//! is an interface type; otherwise (no receiver, `dynamic`, a type parameter)
//! it is the type of the most recent class, mixin, or extension declaration
//! in the file — the official visitor never resets it, so it carries over into
//! later functions, enums, and extension types. A tear-off through a simple
//! identifier prefix (`t.toString`) uses only an interface-typed prefix.
//! `Type` itself (and so every `runtimeType` read) is never reported, because
//! `dart:core` `Type` redeclares `toString`.

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidTypeToString;

impl Rule for AvoidTypeToString {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = falcon_analyze::SemanticModel::new(
            ctx.file_path(),
            ctx.parsed(),
            identities,
            ctx.types(),
        );
        let stale_this = program
            .declarations()
            .filter_map(|declaration| {
                let ty = match declaration {
                    TopLevelDecl::Class(class) => this_type(class.name(), &model),
                    TopLevelDecl::MixinClass(class) => this_type(class.name(), &model),
                    TopLevelDecl::Mixin(mixin) => this_type(mixin.name(), &model),
                    TopLevelDecl::Extension(extension) => {
                        Some(model.resolve_type(extension.on_type()))
                            .filter(|ty| matches!(ty, ResolvedType::Interface { .. }))
                    }
                    _ => return None,
                };
                Some((declaration.span().start, ty))
            })
            .collect();
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            stale_this,
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

fn this_type(
    name: Identifier<'_>,
    model: &falcon_analyze::SemanticModel<'_>,
) -> Option<ResolvedType> {
    model
        .resolve_name(name.name())
        .map(|identity| ResolvedType::Interface {
            occurrence: Default::default(),
            identity,
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        })
}

/// What the `toString` reference resolves against.
enum Lookup<'t> {
    /// A member of this static type.
    Member(&'t ResolvedType),
    /// `super.toString`: the concrete member inherited by the enclosing type.
    Super,
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// The official visitor's `thisType` after each class, mixin, and
    /// extension declaration (by start offset): `None` for an extension on a
    /// non-interface type.
    stale_this: Vec<(usize, Option<ResolvedType>)>,
}

impl<'a> Collector<'a> {
    fn stale_this(&self, offset: usize) -> Option<&ResolvedType> {
        let after = self
            .stale_this
            .partition_point(|(start, _)| *start <= offset);
        after
            .checked_sub(1)
            .and_then(|index| self.stale_this[index].1.as_ref())
    }

    fn check(
        &mut self,
        name: Identifier<'a>,
        lookup: Lookup<'_>,
        target: Option<&ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        if name.name() != "toString"
            || !target.is_some_and(|target| is_type_subtype(target, state))
            || !resolves_to_object_to_string(lookup, state)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("Using 'toString' on a 'Type' is not safe in production code.", name.span()));
    }

    /// `receiver.name`, invoked or (`prefixed_only`) torn off through a
    /// simple identifier prefix.
    fn check_receiver(
        &mut self,
        receiver: Expr<'a>,
        name: Identifier<'a>,
        prefixed_only: bool,
        state: &SemanticState<'a>,
    ) {
        let is_super = matches!(receiver.unparenthesized(), Expr::Super(_));
        let receiver_type = match receiver.unparenthesized() {
            Expr::This(_) | Expr::Super(_) => state
                .enclosing_type
                .clone()
                .unwrap_or(ResolvedType::Unknown),
            other => state.infer(other),
        };
        if matches!(receiver_type, ResolvedType::Unknown) {
            return;
        }
        let target = if matches!(receiver_type, ResolvedType::Interface { .. }) {
            Some(&receiver_type)
        } else if prefixed_only {
            return;
        } else {
            self.stale_this(name.span().start)
        }
        .cloned();
        let lookup = if is_super {
            Lookup::Super
        } else {
            Lookup::Member(&receiver_type)
        };
        self.check(name, lookup, target.as_ref(), state);
    }

    /// An unqualified `toString`, resolved against implicit `this`.
    fn check_implicit_this(&mut self, name: Identifier<'a>, state: &SemanticState<'a>) {
        if state.environment.is_bound(name.name()) {
            return;
        }
        let Some(enclosing) = state.enclosing_type.as_ref() else {
            return;
        };
        let target = self.stale_this(name.span().start).cloned();
        self.check(name, Lookup::Member(enclosing), target.as_ref(), state);
    }

    fn check_arguments(&mut self, args: ArgList<'a>, state: &SemanticState<'a>) {
        for argument in args
            .positional()
            .chain(args.named().map(|argument| argument.value()))
        {
            match argument.unparenthesized() {
                Expr::Field(field) => {
                    let prefixed = !field.is_null_safe()
                        && matches!(field.object().unparenthesized(), Expr::Ident(_));
                    self.check_receiver(field.object(), field.field(), prefixed, state);
                }
                Expr::Ident(identifier) => self.check_implicit_this(identifier, state),
                _ => {}
            }
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => {
                self.check_arguments(call.args(), state);
                match call.callee().unparenthesized() {
                    Expr::Field(field) => {
                        self.check_receiver(field.object(), field.field(), false, state);
                    }
                    Expr::Ident(identifier) => self.check_implicit_this(identifier, state),
                    _ => {}
                }
            }
            Expr::New(new) => self.check_arguments(new.args(), state),
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    for (index, op) in section.ops().enumerate() {
                        match op {
                            CascadeOp::Call(call) => {
                                self.check_arguments(call.args(), state);
                                // Later links target the previous link's result.
                                if index == 0 {
                                    self.check_receiver(
                                        cascade.object(),
                                        call.name(),
                                        false,
                                        state,
                                    );
                                }
                            }
                            CascadeOp::Invoke(invoke) => self.check_arguments(invoke.args(), state),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// The analyzer's `isSubtypeOf(target, Type)`: nullable types never are.
fn is_type_subtype(target: &ResolvedType, state: &SemanticState<'_>) -> bool {
    matches!(
        target,
        ResolvedType::Interface {
            nullable: false,
            ..
        }
    ) && state.signatures.implements_sdk(target, "dart:core", "Type") == Truth::Yes
}

/// Whether the `toString` reference's element is `Object.toString`.
fn resolves_to_object_to_string(lookup: Lookup<'_>, state: &SemanticState<'_>) -> bool {
    let signatures = &state.signatures;
    // Every type has `toString`: finding none means `Object`'s is unindexed.
    let is_object = |fact: Option<Option<&falcon_analyze::MemberFacts>>| match fact {
        Some(Some(fact)) => fact.declaring_type == DeclarationIdentity::OBJECT,
        Some(None) => true,
        None => false,
    };
    let ty = match lookup {
        Lookup::Super => {
            return match &state.enclosing_type {
                Some(ResolvedType::Interface { identity, .. }) => {
                    is_object(signatures.inherited_concrete_member(identity, "toString"))
                }
                _ => false,
            };
        }
        Lookup::Member(ty) => ty,
    };
    match ty {
        ResolvedType::Dynamic => true,
        ResolvedType::TypeParameter { bound, .. } => {
            resolves_to_object_to_string(Lookup::Member(bound), state)
        }
        ResolvedType::Interface { identity, .. } => {
            // `dart:core` `Type` redeclares `toString`.
            if identity.is_sdk("dart:core", "Type") {
                return false;
            }
            if matches!(
                signatures.type_kind(identity),
                Some(TypeKind::Mixin | TypeKind::ExtensionType)
            ) {
                // No concrete superclass: the interface member wins.
                return is_object(signatures.interface_member(identity, identity, "toString"));
            }
            let owner_is_object = signatures
                .resolved_member_owner(ty, "toString", &state.model)
                .is_some_and(|owner| owner == DeclarationIdentity::OBJECT);
            // Abstract redeclarations (such as `Type.toString`) leave the
            // concrete implementation, which is the resolved element, unchanged.
            owner_is_object
                && signatures
                    .inherited_member_facts(identity, "toString")
                    .is_some_and(|facts| {
                        facts
                            .iter()
                            .filter(|fact| !fact.is_abstract)
                            .all(|fact| fact.declaring_type == DeclarationIdentity::OBJECT)
                    })
        }
        _ => false,
    }
}
