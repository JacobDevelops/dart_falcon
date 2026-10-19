//! Decode JSON containers using their actual runtime generic types.
//!
//! Instead of `jsonDecode(text) as List<int>`, use
//! `(jsonDecode(text) as List<Object?>).cast<int>()`. For maps, first use
//! `Map<String, Object?>`, then `.cast<String, Value>()`.
//!
//! Tracks canonical SDK jsonDecode/json.decode calls without a reviver, direct
//! no-reviver JsonDecoder().convert calls, immutable local aliases, casts, and
//! indexing into those decoded values. Cast patterns in
//! conditionals, switch cases/expressions, and pattern declarations are checked.
//! Arbitrary dynamic values, unresolved generic arguments, parameters merely
//! named json, mutable aliases, custom decoders, and values returned from
//! collection conversions are not assumed to be decoded JSON.
//!
//! Reference: https://dcm.dev/docs/rules/common/prefer-correct-json-casts/

use std::collections::HashSet;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, QualifiedName, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferCorrectJsonCasts;

impl Rule for PreferCorrectJsonCasts {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
            decoded_locals: HashSet::new(),
            decoded_patterns: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    decoded_locals: HashSet<usize>,
    decoded_patterns: Vec<Span>,
}

impl<'a> Collector<'a> {
    fn decoded(&self, expression: Expr<'a>, state: &SemanticState<'_>) -> bool {
        struct Provenance<F> {
            next: Span,
            leaf: F,
            found: bool,
        }

        impl<'node, F: FnMut(Expr<'node>) -> bool> visit::Visitor<'node> for Provenance<F> {
            fn visit_expr(&mut self, node: Expr<'node>) {
                if node.span() != self.next {
                    return;
                }
                let node = node.unparenthesized();
                // Follow only the value-producing edge. In particular, a JSON
                // call used as an index or argument does not taint its container.
                let next = match node {
                    Expr::As(cast) => Some(cast.expr().span()),
                    Expr::NullAssert(assertion) => Some(assertion.operand().span()),
                    Expr::Index(index) => Some(index.object().span()),
                    _ => None,
                };
                if let Some(next) = next {
                    self.next = next;
                    visit::walk_expr(self, node);
                } else {
                    self.found = (self.leaf)(node);
                }
            }
        }

        let mut provenance = Provenance {
            next: expression.span(),
            leaf: |node| self.decoded_leaf(node, state),
            found: false,
        };
        visit::Visitor::visit_expr(&mut provenance, expression);
        provenance.found
    }

    fn decoded_leaf(&self, expression: Expr<'a>, state: &SemanticState<'_>) -> bool {
        match expression.unparenthesized() {
            Expr::Ident(name) => state
                .environment
                .binding_declaration(name.name())
                .is_some_and(|span| self.decoded_locals.contains(&span.start)),
            Expr::Call(call) if call.args().named().is_empty() => {
                let (callee, sdk_member) = match call.callee().unparenthesized() {
                    Expr::Field(field) if field.field().name() == "convert" => {
                        let receiver = field.object().unparenthesized();
                        let arguments = match receiver {
                            Expr::Call(constructor) => {
                                let Some(segments) = falcon_analyze::expression_segments(
                                    constructor.callee(),
                                    false,
                                ) else {
                                    return false;
                                };
                                let Some(head) = segments.first() else {
                                    return false;
                                };
                                if state.environment.is_bound(head)
                                    || state.constant_owner.as_ref().is_some_and(|owner| {
                                        !state
                                            .signatures
                                            .declared_member_facts(owner, head)
                                            .is_empty()
                                    })
                                    || !state.model.resolve_type_name(&segments).is_some_and(|ty| {
                                        ty.interface("dart:convert", "JsonDecoder")
                                    })
                                {
                                    return false;
                                }
                                Some(constructor.args())
                            }
                            Expr::New(constructor) => Some(constructor.args()),
                            _ => None,
                        };
                        return arguments.is_some_and(|args| {
                            args.positional().is_empty()
                                && args.named().is_empty()
                                && state
                                    .infer(receiver)
                                    .interface("dart:convert", "JsonDecoder")
                        });
                    }
                    Expr::Field(field) if field.field().name() == "decode" => {
                        (field.object(), "json")
                    }
                    _ => (call.callee(), "jsonDecode"),
                };
                let Some(segments) = falcon_analyze::expression_segments(callee, false) else {
                    return false;
                };
                let Some(head) = segments.first() else {
                    return false;
                };
                !state.environment.is_bound(head)
                    && state.implicit_member_type(head).is_none()
                    // Only directly declared members precede library/import lookup;
                    // inherited methods do not shadow imported functions or prefixes.
                    && !state.constant_owner.as_ref().is_some_and(|owner| {
                        !state.signatures.declared_member_facts(owner, head).is_empty()
                    })
                    && QualifiedName::from_segments(&segments)
                        .and_then(|name| state.model.resolve_sdk_member(name))
                        .is_some_and(|identity| identity.is_sdk("dart:convert", sdk_member))
            }
            _ => false,
        }
    }

    fn check(&mut self, target: DartType<'a>, state: &SemanticState<'_>) {
        let ty = state.model.resolve_type_in(target, &state.type_parameters);
        let broad = |ty: &ResolvedType| {
            matches!(
                ty,
                ResolvedType::Dynamic
                    | ResolvedType::Void
                    | ResolvedType::Unknown
                    | ResolvedType::TypeParameter { .. }
                    | ResolvedType::NonNullTypeParameter { .. }
            ) || (ty.interface("dart:core", "Object") && ty.nullable())
        };
        let unsafe_cast = if ty.interface("dart:core", "List") {
            ty.type_argument(0).is_some_and(|element| !broad(&element))
        } else if ty.interface("dart:core", "Map") {
            ty.type_argument(0).is_some_and(|key| {
                !broad(&key)
                    && !key.interface("dart:core", "Object")
                    && !key.interface("dart:core", "String")
            }) || ty.type_argument(1).is_some_and(|value| !broad(&value))
        } else {
            false
        };
        if unsafe_cast {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-correct-json-casts",
                "Cast JSON to List<Object?> or Map<String, Object?> first, then use cast() to check its contents.",
                target.span(),
            ));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'_>) {
        match node {
            Stmt::LocalVar(declaration) if declaration.keyword() == VarKeyword::Final => {
                for declarator in declaration.declarators() {
                    if declarator
                        .initializer()
                        .is_some_and(|value| self.decoded(value, state))
                    {
                        self.decoded_locals.insert(declarator.name().span().start);
                    }
                }
            }
            Stmt::If(statement) => {
                if let IfCondition::Case(condition) = statement.condition()
                    && self.decoded(condition.expr(), state)
                {
                    self.decoded_patterns.push(condition.pattern().span());
                }
            }
            Stmt::PatternDecl(declaration) if self.decoded(declaration.init(), state) => {
                self.decoded_patterns.push(declaration.pattern().span());
            }
            Stmt::Switch(statement) if self.decoded(statement.subject(), state) => {
                for case in statement.cases() {
                    for kind in case.cases() {
                        if let SwitchCaseKind::Pattern(label) = kind {
                            self.decoded_patterns.push(label.pattern().span());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        if let Expr::Switch(expression) = node
            && self.decoded(expression.subject(), state)
        {
            self.decoded_patterns
                .extend(expression.arms().map(|arm| arm.pattern().span()));
        }
        if let Expr::As(cast) = node
            && self.decoded(cast.expr(), state)
        {
            self.check(cast.dart_type(), state);
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        _matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if let Pattern::Cast(cast) = node
            && self
                .decoded_patterns
                .iter()
                .any(|span| span.start <= cast.span().start && cast.span().end <= span.end)
        {
            self.check(cast.cast_type(), state);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
