//! Flags potentially throwing element access on SDK collections.
//!
//! Iterable first/last/single (including object-pattern getters), predicate
//! searches without an `orElse` fallback, elementAt, and List indexing can
//! throw when an element does not exist. The max/min extension getters are
//! included only when their resolved extension comes from package:collection.
//! Prefer nullable alternatives. Map indexing, unknown receivers, and custom
//! collection implementations are excluded. List indexing is safe in a resolved
//! Flutter ListView.builder/GridView.builder's direct, synchronous itemBuilder
//! body when itemCount is the same local list's length (including
//! `list?.length ?? 0`) and the index is its second parameter. Both bindings
//! must be unreassigned. The list must otherwise appear only as an index or
//! length receiver: aliases, helper arguments, method calls, setters and cascades
//! conservatively disable the exemption. Shadowed bindings and nested callbacks
//! do not inherit it. Other bounds/cardinality proofs are not attempted.

use std::collections::{HashMap, HashSet};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnsafeCollectionMethods;

impl Rule for AvoidUnsafeCollectionMethods {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
            escaped_lists: HashSet::new(),
            bounded_indexes: Vec::new(),
            declarations: falcon_analyze::resolve::lexical_usage::local_declarations(program),
            mutated: ctx.mutated_declarations(program),
            bodies: crate::lint::function_bodies::FunctionBodies::new(program),
            builders: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    escaped_lists: HashSet<NodeId>,
    bounded_indexes: Vec<(Span, NodeId)>,
    declarations: HashMap<usize, NodeId>,
    mutated: &'a HashSet<NodeId>,
    bodies: crate::lint::function_bodies::FunctionBodies<'a>,
    builders: Vec<BuilderBounds>,
}

struct BuilderBounds {
    body: Span,
    list: NodeId,
    index: NodeId,
}

fn iterable(ty: &ResolvedType) -> bool {
    ["Iterable", "List", "Set"]
        .iter()
        .any(|name| ty.interface("dart:core", name))
}

fn collection_extremum(receiver: &ResolvedType, name: &str, state: &SemanticState<'_>) -> bool {
    matches!(name, "max" | "min")
        && iterable(receiver)
        && state
            .signatures
            .resolved_extension_owner(receiver, name, &state.model)
            .is_some_and(|owner| {
                owner.is_package("collection", owner.name())
                    || state
                        .model
                        .declaration_uri(&owner)
                        .is_some_and(|uri| uri.starts_with("package:collection/"))
            })
}

impl Collector<'_> {
    fn binding(&self, expression: Expr<'_>) -> Option<NodeId> {
        let expression = expression.unparenthesized();
        let expression = match expression {
            Expr::NullAssert(assertion) => assertion.operand().unparenthesized(),
            _ => expression,
        };
        let Expr::Ident(identifier) = expression else {
            return None;
        };
        self.declarations.get(&identifier.span().start).copied()
    }

    fn record_builder(&mut self, expression: Expr<'_>, state: &SemanticState<'_>) {
        let Some((owner, "builder", args)) =
            crate::lint::flutter::scoped_constructor_invocation(expression, state)
        else {
            return;
        };
        if !owner.is_package("flutter", "ListView") && !owner.is_package("flutter", "GridView") {
            return;
        }
        let Some(count) = args.named().find(|arg| arg.name().name() == "itemCount") else {
            return;
        };
        let count = count.value().unparenthesized();
        let (length, nullable) = match count {
            Expr::Binary(binary)
                if binary.op() == BinaryOp::NullCoalesce
                    && matches!(binary.right().unparenthesized(), Expr::IntLit(zero) if zero.value() == "0") =>
            {
                (binary.left().unparenthesized(), true)
            }
            _ => (count, false),
        };
        let Expr::Field(length) = length else {
            return;
        };
        if length.field().name() != "length"
            || length.is_null_safe() != nullable
            || !state.infer(length.object()).interface("dart:core", "List")
        {
            return;
        }
        let Some(list) = self.binding(length.object()) else {
            return;
        };
        let Some(callback) = args.named().find(|arg| arg.name().name() == "itemBuilder") else {
            return;
        };
        let Expr::FuncExpr(callback) = callback.value().unparenthesized() else {
            return;
        };
        let parameters = callback.params().params();
        if parameters.len() != 2 || callback.async_marker() != AsyncMarker::Sync {
            return;
        }
        let Some(index) = parameters.last() else {
            return;
        };
        let index = index.name().syntax().id();
        if self.mutated.contains(&list) || self.mutated.contains(&index) {
            return;
        }
        self.builders.push(BuilderBounds {
            body: callback.body().span(),
            list,
            index,
        });
    }

    fn builder_index(&self, index: IndexExpr<'_>) -> Option<NodeId> {
        let body = self.bodies.enclosing(&index.span())?;
        self.builders.iter().find_map(|builder| {
            (builder.body == body.span
                && self.binding(index.object()) == Some(builder.list)
                && self.binding(index.index()) == Some(builder.index))
            .then_some(builder.list)
        })
    }

    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic("Collection access may throw — use a nullable lookup or an explicit fallback", span));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        match operation {
            CascadeOp::Field(field)
                if (iterable(receiver)
                    && matches!(field.name().name(), "first" | "last" | "single"))
                    || collection_extremum(receiver, field.name().name(), state) =>
            {
                self.report(field.name().span());
            }
            CascadeOp::Call(call) if iterable(receiver) => {
                let name = call.name().name();
                if matches!(
                    name,
                    "firstWhere" | "lastWhere" | "singleWhere" | "elementAt"
                ) && (name == "elementAt"
                    || !call
                        .args()
                        .named()
                        .any(|argument| argument.name().name() == "orElse"))
                {
                    self.report(call.name().span());
                }
            }
            CascadeOp::Index(index) if receiver.interface("dart:core", "List") => {
                self.report(index.span());
            }
            _ => {}
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        let indexes = std::mem::take(&mut self.bounded_indexes);
        for (span, list) in indexes {
            if self.escaped_lists.contains(&list) {
                self.report(span);
            }
        }
        self.diagnostics
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        _matched: Option<&ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        let Pattern::Object(pattern) = node else {
            return;
        };
        let receiver = state
            .model
            .resolve_type_in(pattern.type_(), &state.type_parameters);
        for field in pattern.fields() {
            let name = field.name().name();
            if (iterable(&receiver) && matches!(name, "first" | "last" | "single"))
                || collection_extremum(&receiver, name, state)
            {
                self.report(field.name().span());
            }
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        self.record_builder(node, state);
        match node {
            Expr::Ident(identifier) if !bounds_only_use(identifier) => {
                if let Some(binding) = self.declarations.get(&identifier.span().start) {
                    self.escaped_lists.insert(*binding);
                }
            }
            Expr::Field(field) => {
                let name = field.field().name();
                let receiver = state.infer(field.object());
                if (iterable(&receiver) && matches!(name, "first" | "last" | "single"))
                    || collection_extremum(&receiver, name, state)
                {
                    self.report(field.field().span());
                }
            }
            Expr::Call(call) => {
                let Expr::Field(field) = call.callee().unparenthesized() else {
                    return;
                };
                let name = field.field().name();
                if !matches!(
                    name,
                    "firstWhere" | "lastWhere" | "singleWhere" | "elementAt"
                ) || !iterable(&state.infer(field.object()))
                {
                    return;
                }
                if name != "elementAt"
                    && call
                        .args()
                        .named()
                        .any(|argument| argument.name().name() == "orElse")
                {
                    return;
                }
                self.report(field.field().span());
            }
            Expr::Index(index) if state.infer(index.object()).interface("dart:core", "List") => {
                if let Some(list) = self.builder_index(index) {
                    self.bounded_indexes.push((index.span(), list));
                } else {
                    self.report(index.span());
                }
            }
            _ => {}
        }
    }
}

fn bounds_only_use(identifier: Identifier<'_>) -> bool {
    let mut node = identifier.syntax();
    while let Some(parent) = node.parent() {
        match Expr::cast(parent) {
            Some(Expr::Paren(_) | Expr::NullAssert(_)) => node = parent,
            Some(Expr::Index(index)) => return index.object().span() == node.span(),
            Some(Expr::Field(field))
                if field.field().name() == "length" && field.object().span() == node.span() =>
            {
                node = parent;
                while let Some(parent) = node.parent() {
                    match Expr::cast(parent) {
                        Some(Expr::Paren(_)) => node = parent,
                        Some(Expr::Assign(assignment)) => {
                            return assignment.target().span() != node.span();
                        }
                        Some(Expr::PostfixIncDec(_)) => return false,
                        Some(Expr::Unary(unary)) => {
                            return !matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus);
                        }
                        _ => return true,
                    }
                }
                return true;
            }
            _ => return false,
        }
    }
    false
}
