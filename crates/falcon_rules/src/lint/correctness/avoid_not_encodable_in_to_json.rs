//! Reports non-JSON-encodable values returned by `toJson` methods.
//!
//! Checks returned maps and lists recursively, including collection branches, and
//! direct returns. Primitive values, null, and objects with an inherited or
//! declared instance `toJson` method are accepted. Unknown types, incomplete
//! supertype graphs, and broad object types are left alone because their runtime
//! values may be encodable.
//!
//! # Options
//!
//! `methods` adds serialization method names to check (default: empty).

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, FunctionBodies, ResolvedType, ResolvedTypeView, Rule,
    RulePreparation, SemanticRuleVisitor, SemanticState, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use rustc_hash::FxHashMap;

use crate::lint::options;

#[derive(Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Options {
    /// Additional serialization methods to check alongside `toJson`.
    pub methods: Vec<String>,
}

options::impl_options!(Options);

#[derive(Default)]
pub struct AvoidNotEncodableInToJson;

impl Rule for AvoidNotEncodableInToJson {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let config = options::get::<Options>(ctx, Self::NAME);
        let mut methods = Vec::new();
        let mut pending = FxHashMap::default();
        for declaration in program.declarations() {
            let members = match declaration {
                TopLevelDecl::Class(node) => node.members(),
                TopLevelDecl::MixinClass(node) => node.members(),
                TopLevelDecl::Mixin(node) => node.members(),
                TopLevelDecl::Enum(node) => node.members(),
                TopLevelDecl::Extension(node) => node.members(),
                TopLevelDecl::ExtensionType(node) => node.members(),
                _ => continue,
            };
            for member in members {
                if let ClassMember::Method(method) = member
                    && (method.name().name() == "toJson"
                        || config
                            .methods
                            .iter()
                            .any(|name| name == method.name().name()))
                    && let Some(body) = method.body()
                {
                    methods.push(body.span());
                    if let FunctionBody::Arrow(arrow) = body {
                        pending.insert(arrow.expr().span(), Position::Value);
                    }
                }
            }
        }
        if methods.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            bodies: ctx.function_bodies(program),
            methods,
            pending,
            collection_work: Vec::new(),
            map_work: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    bodies: &'a FunctionBodies<'a>,
    methods: Vec<Span>,
    pending: FxHashMap<Span, Position>,
    collection_work: Vec<CollectionElement<'a>>,
    map_work: Vec<MapElement<'a>>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    Value,
    Key,
    Spread,
}

impl<'a> Collector<'a> {
    fn collection(&mut self, elements: impl Iterator<Item = CollectionElement<'a>>) {
        self.collection_work.extend(elements);
        while let Some(element) = self.collection_work.pop() {
            match element {
                CollectionElement::Expr(value) => {
                    self.pending.insert(value.span(), Position::Value);
                }
                CollectionElement::NullAware(value) => {
                    self.pending.insert(value.expr().span(), Position::Value);
                }
                CollectionElement::Spread(value) => {
                    self.pending.insert(value.expr().span(), Position::Spread);
                }
                CollectionElement::If(branch) => {
                    self.collection_work.push(branch.then_elem());
                    self.collection_work.extend(branch.else_elem());
                }
                CollectionElement::For(element) => self.collection_work.push(element.element()),
            }
        }
    }

    fn map(&mut self, elements: impl Iterator<Item = MapElement<'a>>) {
        self.map_work.extend(elements);
        while let Some(element) = self.map_work.pop() {
            match element {
                MapElement::Entry(entry) => {
                    self.pending.insert(entry.key().span(), Position::Key);
                    self.pending.insert(entry.value().span(), Position::Value);
                }
                MapElement::Spread(value) => {
                    self.pending.insert(value.expr().span(), Position::Value);
                }
                MapElement::If(branch) => {
                    self.map_work.push(branch.then_entry());
                    self.map_work.extend(branch.else_entry());
                }
                MapElement::For(element) => self.map_work.push(element.entry()),
            }
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        if let Stmt::Return(statement) = node
            && let Some(body) = self.bodies.enclosing(&node.span())
            && self.methods.contains(&body.span)
            && let Some(value) = statement.value()
        {
            self.pending.insert(value.span(), Position::Value);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some(position) = self.pending.remove(&node.span()) else {
            return;
        };
        let value = node.unparenthesized();
        if position != Position::Key {
            match value {
                Expr::Map(map) => {
                    self.map(map.elements());
                    return;
                }
                Expr::List(list) => {
                    self.collection(list.elements());
                    return;
                }
                Expr::Conditional(branch) => {
                    self.pending.insert(branch.then_expr().span(), position);
                    self.pending.insert(branch.else_expr().span(), position);
                    return;
                }
                Expr::Switch(branch) => {
                    self.pending
                        .extend(branch.arms().map(|arm| (arm.body().span(), position)));
                    return;
                }
                _ => {}
            }
        }
        let ty = state.infer(value);
        let invalid = match position {
            Position::Key => invalid_key(&ty, state),
            Position::Value => not_encodable(&ty, state, 0),
            Position::Spread => state
                .signatures
                .instantiated_supertype(&ty, "dart:core", "Iterable", &state.model)
                .is_some_and(|iterable| {
                    iterable
                        .arguments()
                        .first()
                        .is_some_and(|ty| not_encodable(ty, state, 0))
                }),
        };
        if invalid {
            self.diagnostics.push(self.ctx.diagnostic(
                if position == Position::Key {
                    "JSON map keys must be strings."
                } else {
                    "This expression is not JSON-encodable; convert it or provide an instance `toJson` method."
                },
                value.span(),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

fn invalid_key(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    match ty.view() {
        ResolvedTypeView::Interface {
            extension_type: true,
            ..
        } => state
            .signatures
            .extension_type_erasure(ty, &state.model)
            .is_some_and(|erased| invalid_key(&erased, state)),
        ResolvedTypeView::Interface { identity, .. } => {
            !identity.is_sdk("dart:core", "String") && !identity.is_sdk("dart:core", "Object")
        }
        ResolvedTypeView::Function { .. }
        | ResolvedTypeView::Record { .. }
        | ResolvedTypeView::Null => true,
        _ => false,
    }
}

fn not_encodable(ty: &ResolvedType, state: &SemanticState<'_>, depth: usize) -> bool {
    if depth >= 32 {
        return false;
    }
    match ty.view() {
        ResolvedTypeView::Function { .. } | ResolvedTypeView::Record { .. } => true,
        ResolvedTypeView::Interface {
            identity,
            arguments,
            extension_type,
            ..
        } => {
            if extension_type {
                return state
                    .signatures
                    .extension_type_erasure(ty, &state.model)
                    .is_some_and(|erased| not_encodable(&erased, state, depth + 1));
            }
            if identity.is_sdk("dart:core", "Object") {
                return false;
            }
            if ["String", "bool", "int", "double", "num", "Null"]
                .iter()
                .any(|name| identity.is_sdk("dart:core", name))
            {
                return false;
            }
            if identity.is_sdk("dart:core", "List") {
                return arguments
                    .first()
                    .is_some_and(|ty| not_encodable(ty, state, depth + 1));
            }
            if identity.is_sdk("dart:core", "Map") {
                return arguments.first().is_some_and(|ty| invalid_key(ty, state))
                    || arguments
                        .get(1)
                        .is_some_and(|ty| not_encodable(ty, state, depth + 1));
            }
            for name in ["List", "Map"] {
                if let Some(collection) =
                    state
                        .signatures
                        .instantiated_supertype(ty, "dart:core", name, &state.model)
                {
                    return not_encodable(&collection, state, depth + 1);
                }
            }
            if state
                .signatures
                .resolved_member_owner(ty, "toJson", &state.model)
                .is_some()
                && state
                    .signatures
                    .resolved_member(ty, "toJson", &state.model)
                    .is_some_and(|(signature, _)| {
                        signature.positional.len() == signature.optional_positional
                            && signature
                                .shape
                                .as_ref()
                                .is_none_or(|shape| shape.required_named.is_empty())
                    })
            {
                return false;
            }
            [
                "DateTime", "Duration", "Set", "Iterable", "Function", "Symbol", "Type", "RegExp",
                "Uri", "BigInt",
            ]
            .iter()
            .any(|name| identity.is_sdk("dart:core", name))
                || ["Future", "Stream"]
                    .iter()
                    .any(|name| identity.is_sdk("dart:async", name))
                // A negative hierarchy proof requires complete local or loaded package facts.
                || (state.signatures.declaration(identity).is_some()
                    && state.signatures.relation(
                        ty,
                        TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::sdk(
                            "dart:core",
                            "List",
                        )),
                        &state.model,
                    ) == Truth::No)
        }
        _ => false,
    }
}
