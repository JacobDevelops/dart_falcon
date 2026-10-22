//! Flow transfers used by the shared syntax visitor; no independent syntax-tree walk.
use crate::resolve::semantic::ResolvedTypeKind;
use std::borrow::Cow;

use falcon_syntax::cst::visit::{Visitor, walk_expr, walk_stmt};
use falcon_syntax::cst::{
    AstChildren, BinaryOp, DartType, Expr, ForInit, ListPatternElement, LiteralPatternValue,
    Pattern, Stmt, SwitchCaseKind, SwitchStmt, UnaryOp,
};

use super::semantic::operator_result;
use super::{
    DeclarationIdentity, QualifiedName, ResolvedType, SemanticModel, SignatureIndex, Truth,
    TypeEnvironment, TypeParameterScope,
};

pub struct FlowContext<'a, 'model> {
    pub model: &'a SemanticModel<'model>,
    pub signatures: &'a SignatureIndex,
    pub type_parameters: &'a TypeParameterScope,
    pub field_promotions: bool,
}

impl FlowContext<'_, '_> {
    /// Proves only spaces represented by the semantic index, never a guessed
    /// closed subtype set. Guards cannot contribute unconditional coverage.
    pub fn switch_is_exhaustive(
        &self,
        statement: falcon_syntax::cst::SwitchStmt<'_>,
        matched: &ResolvedType,
    ) -> bool {
        use falcon_syntax::cst::{ConstPatternValue, LiteralPatternValue, Pattern, SwitchCaseKind};
        let patterns = || {
            statement
                .cases()
                .flat_map(|case| case.cases())
                .filter_map(|case| match case {
                    SwitchCaseKind::Pattern(label) if label.guard().is_none() => {
                        Some(label.pattern())
                    }
                    _ => None,
                })
        };
        let covers_type = |matched: &ResolvedType| {
            patterns().any(|pattern| {
                let target = match pattern {
                    Pattern::Wildcard(wildcard) => {
                        let Some(type_) = wildcard.type_() else {
                            return true;
                        };
                        self.model.resolve_type_in(type_, self.type_parameters)
                    }
                    Pattern::Variable(variable) => {
                        let Some(type_) = variable.type_() else {
                            return true;
                        };
                        self.model.resolve_type_in(type_, self.type_parameters)
                    }
                    Pattern::Object(object) if object.fields().is_empty() => {
                        self.object_pattern_type(object.type_(), matched)
                    }
                    _ => return false,
                };
                if matches!(
                    target,
                    ResolvedType {
                        kind: ResolvedTypeKind::Unknown
                    }
                ) {
                    return false;
                }
                matched == &target
                    || self.signatures.relation(
                        matched,
                        crate::TypeRelation::SubtypeOf(&target),
                        self.model,
                    ) == Truth::Yes
            })
        };
        if covers_type(matched) {
            return true;
        }
        let covers_null = patterns().any(|pattern| {
            matches!(pattern, Pattern::Literal(literal) if matches!(literal.value(), LiteralPatternValue::Null))
        });
        if *matched == ResolvedType::NULL {
            return covers_null;
        }
        if matched.nullable() && !covers_null {
            return false;
        }
        if matched.nullable() && covers_type(&matched.with_nullable(false)) {
            return true;
        }
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = matched
        else {
            return false;
        };
        if identity.is_sdk("dart:core", "bool") {
            return [false, true].into_iter().all(|value| {
                patterns().any(|pattern| {
                    matches!(pattern, Pattern::Literal(literal) if matches!(literal.value(), LiteralPatternValue::Bool(actual) if actual == value))
                })
            });
        }
        let Some(values) = self.signatures.enum_value_names(identity) else {
            return false;
        };
        !values.is_empty()
            && values.iter().all(|value| {
                patterns().any(|pattern| {
                    let Pattern::Const(constant) = pattern else {
                        return false;
                    };
                    let ConstPatternValue::Name(names) = constant.value() else {
                        return false;
                    };
                    let Some(name) = names.clone().last() else {
                        return false;
                    };
                    let owner = names.clone().take(names.len() - 1);
                    name.name() == *value
                        && QualifiedName::from_identifiers(owner)
                            .and_then(|owner| self.model.resolve_name(owner))
                            .as_ref()
                            == Some(identity)
                })
            })
    }

    /// Replacement paths drop deeper scopes; later joins project onto the accumulator.
    pub(crate) fn join_paths<'a>(
        environment: &mut TypeEnvironment,
        paths: impl IntoIterator<Item = (Cow<'a, TypeEnvironment>, bool)>,
        depth: usize,
        reachable: bool,
    ) {
        let mut joined: Option<Cow<'_, TypeEnvironment>> =
            reachable.then(|| Cow::Owned(std::mem::take(environment)));
        for (mut path, completes) in paths {
            if !completes {
                continue;
            }
            joined = Some(match joined {
                Some(previous) => Cow::Owned(previous.join(&path)),
                None => {
                    if path.depth() > depth {
                        let path = path.to_mut();
                        while path.depth() > depth {
                            path.pop_scope();
                        }
                    }
                    path
                }
            });
        }
        if let Some(joined) = joined {
            *environment = joined.into_owned();
        }
    }

    /// Selects completing branch ends; when both exit, retain the then state.
    pub(crate) fn join_branches(
        environment: &mut TypeEnvironment,
        then_environment: TypeEnvironment,
        then_exits: bool,
        else_exits: bool,
    ) {
        if else_exits {
            *environment = then_environment;
        } else if !then_exits {
            let depth = then_environment.depth();
            let else_environment = std::mem::replace(environment, then_environment);
            Self::join_paths(
                environment,
                [(Cow::Owned(else_environment), true)],
                depth,
                true,
            );
        }
    }

    fn infer(&self, environment: &TypeEnvironment, expression: Expr<'_>) -> ResolvedType {
        if let Expr::Ident(name) = expression
            && !environment.is_bound(name.name())
        {
            if let Some(ty) = environment.promoted_this_field(name.name()) {
                return ty;
            }
            let receiver = environment.receiver();
            if let Some((ty, substitutions)) = self
                .signatures
                .resolved_field(&receiver, name.name(), self.model)
                .or_else(|| {
                    self.signatures
                        .resolved_extension_getter(&receiver, name.name(), self.model)
                })
            {
                return self.model.substitute(&ty, &substitutions);
            }
        }
        environment.infer_with_signatures(
            expression,
            self.model,
            self.signatures,
            self.type_parameters,
        )
    }

    fn compatible(&self, value: &ResolvedType, target: &ResolvedType) -> bool {
        // `FutureOr<T>` is the union of `T` and `Future<T>`. Check it
        // before ordinary nullability: its value branch may itself be nullable.
        if let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    arguments,
                    ..
                },
        } = target
            && identity.is_sdk("dart:async", "FutureOr")
            && let Some(argument) = arguments.first()
        {
            // A written nullable union retains its null arm independently of
            // the argument: T? <: FutureOr<T>? follows from T <: FutureOr<T>.
            if target.nullable() {
                if matches!(
                    value,
                    ResolvedType {
                        kind: ResolvedTypeKind::Null
                    }
                ) {
                    return true;
                }
                if value.nullable()
                    && !matches!(
                        value,
                        ResolvedType {
                            kind: ResolvedTypeKind::Dynamic
                        } | ResolvedType {
                            kind: ResolvedTypeKind::Unknown
                        }
                    )
                {
                    return self.subtype(&value.with_nullable(false), &target.with_nullable(false));
                }
            }
            if self.subtype(value, argument) {
                return true;
            }
            if value.nullable() && !target.nullable() && !argument.nullable() {
                return false;
            }
            return match value {
                ResolvedType {
                    kind:
                        ResolvedTypeKind::Interface {
                            identity,
                            arguments,
                            ..
                        },
                } if identity.is_sdk("dart:async", "FutureOr") => arguments
                    .first()
                    .is_some_and(|value| self.subtype(value, argument)),
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { bound, .. },
                } => self.subtype(bound, target),
                ResolvedType {
                    kind: ResolvedTypeKind::NonNullTypeParameter { bound, .. },
                } => self.subtype(&bound.non_null(), target),
                _ => self
                    .signatures
                    .instantiated_supertype(value, "dart:async", "Future", self.model)
                    .is_some_and(|future| {
                        future
                            .arguments()
                            .first()
                            .is_some_and(|value| self.subtype(value, argument))
                    }),
            };
        }
        if value.nullable() && !target.nullable() {
            return false;
        }
        if same_shape(value, target) {
            return true;
        }
        if matches!(
            value,
            ResolvedType {
                kind: ResolvedTypeKind::Function { .. }
            }
        ) && target.interface("dart:core", "Function")
        {
            return true;
        }
        if let (
            ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { id: value, .. },
            },
            ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id: target, .. },
            },
        ) = (value, target)
        {
            return value == target;
        }
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    arguments,
                    ..
                },
        } = target
        else {
            return false;
        };
        // Every resolved type but `Null` (rejected above by nullability) is an `Object`.
        if *identity == DeclarationIdentity::OBJECT {
            return !matches!(
                value,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic
                } | ResolvedType {
                    kind: ResolvedTypeKind::Unknown
                } | ResolvedType {
                    kind: ResolvedTypeKind::Void
                }
            );
        }
        // Interface type arguments are covariant.
        self.signatures
            .instantiated_identity(value, identity, self.model)
            .is_some_and(|instance| {
                instance.arguments().len() == arguments.len()
                    && instance
                        .arguments()
                        .iter()
                        .zip(arguments.iter())
                        .all(|(value, target)| value == target || self.subtype(value, target))
            })
    }

    /// Whether `value` is provably a subtype of `target`.
    fn subtype(&self, value: &ResolvedType, target: &ResolvedType) -> bool {
        match (value, target) {
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown,
                },
                _,
            ) => false,
            (
                _,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic,
                }
                | ResolvedType {
                    kind: ResolvedTypeKind::Void,
                },
            ) => true,
            (
                _,
                ResolvedType {
                    kind: ResolvedTypeKind::Interface { identity, .. },
                },
            ) if identity.is_sdk("dart:async", "FutureOr") => self.compatible(value, target),
            // A potentially nullable bound proves subtyping only when the
            // bound itself fits the target; stripping its `?` would lose null.
            (
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { bound, .. },
                },
                _,
            ) if !target.nullable() && !self.signatures.is_non_nullable(bound) => {
                !value.nullable() && (same_shape(value, target) || self.subtype(bound, target))
            }
            _ => {
                self.compatible(value, target)
                    || self.signatures.relation(
                        value,
                        crate::TypeRelation::SubtypeOf(target),
                        self.model,
                    ) == super::Truth::Yes
            }
        }
    }

    /// Dart's `factor(T, S)`: the representable remainder after a failed `is S`.
    /// Nullable types and `FutureOr` are unions; subtract a component only when
    /// subtyping proves the test excludes it, retaining unrepresentable differences.
    fn factor(&self, value: &ResolvedType, excluded: &ResolvedType) -> ResolvedType {
        if super::semantic::has_inference_holes(value)
            || super::semantic::has_inference_holes(excluded)
        {
            return value.clone();
        }
        if self.subtype(value, excluded) {
            return ResolvedType::NEVER;
        }
        match value {
            ResolvedType {
                kind: ResolvedTypeKind::Interface { nullable: true, .. },
            }
            | ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { nullable: true, .. },
            }
            | ResolvedType {
                kind: ResolvedTypeKind::Function { nullable: true, .. },
            }
            | ResolvedType {
                kind: ResolvedTypeKind::Record { nullable: true, .. },
            } => {
                let remainder = self.factor(&value.with_nullable(false), excluded);
                if self.subtype(&ResolvedType::NULL, excluded) {
                    remainder
                } else {
                    remainder.with_nullable(true)
                }
            }
            ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } if identity.is_sdk("dart:async", "FutureOr") => {
                let Some(argument) = value.type_argument(0) else {
                    return value.clone();
                };
                let future = ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:async", "Future"),
                        arguments: vec![argument.clone()].into(),
                        nullable: false,
                        extension_type: false,
                    },
                };
                if self.subtype(&future, excluded) {
                    self.factor(&argument, excluded)
                } else if self.subtype(&argument, excluded) {
                    self.factor(&future, excluded)
                } else {
                    value.clone()
                }
            }
            _ => value.clone(),
        }
    }

    /// Dart's write of a `written`-typed value to local `name`: the promotion
    /// chain keeps its supertypes of `written`, then promotes to a type of interest.
    pub fn assign_variable(
        &self,
        environment: &mut TypeEnvironment,
        name: &str,
        written: &ResolvedType,
    ) {
        let declared = environment.declared_type(name);
        let mut chain: Vec<ResolvedType> = environment
            .promotion_chain(name)
            .iter()
            .take_while(|ty| self.subtype(written, ty))
            .cloned()
            .collect();
        if self.subtype(written, &declared)
            && let Some(ty) = self.type_of_interest(
                environment,
                name,
                &declared,
                chain.last().unwrap_or(&declared),
                written,
            )
        {
            chain.push(ty);
        }
        environment.assign(name, chain);
    }

    /// Dart's `tryPromoteToTypeOfInterest`: `written` itself when it is a type of
    /// interest, else the unique most specific type of interest between it and `current`.
    fn type_of_interest(
        &self,
        environment: &TypeEnvironment,
        name: &str,
        declared: &ResolvedType,
        current: &ResolvedType,
        written: &ResolvedType,
    ) -> Option<ResolvedType> {
        let mut candidates = Vec::new();
        for interest in std::iter::once(declared).chain(environment.tested_types(name)) {
            for candidate in [Some(interest.clone()), non_null(interest)]
                .into_iter()
                .flatten()
            {
                if candidate == *current
                    || !self.subtype(written, &candidate)
                    || !self.subtype(&candidate, current)
                {
                    continue;
                }
                if candidate == *written {
                    return Some(candidate);
                }
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
        candidates
            .iter()
            .find(|candidate| {
                candidates
                    .iter()
                    .all(|other| self.subtype(candidate, other))
            })
            .cloned()
    }

    fn promote(&self, environment: &mut TypeEnvironment, expression: Expr<'_>, ty: ResolvedType) {
        if let Expr::Ident(name) = expression {
            environment.record_tested_type(name.name(), ty.clone());
        }
        self.promote_without_test(environment, expression, ty);
    }

    fn promote_without_test(
        &self,
        environment: &mut TypeEnvironment,
        expression: Expr<'_>,
        mut ty: ResolvedType,
    ) {
        let current = self.infer(environment, expression);
        if matches!(
            ty,
            ResolvedType {
                kind: ResolvedTypeKind::Dynamic
            } | ResolvedType {
                kind: ResolvedTypeKind::Unknown
            }
        ) {
            return;
        }
        if self.subtype(&current, &ty) {
            return;
        }
        if current.nullable()
            && !ty.nullable()
            && !matches!(
                current,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic
                } | ResolvedType {
                    kind: ResolvedTypeKind::Unknown
                }
            )
        {
            let non_null = current.non_null();
            if self.subtype(&non_null, &ty) {
                ty = non_null;
            }
        }
        let compatible = match &current {
            ResolvedType {
                kind: ResolvedTypeKind::Dynamic,
            } => true,
            ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { bound, .. },
            } => self.subtype(&ty, bound),
            _ => self.subtype(&ty, &current),
        };
        if !compatible {
            return;
        }
        match expression.unparenthesized() {
            Expr::Ident(name) if environment.is_bound(name.name()) => {
                environment.promote_without_test(name.name(), ty)
            }
            Expr::Ident(name)
                if self.field_promotions
                    && self.signatures.is_promotable_field(
                        &environment.receiver(),
                        name.name(),
                        self.model,
                    ) =>
            {
                environment.promote_this_field(name.name(), ty)
            }
            Expr::Field(field)
                if !field.is_null_safe()
                    && self.field_promotions
                    && self.signatures.is_promotable_field(
                        &self.infer(environment, field.object()),
                        field.field().name(),
                        self.model,
                    ) =>
            {
                environment.promote_field(field.object(), field.field().name(), ty)
            }
            _ => {}
        }
    }

    pub fn assume_condition(
        &self,
        environment: &mut TypeEnvironment,
        expression: Expr<'_>,
        truth: bool,
    ) {
        let expression = expression.unparenthesized();
        match expression {
            Expr::Unary(unary) if unary.op() == UnaryOp::Bang => {
                self.assume_condition(environment, unary.operand(), !truth)
            }
            Expr::Binary(binary) if binary.op() == BinaryOp::And && truth => {
                self.assume_condition(environment, binary.left(), true);
                self.assume_condition(environment, binary.right(), true);
            }
            Expr::Binary(binary) if binary.op() == BinaryOp::Or && !truth => {
                self.assume_condition(environment, binary.left(), false);
                self.assume_condition(environment, binary.right(), false);
            }
            _ => {
                CompletedPromotions {
                    context: self,
                    environment,
                }
                .visit_expr(expression);
                match expression {
                    Expr::Is(test) => {
                        let expr = test.expr();
                        let tested = self
                            .model
                            .resolve_type_in(test.dart_type(), self.type_parameters);
                        if let Expr::Ident(name) = expr.unparenthesized() {
                            environment.record_tested_type(name.name(), tested.clone());
                        }
                        let ty = if truth != test.negated() {
                            tested
                        } else {
                            self.factor(&self.infer(environment, expr), &tested)
                        };
                        // An impossible type-test branch does not change the
                        // variable's static type to Never.
                        if ty != ResolvedType::NEVER {
                            self.promote_without_test(environment, expr, ty);
                        }
                    }
                    Expr::Binary(binary)
                        if matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) =>
                    {
                        let left = binary.left();
                        let right = binary.right();
                        let operand = if matches!(right.unparenthesized(), Expr::NullLit(_)) {
                            left
                        } else if matches!(left.unparenthesized(), Expr::NullLit(_)) {
                            right
                        } else {
                            return;
                        };
                        let non_null = (binary.op() == BinaryOp::NotEq) == truth;
                        if non_null {
                            self.promote(
                                environment,
                                operand,
                                self.infer(environment, operand).non_null(),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn assume_pattern(
        &self,
        environment: &mut TypeEnvironment,
        expression: Expr<'_>,
        pattern: Pattern<'_>,
        truth: bool,
    ) {
        match pattern {
            Pattern::ParenPattern(paren) => {
                self.assume_pattern(environment, expression, paren.inner(), truth)
            }
            Pattern::Literal(literal) if matches!(literal.value(), LiteralPatternValue::Null) => {
                if !truth {
                    self.promote(
                        environment,
                        expression,
                        self.infer(environment, expression).non_null(),
                    );
                }
            }
            Pattern::NullAssert(assertion) => {
                self.promote(
                    environment,
                    expression,
                    self.infer(environment, expression).non_null(),
                );
                self.assume_pattern(environment, expression, assertion.inner(), truth);
            }
            Pattern::NullCheck(check) if truth => {
                self.promote(
                    environment,
                    expression,
                    self.infer(environment, expression).non_null(),
                );
                self.assume_pattern(environment, expression, check.inner(), true);
            }
            Pattern::LogicalAnd(logical) if truth => {
                self.assume_pattern(environment, expression, logical.left(), true);
                self.assume_pattern(environment, expression, logical.right(), true);
            }
            Pattern::LogicalOr(logical) if !truth => {
                self.assume_pattern(environment, expression, logical.left(), false);
                self.assume_pattern(environment, expression, logical.right(), false);
            }
            Pattern::LogicalOr(logical) if truth => {
                let mut alternative = environment.clone();
                self.assume_pattern(environment, expression, logical.left(), true);
                self.assume_pattern(&mut alternative, expression, logical.right(), true);
                *environment = environment.join(&alternative);
            }
            Pattern::Wildcard(_) | Pattern::Variable(_) if truth => {
                let ty = match pattern {
                    Pattern::Wildcard(wildcard) => wildcard.type_(),
                    Pattern::Variable(variable) => variable.type_(),
                    _ => unreachable!(),
                };
                if let Some(ty) = ty {
                    self.promote(
                        environment,
                        expression,
                        self.model.resolve_type_in(ty, self.type_parameters),
                    );
                }
            }
            Pattern::Cast(cast) if truth => self.promote(
                environment,
                expression,
                self.model
                    .resolve_type_in(cast.cast_type(), self.type_parameters),
            ),
            Pattern::Object(object) if truth => {
                let ty =
                    self.object_pattern_type(object.type_(), &self.infer(environment, expression));
                self.promote(environment, expression, ty)
            }
            Pattern::List(_) if truth => {
                self.promote(environment, expression, self.model.core_type("List", false))
            }
            Pattern::Map(_) if truth => {
                self.promote(environment, expression, self.model.core_type("Map", false))
            }
            Pattern::Record(_) if truth => self.promote(
                environment,
                expression,
                self.model.core_type("Record", false),
            ),
            _ => {}
        }
    }

    pub fn statement_exits(&self, environment: &TypeEnvironment, statement: Stmt<'_>) -> bool {
        exits(
            statement,
            &|expression| self.infer(environment, expression) == ResolvedType::NEVER,
            &|_| false,
            true,
        )
    }

    /// The element type a `for`-in loop binds from `iterable`; `await for` iterates a `Stream`.
    pub fn iterable_element(&self, iterable: &ResolvedType, is_await: bool) -> ResolvedType {
        let (library, name) = if is_await {
            ("dart:async", "Stream")
        } else {
            ("dart:core", "Iterable")
        };
        self.signatures
            .instantiated_supertype(iterable, library, name, self.model)
            .and_then(|ty| ty.type_argument(0))
            .unwrap_or(ResolvedType::UNKNOWN)
    }

    /// Declares every variable a declaration or matched pattern binds, typed from `matched`.
    pub fn declare_pattern(
        &self,
        environment: &mut TypeEnvironment,
        pattern: Pattern<'_>,
        matched: ResolvedType,
    ) {
        match pattern {
            Pattern::Variable(variable) => {
                let ty = variable.type_().map_or(matched, |ty| {
                    self.model.resolve_type_in(ty, self.type_parameters)
                });
                environment.declare(variable.name().name(), ty);
            }
            Pattern::List(list) => {
                let element = list
                    .type_arg()
                    .map(|ty| self.model.resolve_type_in(ty, self.type_parameters))
                    .or_else(|| {
                        self.signatures
                            .instantiated_supertype(&matched, "dart:core", "List", self.model)
                            .and_then(|ty| ty.type_argument(0))
                    })
                    .unwrap_or(ResolvedType::UNKNOWN);
                for child in list.elements() {
                    match child {
                        ListPatternElement::Pattern(child) => {
                            self.declare_pattern(environment, child, element.clone())
                        }
                        ListPatternElement::Rest(rest) => {
                            if let Some(child) = rest.pattern() {
                                self.declare_pattern(environment, child, matched.clone());
                            }
                        }
                    }
                }
            }
            Pattern::Record(record) => {
                let mut positional = 0;
                for field in record.fields() {
                    let ty = match field.name() {
                        Some(name) => matched.record_field(name.name()),
                        None => {
                            let ty = matched.record_positional(positional);
                            positional += 1;
                            ty
                        }
                    };
                    self.declare_pattern(
                        environment,
                        field.pattern(),
                        ty.unwrap_or(ResolvedType::UNKNOWN),
                    );
                }
            }
            Pattern::Map(map) => {
                let value = map
                    .type_args()
                    .nth(1)
                    .map(|ty| self.model.resolve_type_in(ty, self.type_parameters))
                    .or_else(|| {
                        self.signatures
                            .instantiated_supertype(&matched, "dart:core", "Map", self.model)
                            .and_then(|ty| ty.type_argument(1))
                    })
                    .unwrap_or(ResolvedType::UNKNOWN);
                for entry in map.entries() {
                    self.declare_pattern(environment, entry.pattern(), value.clone());
                }
            }
            Pattern::Object(object) => {
                let object_type = self.object_pattern_type(object.type_(), &matched);
                for field in object.fields() {
                    let ty = self
                        .signatures
                        .resolved_field(&object_type, field.name().name(), self.model)
                        .or_else(|| {
                            self.signatures.resolved_extension_getter(
                                &object_type,
                                field.name().name(),
                                self.model,
                            )
                        })
                        .map_or(ResolvedType::UNKNOWN, |(ty, substitutions)| {
                            self.model.substitute(&ty, &substitutions)
                        });
                    match field.pattern() {
                        Some(child) => self.declare_pattern(environment, child, ty),
                        None => environment.declare(field.name().name(), ty),
                    }
                }
            }
            Pattern::LogicalAnd(_) | Pattern::LogicalOr(_) => {
                let (left, right) = match pattern {
                    Pattern::LogicalAnd(logical) => (logical.left(), logical.right()),
                    Pattern::LogicalOr(logical) => (logical.left(), logical.right()),
                    _ => unreachable!(),
                };
                self.declare_pattern(environment, left, matched.clone());
                self.declare_pattern(environment, right, matched);
            }
            Pattern::Cast(cast) => self.declare_pattern(
                environment,
                cast.inner(),
                self.model
                    .resolve_type_in(cast.cast_type(), self.type_parameters),
            ),
            Pattern::NullCheck(_) | Pattern::NullAssert(_) => {
                let inner = match pattern {
                    Pattern::NullCheck(check) => check.inner(),
                    Pattern::NullAssert(assertion) => assertion.inner(),
                    _ => unreachable!(),
                };
                self.declare_pattern(environment, inner, matched.non_null())
            }
            Pattern::ParenPattern(paren) => {
                self.declare_pattern(environment, paren.inner(), matched)
            }
            Pattern::Wildcard(_)
            | Pattern::Literal(_)
            | Pattern::Const(_)
            | Pattern::Relational(_)
            | Pattern::Error(_) => {}
        }
    }

    /// The type an object pattern matches: written type arguments, or else the
    /// matched value's instantiation of the named declaration (`Box(...)`
    /// matching a `Box<T>` is `Box<T>`), falling back to the raw type.
    pub fn object_pattern_type(
        &self,
        written: DartType<'_>,
        matched: &ResolvedType,
    ) -> ResolvedType {
        let resolved = self.model.resolve_type_in(written, self.type_parameters);
        match &resolved {
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity,
                        arguments,
                        ..
                    },
            } if arguments.is_empty() => self
                .signatures
                .as_instance_of(matched, identity, self.model)
                .map_or(resolved.clone(), |instance| instance.with_nullable(false)),
            _ => resolved,
        }
    }

    /// Apply only after the shared visitor has evaluated the expression's children.
    /// `assigned` is the caller's type for an assignment's right-hand side;
    /// `None` falls back to this context's own inference.
    pub fn finish_expression(
        &self,
        environment: &mut TypeEnvironment,
        expression: Expr<'_>,
        assigned: Option<ResolvedType>,
    ) {
        match expression {
            Expr::As(cast) => {
                let ty = self
                    .model
                    .resolve_type_in(cast.dart_type(), self.type_parameters);
                if ty != ResolvedType::DYNAMIC {
                    self.promote(environment, cast.expr(), ty);
                }
            }
            Expr::NullAssert(assertion) => {
                let operand = assertion.operand();
                let ty = self.infer(environment, operand).non_null();
                self.promote(environment, operand, ty);
            }
            Expr::Assign(assignment) => {
                let target = assignment.target();
                if let Expr::Ident(name) = target {
                    let value =
                        assigned.unwrap_or_else(|| self.infer(environment, assignment.value()));
                    match assignment.op().binary() {
                        None => self.assign_variable(environment, name.name(), &value),
                        // Either `name` was non-null, or it was assigned `value`;
                        // the write is the whole expression, `NonNull(T1) ⊔ T2`.
                        Some(BinaryOp::NullCoalesce) => {
                            let mut non_null_path = environment.clone();
                            let read = non_null(&self.infer(environment, target));
                            if let Some(ty) = &read {
                                self.promote(&mut non_null_path, target, ty.clone());
                            }
                            let written = match read {
                                Some(read) if self.subtype(&value, &read) => read,
                                _ => value,
                            };
                            self.assign_variable(environment, name.name(), &written);
                            *environment = environment.join(&non_null_path);
                        }
                        Some(binary) => {
                            let written = operator_result(
                                binary,
                                &self.infer(environment, target),
                                &value,
                                self.model,
                            );
                            self.assign_variable(environment, name.name(), &written);
                        }
                    }
                }
            }
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                self.finish_increment(environment, unary.operand());
            }
            Expr::PostfixIncDec(postfix) => {
                self.finish_increment(environment, postfix.operand());
            }
            _ => {}
        }
    }

    fn finish_increment(&self, environment: &mut TypeEnvironment, operand: Expr<'_>) {
        if let Expr::Ident(name) = operand {
            let written = operator_result(
                BinaryOp::Add,
                &self.infer(environment, operand),
                &self.model.core_type("int", false),
                self.model,
            );
            self.assign_variable(environment, name.name(), &written);
        }
    }
}

/// `NonNull(ty)` for values with a resolved non-null refinement.
fn non_null(ty: &ResolvedType) -> Option<ResolvedType> {
    match ty {
        ResolvedType {
            kind: ResolvedTypeKind::Dynamic,
        }
        | ResolvedType {
            kind: ResolvedTypeKind::Unknown,
        }
        | ResolvedType {
            kind: ResolvedTypeKind::Void,
        }
        | ResolvedType {
            kind: ResolvedTypeKind::Null,
        } => None,
        ty => Some(ty.non_null()),
    }
}

fn same_shape(left: &ResolvedType, right: &ResolvedType) -> bool {
    match (left, right) {
        (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity: a,
                        arguments: aa,
                        extension_type: ae,
                        ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity: b,
                        arguments: ba,
                        extension_type: be,
                        ..
                    },
            },
        ) => a == b && aa == ba && ae == be,
        (
            ResolvedType {
                kind:
                    ResolvedTypeKind::TypeParameter {
                        id: a, bound: ab, ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::TypeParameter {
                        id: b, bound: bb, ..
                    },
            },
        ) => a == b && ab == bb,
        (
            ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { id: a, bound: ab },
            },
            ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { id: b, bound: bb },
            },
        ) => a == b && ab == bb,
        (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type: a,
                        positional: ap,
                        named: an,
                        optional_positional: ao,
                        shape: a_shape,
                        ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type: b,
                        positional: bp,
                        named: bn,
                        optional_positional: bo,
                        shape: b_shape,
                        ..
                    },
            },
        ) => a == b && ap == bp && an == bn && ao == bo && a_shape == b_shape,
        (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        positional: a,
                        named: an,
                        ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        positional: b,
                        named: bn,
                        ..
                    },
            },
        ) => a == b && an == bn,
        _ => left == right,
    }
}

struct CompletedPromotions<'a, 'facts, 'model> {
    context: &'a FlowContext<'facts, 'model>,
    environment: &'a mut TypeEnvironment,
}

impl<'tree> Visitor<'tree> for CompletedPromotions<'_, '_, '_> {
    fn visit_expr(&mut self, expression: Expr<'tree>) {
        match expression {
            Expr::FuncExpr(_) | Expr::Cascade(_) => return,
            Expr::Binary(binary)
                if matches!(
                    binary.op(),
                    BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce
                ) =>
            {
                self.visit_expr(binary.left())
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
            Expr::Conditional(conditional) => self.visit_expr(conditional.condition()),
            Expr::Switch(switch) => self.visit_expr(switch.subject()),
            Expr::Call(call) if matches!(call.callee(), Expr::Field(field) if field.is_null_safe()) => {
                self.visit_expr(call.callee())
            }
            _ => walk_expr(self, expression),
        }
        if matches!(expression, Expr::As(_) | Expr::NullAssert(_)) {
            self.context
                .finish_expression(self.environment, expression, None);
        }
    }
}

/// Whether control can reach the end of `statements`; a reachable block-closure end returns `null`.
pub fn completes_normally<'tree>(
    mut statements: AstChildren<'tree, Stmt<'tree>>,
    never: &impl Fn(Expr<'tree>) -> bool,
    exhaustive: &impl Fn(SwitchStmt<'tree>) -> bool,
) -> bool {
    !statements.any(|statement| exits(statement, never, exhaustive, true))
}

pub(super) fn exits<'tree>(
    statement: Stmt<'tree>,
    never: &impl Fn(Expr<'tree>) -> bool,
    exhaustive: &impl Fn(SwitchStmt<'tree>) -> bool,
    breaks: bool,
) -> bool {
    let block_exits = |mut statements: AstChildren<'tree, Stmt<'tree>>| {
        statements.any(|statement| exits(statement, never, exhaustive, breaks))
    };
    match statement {
        Stmt::Return(_) | Stmt::Throw(_) | Stmt::Rethrow(_) | Stmt::Continue(_) => true,
        Stmt::Break(statement) => breaks || statement.label().is_some(),
        Stmt::Expr(statement) => {
            matches!(statement.expr(), Expr::Throw(_)) || never(statement.expr())
        }
        Stmt::Block(block) => block_exits(block.stmts()),
        Stmt::If(statement) => {
            exits(statement.then_branch(), never, exhaustive, breaks)
                && statement
                    .else_branch()
                    .is_some_and(|statement| exits(statement, never, exhaustive, breaks))
        }
        Stmt::Labeled(statement) => {
            !breaks_to(Some(statement.label().name()), statement.stmt())
                && exits(statement.stmt(), never, exhaustive, breaks)
        }
        // Only an unconditional loop that nothing breaks out of never completes.
        Stmt::While(statement) => {
            always_true(Some(statement.condition())) && !breaks_to(None, statement.body())
        }
        Stmt::DoWhile(statement) => {
            always_true(Some(statement.condition())) && !breaks_to(None, statement.body())
        }
        Stmt::For(statement) => {
            matches!(
                statement.header().init(),
                None | Some(ForInit::VarDecl(_) | ForInit::Exprs(_))
            ) && always_true(statement.header().condition())
                && !breaks_to(None, statement.body())
        }
        Stmt::TryCatch(statement) => {
            (block_exits(statement.body().stmts())
                && statement
                    .catches()
                    .all(|catch| block_exits(catch.body().stmts())))
                || statement
                    .finally()
                    .is_some_and(|finally| block_exits(finally.stmts()))
        }
        // Cases never fall through; an empty case shares the next body, and `break` leaves the switch.
        Stmt::Switch(statement) => {
            (exhaustive(statement)
                || statement.cases().any(|case| {
                    case.cases()
                        .any(|kind| matches!(kind, SwitchCaseKind::Default(_)))
                }))
                && statement
                    .cases()
                    .last()
                    .is_some_and(|case| !case.body().is_empty())
                && statement
                    .cases()
                    .filter(|case| !case.body().is_empty())
                    .all(|case| {
                        case.body()
                            .any(|statement| exits(statement, never, exhaustive, false))
                    })
        }
        _ => false,
    }
}

fn always_true(condition: Option<Expr<'_>>) -> bool {
    condition.is_none_or(|condition| matches!(condition, Expr::BoolLit(value) if value.value()))
}

/// Whether `statement` contains a `break` leaving it: one naming `label`, or an
/// unlabeled one outside nested loops and switches when `label` is `None`.
fn breaks_to(label: Option<&str>, statement: Stmt<'_>) -> bool {
    let mut visitor = Breaks {
        label,
        found: false,
    };
    visitor.visit_stmt(statement);
    visitor.found
}

struct Breaks<'a> {
    label: Option<&'a str>,
    found: bool,
}

impl<'tree> Visitor<'tree> for Breaks<'_> {
    fn visit_stmt(&mut self, statement: Stmt<'tree>) {
        if self.found {
            return;
        }
        match statement {
            Stmt::Break(statement) if statement.label().map(|label| label.name()) == self.label => {
                self.found = true
            }
            Stmt::Labeled(statement) if self.label == Some(statement.label().name()) => {}
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) | Stmt::Switch(_)
                if self.label.is_none() => {}
            Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_expr(&mut self, _: Expr<'tree>) {}
}

/// Whether a loop `body` may leave an iteration early: by `break` when
/// `continues` is false, else by `continue`. Unlabeled jumps inside nested
/// loops (and, for `break`, switches) target those; any labeled jump is
/// assumed to target the loop.
pub fn loop_jumps(body: Stmt<'_>, continues: bool) -> bool {
    let mut jumps = Jumps {
        continues,
        shielded: 0,
        found: false,
    };
    jumps.visit_stmt(body);
    jumps.found
}

struct Jumps {
    continues: bool,
    shielded: usize,
    found: bool,
}

impl<'tree> Visitor<'tree> for Jumps {
    fn visit_stmt(&mut self, statement: Stmt<'tree>) {
        if self.found {
            return;
        }
        match statement {
            Stmt::Break(jump) if !self.continues => {
                self.found = jump.label().is_some() || self.shielded == 0
            }
            Stmt::Continue(jump) if self.continues => {
                self.found = jump.label().is_some() || self.shielded == 0
            }
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => {
                self.shielded += 1;
                walk_stmt(self, statement);
                self.shielded -= 1;
            }
            Stmt::Switch(_) if !self.continues => {
                self.shielded += 1;
                walk_stmt(self, statement);
                self.shielded -= 1;
            }
            Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_expr(&mut self, _: Expr<'tree>) {}
}

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;
    use falcon_syntax::cst::{FunctionBody, TopLevelDecl};

    use super::{ResolvedTypeKind, completes_normally};

    fn with_flow_context(test: impl FnOnce(&super::FlowContext<'_, '_>)) {
        use std::path::PathBuf;

        use crate::resolve::{IdentityIndex, IdentitySource, TypeIndex};

        let path = PathBuf::from("/project/lib/main.dart");
        let source = "import 'dart:async';";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(program);
        let model = super::SemanticModel::new(&path, &parsed, &identities, Some(&types));
        let signatures = super::SignatureIndex::from_program(program, &model);
        let type_parameters = super::TypeParameterScope::default();
        test(&super::FlowContext {
            model: &model,
            signatures: &signatures,
            type_parameters: &type_parameters,
            field_promotions: false,
        });
    }

    fn async_type(name: &'static str, argument: super::ResolvedType) -> super::ResolvedType {
        super::ResolvedType {
            kind: ResolvedTypeKind::Interface {
                occurrence: Default::default(),
                identity: super::DeclarationIdentity::sdk("dart:async", name),
                arguments: vec![argument].into(),
                nullable: false,
                extension_type: false,
            },
        }
    }

    #[test]
    fn future_or_is_covariant_including_nullable_value_branches() {
        with_flow_context(|context| {
            let int = context.model.core_type("int", false);
            let num = context.model.core_type("num", false);
            let value = async_type("FutureOr", int.clone());
            for argument in [
                num.clone(),
                int.with_nullable(true),
                num.with_nullable(true),
            ] {
                let target = async_type("FutureOr", argument);
                assert!(context.subtype(&value, &target), "{value:?} <: {target:?}");
            }
            assert!(!context.subtype(&async_type("FutureOr", num), &async_type("FutureOr", int),));
        });
    }

    #[test]
    fn future_or_accepts_its_parameter_and_future_bounded_parameters() {
        use crate::resolve::{ParameterOwner, ResolvedType, TypeParameterId};

        with_flow_context(|context| {
            let parameter = |name: &str, ordinal, bound: ResolvedType| ResolvedType {
                kind: ResolvedTypeKind::TypeParameter {
                    id: TypeParameterId {
                        owner: ParameterOwner::Sdk("test"),
                        ordinal,
                        name: name.into(),
                    },
                    bound: bound.into(),
                    nullable: false,
                },
            };
            let t = parameter("T", 0, context.model.core_type("Object", true));
            assert!(context.subtype(&t, &async_type("FutureOr", t.clone())));
            let int = context.model.core_type("int", false);
            let f = parameter("F", 1, async_type("Future", int.clone()));
            assert!(context.subtype(&f, &async_type("FutureOr", int.clone())));
            assert!(!context.subtype(
                &f,
                &async_type("FutureOr", context.model.core_type("String", false)),
            ));
            let nullable_future = async_type("Future", int.clone()).with_nullable(true);
            assert!(!context.subtype(&nullable_future, &async_type("FutureOr", int.clone())));
            assert!(context.subtype(
                &nullable_future,
                &async_type("FutureOr", int.with_nullable(true)),
            ));
        });
    }

    #[test]
    fn parameter_is_subtype_of_its_potentially_nullable_extension_bound() {
        use crate::resolve::{ParameterOwner, ResolvedType, TypeParameterId};

        with_flow_context(|context| {
            let bound = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: super::DeclarationIdentity::sdk("test", "IntBox"),
                    arguments: Default::default(),
                    nullable: false,
                    extension_type: true,
                },
            };
            let parameter = |bound: ResolvedType| ResolvedType {
                kind: ResolvedTypeKind::TypeParameter {
                    id: TypeParameterId {
                        owner: ParameterOwner::Sdk("test"),
                        ordinal: 0,
                        name: "T".into(),
                    },
                    bound: bound.into(),
                    nullable: false,
                },
            };
            let exact = parameter(bound.clone());
            assert!(context.subtype(&exact, &bound));
            assert_eq!(context.factor(&exact, &bound), ResolvedType::NEVER);
            let nullable = parameter(bound.with_nullable(true));
            assert!(!context.subtype(&nullable, &bound));
            assert_eq!(context.factor(&nullable, &bound), nullable);
        });
    }

    #[test]
    fn null_assert_patterns_promote_on_matched_and_failed_paths() {
        use falcon_syntax::cst::{IfCondition, Stmt};

        for pattern in ["final y!", "1!", "(final y!)"] {
            let source = format!("void f(Object? x) {{ if (x case {pattern}) {{}} }}");
            let parsed = parse(&source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let parsed = std::sync::Arc::new(crate::ParsedFile {
                source: source.into_boxed_str(),
                tree: parsed.tree,
            });
            let program = parsed.program();
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("function");
            };
            let FunctionBody::Block(block) = function.body().unwrap() else {
                panic!("block");
            };
            let Stmt::If(statement) = block.stmts().next().unwrap() else {
                panic!("if");
            };
            let IfCondition::Case(case) = statement.condition() else {
                panic!("case");
            };
            let expression = case.expr();
            let pattern = case.pattern();
            with_flow_context(|context| {
                for truth in [true, false] {
                    let mut environment = super::TypeEnvironment::new();
                    environment.declare("x", context.model.core_type("Object", true));
                    context.assume_pattern(&mut environment, expression, pattern, truth);
                    assert!(!environment.lookup("x").nullable(), "{pattern:?}, {truth}");
                }
            });
        }
    }

    #[test]
    fn assignments_preserve_only_compatible_future_or_promotions() {
        with_flow_context(|context| {
            let int = context.model.core_type("int", false);
            let interest = async_type("FutureOr", context.model.core_type("num", false));
            let future = async_type("Future", int.clone());
            let declared = context.model.core_type("Object", true);
            for (written, preserves) in [
                (async_type("FutureOr", int), true),
                (future.clone(), true),
                (future.with_nullable(true), false),
                (context.model.core_type("String", false), false),
            ] {
                let mut environment = super::TypeEnvironment::new();
                environment.declare("value", declared.clone());
                environment.promote("value", interest.clone());
                context.assign_variable(&mut environment, "value", &written);
                assert_eq!(
                    environment.lookup("value"),
                    if preserves {
                        interest.clone()
                    } else {
                        declared.with_nullable(written.nullable())
                    },
                    "assignment of {written:?}",
                );
            }
        });
    }

    #[test]
    fn negative_type_tests_factor_nullable_and_future_or_unions() {
        use std::path::PathBuf;

        use falcon_syntax::cst::{IfCondition, Stmt};

        use crate::resolve::{
            IdentityIndex, IdentitySource, ResolvedType, SemanticModel, SignatureIndex,
            TypeEnvironment, TypeIndex, TypeParameterScope,
        };

        let source = concat!(
            "import 'dart:async'; class C {} ",
            "Future<T> future<T>(FutureOr<T> value) { if (value is T) throw 0; return value; } ",
            "T synchronous<T>(FutureOr<T> value) { if (value is Future<T>) throw 0; return value; } ",
            "Future<T> nullableFuture<T>(FutureOr<T>? value) { if (value is T?) throw 0; return value; } ",
            "T? nullableValue<T>(FutureOr<T>? value) { if (value is Future<T>) throw 0; return value; } ",
            "Null nullOnly(C? value) { if (value is C) throw 0; return value; } ",
            "C nonNull(C? value) { if (value is Null) throw 0; return value; } ",
            "Object unchanged(Object value) { if (value is String) throw 0; return value; } ",
            "FutureOr<int> unrelated(FutureOr<int> value) { if (value is String) throw 0; return value; } ",
            "Future<T> negated<T>(FutureOr<T> value) { if (value is! T) return value; throw 0; } ",
            "Null nullFunction(void Function()? value) { if (value is void Function()) throw 0; return value; } ",
            "Null nullRecord((int, String)? value) { if (value is (int, String)) throw 0; return value; } ",
            "String assignment(Object? value) { if (value is String?) throw 0; value = 'a'; return value; } ",
            "int exhausted(int value) { if (value is int) throw 0; return value; } ",
        );
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let path = PathBuf::from("/project/lib/main.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(program);
        let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(program, &model);
        for declaration in program.declarations() {
            let TopLevelDecl::Function(function) = declaration else {
                continue;
            };
            let mut scope = TypeParameterScope::default();
            scope.push(function.type_params(), &model);
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), &model, &scope);
            let Some(FunctionBody::Block(block)) = function.body() else {
                panic!("expected block");
            };
            let Stmt::If(statement) = block.stmts().next().unwrap() else {
                panic!("expected guard");
            };
            let IfCondition::Expr(condition) = statement.condition() else {
                panic!("expected expression condition");
            };
            let context = super::FlowContext {
                model: &model,
                signatures: &signatures,
                type_parameters: &scope,
                field_promotions: false,
            };
            context.assume_condition(
                &mut environment,
                condition,
                function.name().name() == "negated",
            );
            let super::Expr::Is(test) = condition else {
                panic!("expected type test");
            };
            let tested = model.resolve_type_in(test.dart_type(), &scope);
            assert_eq!(
                environment.tested_types("value"),
                std::slice::from_ref(&tested)
            );
            if function.name().name() == "assignment" {
                context.assign_variable(
                    &mut environment,
                    "value",
                    &model.core_type("String", false),
                );
            }
            let expected = model.resolve_type_in(function.return_type().unwrap(), &scope);
            assert_eq!(
                environment.lookup("value"),
                expected,
                "{}",
                function.name().name()
            );
            assert_eq!(context.factor(&expected, &expected), ResolvedType::NEVER);
            assert_eq!(
                context.factor(&ResolvedType::UNKNOWN, &expected),
                ResolvedType::UNKNOWN,
            );
            assert_eq!(context.factor(&expected, &ResolvedType::UNKNOWN), expected);
            let mut unresolved = environment.declared_type("value");
            match &mut unresolved {
                ResolvedType {
                    kind: ResolvedTypeKind::Interface { arguments, .. },
                } if !arguments.is_empty() => {
                    std::sync::Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
                }
                ResolvedType {
                    kind: ResolvedTypeKind::Function { return_type, .. },
                } => {
                    *std::sync::Arc::make_mut(return_type) = ResolvedType::UNKNOWN;
                }
                ResolvedType {
                    kind: ResolvedTypeKind::Record { positional, .. },
                } if !positional.is_empty() => {
                    std::sync::Arc::make_mut(positional)[0] = ResolvedType::UNKNOWN;
                }
                _ => continue,
            }
            assert_eq!(context.factor(&unresolved, &unresolved), unresolved);
            assert_eq!(context.factor(&expected, &unresolved), expected);
        }
    }

    #[test]
    fn non_null_parameter_promotions_preserve_the_intersection() {
        use crate::resolve::{DeclarationIdentity, ParameterOwner, ResolvedType, TypeParameterId};

        let parameter = ResolvedType {
            kind: ResolvedTypeKind::TypeParameter {
                id: TypeParameterId {
                    owner: ParameterOwner::Sdk("test"),
                    ordinal: 0,
                    name: "T".into(),
                },
                bound: ResolvedType::interface_identity(DeclarationIdentity::OBJECT)
                    .with_nullable(true)
                    .into(),
                nullable: true,
            },
        };
        let promoted = super::non_null(&parameter).unwrap();
        assert!(matches!(
            promoted,
            ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { .. }
            }
        ));
        assert!(!super::same_shape(
            &promoted,
            &parameter.with_nullable(false)
        ));
        assert!(super::same_shape(&promoted, &promoted));
    }

    #[test]
    fn block_ends_are_reachable_unless_every_path_leaves() {
        let cases = [
            ("{}", true),
            ("{ if (flag) return 1; }", true),
            ("{ if (flag) return 1; else throw 0; }", false),
            ("{ while (true) { if (flag) return 1; } }", false),
            ("{ while (true) { if (flag) break; } }", true),
            ("{ while (flag) { return 1; } }", true),
            (
                "{ for (;;) { switch (flag) { case true: break; default: return 1; } } }",
                false,
            ),
            (
                "{ outer: while (true) { while (true) { break outer; } } }",
                true,
            ),
            ("{ for (final x in [1]) { return x; } }", true),
            ("{ try { return 1; } catch (e) { rethrow; } }", false),
            ("{ try { return 1; } catch (e) {} }", true),
            ("{ try {} finally { return 1; } }", false),
            (
                "{ switch (flag) { case true: return 1; default: throw 0; } }",
                false,
            ),
            ("{ switch (flag) { case true: return 1; } }", true),
            (
                "{ switch (flag) { case true: break; default: return 1; } }",
                true,
            ),
        ];
        for (body, expected) in cases {
            let source = format!("f(bool flag) {body}");
            let parsed = parse(&source);
            assert!(parsed.errors.is_empty(), "{body}: {:?}", parsed.errors);
            let parsed = std::sync::Arc::new(crate::ParsedFile {
                source: source.into_boxed_str(),
                tree: parsed.tree,
            });
            let program = parsed.program();
            let Some(TopLevelDecl::Function(function)) = program.declarations().next() else {
                panic!("{body}")
            };
            let Some(FunctionBody::Block(block)) = function.body() else {
                panic!("{body}")
            };
            assert_eq!(
                completes_normally(block.stmts(), &|_| false, &|_| false),
                expected,
                "{body}"
            );
        }
    }
}
