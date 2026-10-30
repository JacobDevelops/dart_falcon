use super::{ClosureReturns, DeclarationIdentity, Expr, ResolvedType, Visitor};
use crate::resolve::semantic::ResolvedTypeKind;

pub(super) struct CollectionTypes<'a, 'model, L> {
    pub(super) lexical: ClosureReturns<'a, 'model, L>,
    pub(super) contexts: [Option<&'a ResolvedType>; 2],
    pub(super) components: [Vec<ResolvedType>; 2],
    /// A brace literal without a distinguishing downward schema can acquire
    /// its map shape from a spread operand rather than an explicit entry.
    pub(super) infer_map_spreads: bool,
}

impl<L: Fn(&str) -> Option<ResolvedType>> CollectionTypes<'_, '_, L> {
    fn branch(
        &mut self,
        condition: falcon_syntax::cst::IfCondition<'_>,
        truth: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        use falcon_syntax::cst::IfCondition;
        let saved = self.lexical.environment.clone();
        self.lexical.environment.push_scope();
        match condition {
            IfCondition::Expr(expression) => {
                self.lexical.flow.assume_condition(
                    &mut self.lexical.environment,
                    expression,
                    truth,
                );
            }
            IfCondition::Case(case) if truth => {
                let expression = case.expr();
                let pattern = case.pattern();
                let guard = case.guard();
                let matched = self.lexical.infer(expression, None);
                self.lexical.declare_pattern(pattern, matched);
                self.lexical.flow.assume_pattern(
                    &mut self.lexical.environment,
                    expression,
                    pattern,
                    true,
                );
                if let Some(guard) = guard {
                    self.lexical
                        .flow
                        .assume_condition(&mut self.lexical.environment, guard, true);
                }
            }
            IfCondition::Case(case) if case.guard().is_none() => self.lexical.flow.assume_pattern(
                &mut self.lexical.environment,
                case.expr(),
                case.pattern(),
                false,
            ),
            IfCondition::Case(_) => {}
        }
        visit(self);
        self.lexical.environment = saved;
    }

    fn loop_body(
        &mut self,
        header: falcon_syntax::cst::ForHeader<'_>,
        is_await: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        use falcon_syntax::cst::ForInit;
        let saved = header.condition().map(|_| self.lexical.environment.clone());
        self.lexical.environment.push_scope();
        match header.init() {
            Some(ForInit::VarDecl(declaration)) => self.lexical.declare_local(declaration),
            Some(ForInit::ForIn(binding)) if binding.declares_variable() => {
                let var_type = binding.var_type();
                let name = binding.name();
                let iterable = binding.iterable();
                let ty = var_type.map_or_else(
                    || {
                        self.lexical
                            .flow
                            .iterable_element(&self.lexical.infer(iterable, None), is_await)
                    },
                    |ty| {
                        self.lexical
                            .flow
                            .model
                            .resolve_type_in(ty, self.lexical.flow.type_parameters)
                    },
                );
                self.lexical.environment.declare(name.name().to_owned(), ty);
            }
            Some(ForInit::PatternForIn(binding)) => {
                let pattern = binding.pattern();
                let iterable = binding.iterable();
                let matched = self
                    .lexical
                    .flow
                    .iterable_element(&self.lexical.infer(iterable, None), is_await);
                self.lexical.declare_pattern(pattern, matched);
            }
            _ => {}
        }
        if let Some(condition) = header.condition() {
            self.lexical
                .flow
                .assume_condition(&mut self.lexical.environment, condition, true);
        }
        visit(self);
        self.lexical.environment.pop_scope();
        if let Some(saved) = saved {
            self.lexical.environment = saved;
        }
    }

    fn spread(&mut self, expression: Expr<'_>, map: bool, null_aware: bool) {
        let context = self.contexts[0].map(|first| ResolvedType {
            kind: ResolvedTypeKind::Interface {
                occurrence: super::FunctionTypeAlias::default(),
                identity: DeclarationIdentity::sdk(
                    "dart:core",
                    if map { "Map" } else { "Iterable" },
                ),
                arguments: if map {
                    vec![
                        first.clone(),
                        self.contexts[1].cloned().unwrap_or(ResolvedType::UNKNOWN),
                    ]
                    .into()
                } else {
                    vec![first.clone()].into()
                },
                nullable: null_aware,
                extension_type: false,
            },
        });
        let ty = self.lexical.infer(expression, context.as_ref());
        let map_type = (map || self.infer_map_spreads)
            .then(|| {
                self.lexical.flow.signatures.instantiated_supertype(
                    &ty.non_null(),
                    "dart:core",
                    "Map",
                    self.lexical.flow.model,
                )
            })
            .flatten();
        let map = map || map_type.is_some();
        let ty = if map {
            map_type
        } else {
            self.lexical.flow.signatures.instantiated_supertype(
                &ty.non_null(),
                "dart:core",
                "Iterable",
                self.lexical.flow.model,
            )
        };
        for (ordinal, component) in
            self.components
                .iter_mut()
                .enumerate()
                .take(if map { 2 } else { 1 })
        {
            component.push(
                ty.as_ref()
                    .and_then(|ty| ty.type_argument(ordinal))
                    .unwrap_or(ResolvedType::UNKNOWN),
            );
        }
    }
}

impl<'tree, L: Fn(&str) -> Option<ResolvedType>> Visitor<'tree> for CollectionTypes<'_, '_, L> {
    fn visit_element<E: falcon_syntax::cst::Element<'tree>>(&mut self, element: E) {
        use falcon_syntax::cst::ElementKind;
        match element.kind() {
            ElementKind::Expression(expression) => {
                self.components[0].push(self.lexical.infer(expression, self.contexts[0]));
            }
            ElementKind::NullAware(element) => self.components[0].push(
                self.lexical
                    .infer(element.expr(), self.contexts[0])
                    .non_null(),
            ),
            ElementKind::Entry(entry) => {
                let key = self.lexical.infer(entry.key(), self.contexts[0]);
                let value = self.lexical.infer(entry.value(), self.contexts[1]);
                self.components[0].push(if entry.key_null_aware() {
                    key.non_null()
                } else {
                    key
                });
                self.components[1].push(if entry.value_null_aware() {
                    value.non_null()
                } else {
                    value
                });
            }
            ElementKind::Spread { expression } => self.spread(
                expression,
                E::IS_MAP || self.contexts[1].is_some(),
                element.spread_is_null_aware(),
            ),
            ElementKind::If {
                condition,
                then_element,
                else_element,
            } => {
                self.branch(condition, true, |visitor| {
                    visitor.visit_element(then_element);
                });
                if let Some(element) = else_element {
                    self.branch(condition, false, |visitor| visitor.visit_element(element));
                }
            }
            ElementKind::For { header, body } => {
                self.loop_body(header, element.is_await(), |visitor| {
                    visitor.visit_element(body);
                });
            }
        }
    }

    fn visit_expr(&mut self, _expression: Expr<'tree>) {}
}
