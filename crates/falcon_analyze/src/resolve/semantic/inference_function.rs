use super::flow::FlowContext;
use crate::resolve::semantic::ResolvedTypeKind;

use super::*;

impl TypeEnvironment {
    /// Infers a function literal or local function. Its body is a lexical child of
    /// `self`, so captured bindings keep their flow state and local-function kind.
    pub(super) fn infer_function(
        &self,
        function: FunctionSyntax<'_>,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
        expected: Option<&ResolvedType>,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> ResolvedType {
        use falcon_syntax::cst::AsyncMarker;
        if function.return_type.is_none()
            && matches!(
                function.async_marker,
                AsyncMarker::SyncStar | AsyncMarker::AsyncStar
            )
        {
            return ResolvedType::UNKNOWN;
        }
        let mut function_parameters = type_parameters.clone();
        function_parameters.push(function.type_params.clone(), model);
        let contextual = match expected {
            Some(ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        alias,
                        return_type,
                        positional,
                        named,
                        ..
                    },
            }) => Some((alias, return_type.as_ref(), positional, named)),
            _ => None,
        };
        let mut environment = self.clone();
        environment.enter_callable();
        environment.push_scope();
        let positional: Vec<_> = function
            .params
            .positional()
            .enumerate()
            .map(|(index, param)| {
                let ty = if param.param_type().is_some() || param.function_params().is_some() {
                    model.resolve_formal_parameter_in(param, &function_parameters)
                } else {
                    contextual
                        .and_then(|(alias, _, positional, _)| {
                            positional
                                .get(index)
                                .map(|ty| ty.clone().projected_from(alias))
                        })
                        .unwrap_or_else(|| signatures.uncontextual_closure_type(expected))
                };
                environment.declare(param.name().name().to_owned(), ty.clone());
                ty
            })
            .collect();
        let named: Vec<_> = function
            .params
            .named()
            .map(|param| {
                let ty = if param.param_type().is_some() || param.function_params().is_some() {
                    model.resolve_formal_parameter_in(param, &function_parameters)
                } else {
                    contextual
                        .and_then(|(alias, _, _, named)| {
                            named
                                .iter()
                                .find(|(name, _)| name == param.name().name())
                                .map(|(_, ty)| ty.clone().projected_from(alias))
                        })
                        .unwrap_or_else(|| signatures.uncontextual_closure_type(expected))
                };
                environment.declare(param.name().name().to_owned(), ty.clone());
                (param.name().name().to_owned(), ty)
            })
            .collect();
        let return_type = match function.return_type {
            Some(declared) => model.resolve_type_in(declared, &function_parameters),
            None => {
                let body_context = contextual.map(|(alias, return_type, _, _)| {
                    let return_type = return_type.clone().projected_from(alias);
                    if function.async_marker == AsyncMarker::Async
                        && return_type.interface("dart:async", "Future")
                    {
                        ResolvedType {
                            kind: ResolvedTypeKind::Interface {
                                occurrence: Default::default(),
                                identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
                                arguments: return_type.type_arguments().collect(),
                                nullable: false,
                                extension_type: false,
                            },
                        }
                    } else {
                        return_type.clone()
                    }
                });
                let return_type = match function.body {
                    FunctionBody::Arrow(arrow) => environment.infer_with_context(
                        arrow.expr(),
                        model,
                        signatures,
                        &function_parameters,
                        body_context.as_ref(),
                        lookup,
                    ),
                    FunctionBody::Block(block) => {
                        let mut returns = ClosureReturns {
                            environment,
                            conservative_writes: false,
                            flow: super::flow::FlowContext {
                                model,
                                signatures,
                                type_parameters: &function_parameters,
                                field_promotions: self.field_promotions,
                            },
                            context: body_context.as_ref(),
                            lookup,
                            types: Vec::new(),
                            exhaustive_switches: Vec::new(),
                            never_expressions: FxHashSet::default(),
                            expression_never: None,
                        };
                        for statement in block.stmts() {
                            returns.visit_stmt(statement);
                        }
                        // A reachable end returns `null` implicitly.
                        if super::flow::completes_normally(
                            block.stmts(),
                            &|expression| returns.never_expressions.contains(&expression.span()),
                            &|statement| returns.exhaustive_switches.contains(&statement.span()),
                        ) {
                            returns.types.push(ResolvedType::NULL);
                        }
                        let inferred = returns
                            .types
                            .into_iter()
                            .map(|ty| {
                                if function.async_marker == AsyncMarker::Async {
                                    flatten_async_return(ty, signatures, model)
                                } else {
                                    ty
                                }
                            })
                            .reduce(|left, right| inference_join(&left, &right, signatures, model))
                            .unwrap_or(ResolvedType::NULL);
                        // Inference imputes `void` when the return context is
                        // `void`, or `FutureOr<void>` for an `async` literal.
                        let imputes_void = body_context.as_ref().is_some_and(|context| {
                            matches!(
                                context,
                                ResolvedType {
                                    kind: ResolvedTypeKind::Void
                                }
                            ) || (function.async_marker == AsyncMarker::Async
                                && context.interface("dart:async", "FutureOr")
                                && matches!(
                                    context.arguments().first(),
                                    Some(ResolvedType {
                                        kind: ResolvedTypeKind::Void
                                    })
                                ))
                        });
                        if imputes_void {
                            ResolvedType::VOID
                        } else {
                            inferred
                        }
                    }
                    FunctionBody::Native(_) => ResolvedType::UNKNOWN,
                };
                if function.async_marker == AsyncMarker::Async {
                    let argument = flatten_async_return(return_type, signatures, model);
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:async", "Future"),
                            arguments: vec![argument].into(),
                            nullable: false,
                            extension_type: false,
                        },
                    }
                } else {
                    return_type
                }
            }
        };
        ResolvedType {
            kind: ResolvedTypeKind::Function {
                shape: FunctionTypeShape::from_scope(
                    function
                        .params
                        .named()
                        .filter(|param| matches!(param.kind(), ParamKind::Named { required: true }))
                        .map(|param| param.name().name().to_owned())
                        .collect(),
                    function.type_params,
                    &function_parameters,
                ),
                optional_positional: function
                    .params
                    .positional()
                    .filter(|param| param.kind() == ParamKind::OptionalPositional)
                    .count(),
                alias: FunctionTypeAlias::default(),
                return_type: Arc::new(return_type),
                positional: positional.into(),
                named: named.into(),
                nullable: false,
            },
        }
    }
}

/// Dart's async return inference flattens each return before computing their UP.
fn flatten_async_return(
    ty: ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    let flattened =
        if ty.interface("dart:async", "Future") || ty.interface("dart:async", "FutureOr") {
            Some(ty.type_argument(0).unwrap_or(ResolvedType::DYNAMIC))
        } else {
            signatures
                .instantiated_supertype(&ty, "dart:async", "Future", model)
                .and_then(|future| future.type_argument(0))
        };
    match flattened {
        Some(flattened) if ty.nullable() => flattened.with_nullable(true),
        Some(flattened) => flattened,
        None => ty,
    }
}

/// Collects a block body's return types while binding its locals in a lexical child environment.
pub(super) struct ClosureReturns<'a, 'model, L> {
    pub(super) environment: TypeEnvironment,
    /// Unsupported branch/loop joins must invalidate writes without
    /// publishing a promotion from a path that need not execute.
    pub(super) conservative_writes: bool,
    pub(super) flow: super::flow::FlowContext<'a, 'model>,
    pub(super) context: Option<&'a ResolvedType>,
    pub(super) lookup: &'a L,
    pub(super) types: Vec<ResolvedType>,
    pub(super) exhaustive_switches: Vec<Span>,
    /// Preserve statement-time flow facts without reinferring nested callables.
    pub(super) never_expressions: FxHashSet<Span>,
    /// Reuse assignment inference only within the current expression's flow state.
    pub(super) expression_never: Option<bool>,
}

impl<L: Fn(&str) -> Option<ResolvedType>> ClosureReturns<'_, '_, L> {
    pub(super) fn infer(
        &self,
        expression: Expr<'_>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        self.environment.infer_with_context(
            expression,
            self.flow.model,
            self.flow.signatures,
            self.flow.type_parameters,
            expected,
            self.lookup,
        )
    }

    fn statement_exits(&self, statement: falcon_syntax::cst::Stmt<'_>) -> bool {
        super::flow::exits(
            statement,
            &|expression| self.never_expressions.contains(&expression.span()),
            &|_| false,
            true,
        )
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.environment.push_scope();
        visit(self);
        self.environment.pop_scope();
    }

    pub(super) fn declare_local(&mut self, declaration: falcon_syntax::cst::LocalVarDecl<'_>) {
        for declarator in declaration.declarators() {
            let ty = declaration
                .var_type()
                .map(|ty| {
                    self.flow
                        .model
                        .resolve_type_in(ty, self.flow.type_parameters)
                })
                .or_else(|| {
                    declarator
                        .initializer()
                        .map(|expression| self.infer(expression, None))
                })
                .unwrap_or(ResolvedType::DYNAMIC);
            if let Some(initializer) = declarator.initializer() {
                self.visit_expr(initializer);
            }
            self.environment
                .declare(declarator.name().name().to_owned(), ty);
        }
    }

    pub(super) fn declare_pattern(
        &mut self,
        pattern: falcon_syntax::cst::Pattern<'_>,
        matched: ResolvedType,
    ) {
        self.flow
            .declare_pattern(&mut self.environment, pattern, matched);
    }
}

impl<'tree, L: Fn(&str) -> Option<ResolvedType>> Visitor<'tree> for ClosureReturns<'_, '_, L> {
    fn visit_stmt(&mut self, statement: falcon_syntax::cst::Stmt<'tree>) {
        use falcon_syntax::cst::{ForInit, IfCondition, Stmt, SwitchCaseKind};
        let loop_start = match statement {
            Stmt::For(statement) => Some(statement.span().start),
            Stmt::While(statement) => Some(statement.span().start),
            Stmt::DoWhile(statement) => Some(statement.span().start),
            _ => None,
        };
        if let Some(start) = loop_start
            && let Some(writes) = self
                .flow
                .signatures
                .flow_writes
                .get(&self.flow.model.file)
                .and_then(|writes| writes.region_writes.get(&start))
        {
            for name in writes {
                self.environment.demote(name);
            }
        }
        let conservative_before = self.conservative_writes;
        self.conservative_writes |= matches!(
            statement,
            Stmt::While(_) | Stmt::DoWhile(_) | Stmt::For(_) | Stmt::TryCatch(_) | Stmt::Switch(_)
        ) || matches!(statement, Stmt::If(statement) if matches!(statement.condition(), IfCondition::Case(..)));
        let optional_entry =
            (self.conservative_writes && !conservative_before).then(|| self.environment.clone());
        match statement {
            Stmt::Expr(statement) => {
                let expression = statement.expr();
                self.visit_expr(expression);
                let never = self
                    .expression_never
                    .take()
                    .unwrap_or_else(|| self.infer(expression, None) == ResolvedType::NEVER);
                if never {
                    self.never_expressions.insert(expression.span());
                }
            }
            Stmt::Return(statement) => {
                if let Some(value) = statement.value() {
                    self.visit_expr(value);
                }
                let ty = statement
                    .value()
                    .map_or(ResolvedType::VOID, |value| self.infer(value, self.context));
                self.types.push(ty);
            }
            Stmt::LocalVar(declaration) => self.declare_local(declaration),
            Stmt::LocalFunc(function) => {
                let ty = self.environment.infer_function(
                    FunctionSyntax {
                        type_params: function.type_params(),
                        params: function.params(),
                        body: function.body(),
                        async_marker: function.async_marker(),
                        return_type: function.return_type(),
                    },
                    self.flow.model,
                    self.flow.signatures,
                    self.flow.type_parameters,
                    None,
                    self.lookup,
                );
                self.environment
                    .declare_local_function(function.name().name().to_owned(), ty);
            }
            Stmt::PatternDecl(declaration) => {
                let matched = self.infer(declaration.init(), None);
                self.declare_pattern(declaration.pattern(), matched);
            }
            Stmt::Block(_) => self.scoped(|returns| walk_stmt(returns, statement)),
            Stmt::If(statement) => match statement.condition() {
                IfCondition::Expr(condition) => {
                    self.visit_expr(condition);
                    let before = self.environment.clone();
                    self.flow
                        .assume_condition(&mut self.environment, condition, true);
                    self.scoped(|returns| returns.visit_stmt(statement.then_branch()));
                    let then_exits = self.statement_exits(statement.then_branch());
                    let then_environment = std::mem::replace(&mut self.environment, before);
                    self.flow
                        .assume_condition(&mut self.environment, condition, false);
                    let else_exits = if let Some(branch) = statement.else_branch() {
                        self.scoped(|returns| returns.visit_stmt(branch));
                        self.statement_exits(branch)
                    } else {
                        false
                    };
                    // Unlike the scoped walker, closure inference keeps the else
                    // snapshot when neither branch can complete.
                    if !then_exits || !else_exits {
                        FlowContext::join_branches(
                            &mut self.environment,
                            then_environment,
                            then_exits,
                            else_exits,
                        );
                    }
                    if let Some(entry) = optional_entry {
                        FlowContext::join_branches(&mut self.environment, entry, false, false);
                    }
                    self.conservative_writes = conservative_before;
                    return;
                }
                IfCondition::Case(case) => {
                    let value = case.expr();
                    let pattern = case.pattern();
                    self.visit_expr(value);
                    let matched = self.infer(value, None);
                    let before = self.environment.clone();
                    self.scoped(|returns| {
                        returns.declare_pattern(pattern, matched);
                        returns.visit_stmt(statement.then_branch());
                    });
                    let then_environment = self.environment.clone();
                    self.environment = before.join(&then_environment);
                    if let Some(branch) = statement.else_branch() {
                        self.visit_stmt(branch);
                    }
                    FlowContext::join_branches(
                        &mut self.environment,
                        then_environment,
                        false,
                        false,
                    );
                }
            },
            Stmt::For(statement) => self.scoped(|returns| {
                match statement.header().init() {
                    Some(ForInit::VarDecl(declaration)) => returns.declare_local(declaration),
                    Some(ForInit::ForIn(binding)) if binding.declares_variable() => {
                        let var_type = binding.var_type();
                        let name = binding.name();
                        let iterable = binding.iterable();
                        let ty = match var_type {
                            Some(ty) => returns
                                .flow
                                .model
                                .resolve_type_in(ty, returns.flow.type_parameters),
                            None => returns.flow.iterable_element(
                                &returns.infer(iterable, None),
                                statement.is_await(),
                            ),
                        };
                        returns.environment.declare(name.name().to_owned(), ty);
                    }
                    Some(ForInit::PatternForIn(binding)) => {
                        let pattern = binding.pattern();
                        let iterable = binding.iterable();
                        let matched = returns
                            .flow
                            .iterable_element(&returns.infer(iterable, None), statement.is_await());
                        returns.declare_pattern(pattern, matched);
                    }
                    // A bare `for (x in xs)` writes the enclosing `x`.
                    Some(ForInit::Exprs(_) | ForInit::ForIn(_)) | None => {}
                }
                returns.visit_stmt(statement.body());
            }),
            Stmt::TryCatch(statement) => {
                let before = self.environment.clone();
                self.scoped(|returns| {
                    for statement in statement.body().stmts() {
                        returns.visit_stmt(statement);
                    }
                });
                let mut exits = self.environment.clone();
                for catch in statement.catches() {
                    self.environment = before.join(&exits);
                    self.scoped(|returns| {
                        // Untyped `catch (e, s)` binds `Object` and `StackTrace`.
                        if let Some(name) = catch.exception_var() {
                            let model = returns.flow.model;
                            let ty = catch.exception_type().map_or_else(
                                || model.core_type("Object", false),
                                |ty| model.resolve_type_in(ty, returns.flow.type_parameters),
                            );
                            returns.environment.declare(name.name().to_owned(), ty);
                        }
                        if let Some(name) = catch.stack_trace_var() {
                            let ty = returns.flow.model.core_type("StackTrace", false);
                            returns.environment.declare(name.name().to_owned(), ty);
                        }
                        for statement in catch.body().stmts() {
                            returns.visit_stmt(statement);
                        }
                    });
                    exits = exits.join(&self.environment);
                }
                self.environment = before.join(&exits);
                if let Some(finally) = statement.finally() {
                    self.scoped(|returns| {
                        for statement in finally.stmts() {
                            returns.visit_stmt(statement);
                        }
                    });
                }
            }
            Stmt::Switch(statement) => {
                self.visit_expr(statement.subject());
                let matched = self.infer(statement.subject(), None);
                if self.flow.switch_is_exhaustive(statement, &matched) {
                    self.exhaustive_switches.push(statement.span());
                }
                let before = self.environment.clone();
                let mut exits = before.clone();
                for case in statement.cases() {
                    self.environment = before.join(&exits);
                    self.scoped(|returns| {
                        for kind in case.cases() {
                            if let SwitchCaseKind::Pattern(label) = kind {
                                returns.declare_pattern(label.pattern(), matched.clone());
                            }
                        }
                        for statement in case.body() {
                            returns.visit_stmt(statement);
                        }
                    });
                    exits = exits.join(&self.environment);
                }
                self.environment = exits;
            }
            _ => walk_stmt(self, statement),
        }
        if let Some(entry) = optional_entry {
            FlowContext::join_branches(&mut self.environment, entry, false, false);
        }
        self.conservative_writes = conservative_before;
    }

    fn visit_expr(&mut self, expression: Expr<'tree>) {
        self.expression_never = None;
        let conservative_before = self.conservative_writes;
        self.conservative_writes |= matches!(
            expression,
            Expr::Switch(_) | Expr::List(_) | Expr::Set(_) | Expr::Map(_)
        );
        match expression {
            // A nested callable is inferred separately and does not execute here.
            Expr::FuncExpr(_) => {
                self.conservative_writes = conservative_before;
                return;
            }
            Expr::Conditional(conditional) => {
                let condition = conditional.condition();
                let then_expr = conditional.then_expr();
                let else_expr = conditional.else_expr();
                self.visit_expr(condition);
                let before = self.environment.clone();
                self.flow
                    .assume_condition(&mut self.environment, condition, true);
                self.visit_expr(then_expr);
                let then_environment = std::mem::replace(&mut self.environment, before);
                self.flow
                    .assume_condition(&mut self.environment, condition, false);
                self.visit_expr(else_expr);
                FlowContext::join_branches(&mut self.environment, then_environment, false, false);
            }
            Expr::Binary(binary)
                if matches!(
                    binary.op(),
                    BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce
                ) =>
            {
                let left = binary.left();
                let right = binary.right();
                self.visit_expr(left);
                let before = self.environment.clone();
                self.visit_expr(right);
                self.environment = before.join(&self.environment);
            }
            Expr::Binary(binary) => {
                let mut pending = Vec::new();
                let mut operand = binary.left();
                while let Expr::Binary(next) = operand {
                    if matches!(
                        next.op(),
                        BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce
                    ) {
                        break;
                    }
                    pending.push(next.right());
                    operand = next.left();
                }
                self.visit_expr(operand);
                for right in pending.into_iter().rev() {
                    self.visit_expr(right);
                }
                self.visit_expr(binary.right());
            }
            _ => walk_expr(self, expression),
        }
        // A child's result does not describe its containing expression.
        self.expression_never = None;
        if self.conservative_writes {
            let target = match expression {
                Expr::Assign(assignment) => Some(assignment.target()),
                Expr::Unary(unary)
                    if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
                {
                    Some(unary.operand())
                }
                Expr::PostfixIncDec(postfix) => Some(postfix.operand()),
                _ => None,
            };
            if let Some(Expr::Ident(name)) = target {
                self.environment.demote(name.name());
            }
        } else {
            let assigned = match expression {
                Expr::Assign(assignment) => {
                    let target = assignment.target();
                    let op = assignment.op();
                    let value = assignment.value();
                    let expected = match target {
                        Expr::Ident(name)
                            if self.environment.is_bound(name.name())
                                && matches!(op.binary(), None | Some(BinaryOp::NullCoalesce)) =>
                        {
                            Some(self.environment.declared_type(name.name()))
                        }
                        _ => None,
                    };
                    let assigned = self.infer(value, expected.as_ref());
                    if op.binary().is_none() {
                        self.expression_never = Some(assigned == ResolvedType::NEVER);
                    }
                    Some(assigned)
                }
                _ => None,
            };
            self.flow
                .finish_expression(&mut self.environment, expression, assigned);
        }
        self.conservative_writes = conservative_before;
    }
}

#[cfg(test)]
#[path = "inference_function_tests.rs"]
mod tests;
