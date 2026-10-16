//! Flow transfers used by the shared syntax visitor; no independent AST walk.
use falcon_syntax::ast::{
    BinaryOp, DartType, Expr, ForInit, ListPatternElement, LiteralPatternValue, Pattern, Stmt,
    SwitchCaseKind, UnaryOp,
};
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use super::semantic::operator_result;
use super::{
    DeclarationIdentity, ResolvedType, SemanticModel, SignatureIndex, TypeEnvironment,
    TypeParameterScope,
};

pub struct FlowContext<'a, 'model> {
    pub model: &'a SemanticModel<'model>,
    pub signatures: &'a SignatureIndex,
    pub type_parameters: &'a TypeParameterScope,
    pub field_promotions: bool,
}

impl FlowContext<'_, '_> {
    fn infer(&self, environment: &TypeEnvironment, expression: &Expr) -> ResolvedType {
        if let Expr::Ident(name) = expression
            && !environment.is_bound(&name.name)
        {
            if let Some(ty) = environment.promoted_this_field(&name.name) {
                return ty;
            }
            let receiver = environment.lookup("this");
            if let Some((ty, substitutions)) = self
                .signatures
                .resolved_field(&receiver, &name.name, self.model)
                .or_else(|| {
                    self.signatures
                        .resolved_extension_getter(&receiver, &name.name, self.model)
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
        if let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = target
            && identity.is_sdk("dart:async", "FutureOr")
            && let Some(argument) = arguments.first()
        {
            // A written nullable union retains its null arm independently of
            // the argument: T? <: FutureOr<T>? follows from T <: FutureOr<T>.
            if target.nullable() {
                if matches!(value, ResolvedType::Null) {
                    return true;
                }
                if value.nullable()
                    && !matches!(value, ResolvedType::Dynamic | ResolvedType::Unknown)
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
                ResolvedType::Interface {
                    identity,
                    arguments,
                    ..
                } if identity.is_sdk("dart:async", "FutureOr") => arguments
                    .first()
                    .is_some_and(|value| self.subtype(value, argument)),
                ResolvedType::TypeParameter { bound, .. } => self.subtype(bound, target),
                ResolvedType::NonNullTypeParameter { bound, .. } => {
                    self.subtype(&bound.non_null(), target)
                }
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
        if matches!(value, ResolvedType::Function { .. })
            && target.interface("dart:core", "Function")
        {
            return true;
        }
        if let (
            ResolvedType::NonNullTypeParameter { id: value, .. },
            ResolvedType::TypeParameter { id: target, .. },
        ) = (value, target)
        {
            return value == target;
        }
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = target
        else {
            return false;
        };
        // Every resolved type but `Null` (rejected above by nullability) is an `Object`.
        if *identity == DeclarationIdentity::OBJECT {
            return !matches!(
                value,
                ResolvedType::Dynamic | ResolvedType::Unknown | ResolvedType::Void
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
            (ResolvedType::Unknown, _) => false,
            (_, ResolvedType::Dynamic | ResolvedType::Void) => true,
            (_, ResolvedType::Interface { identity, .. })
                if identity.is_sdk("dart:async", "FutureOr") =>
            {
                self.compatible(value, target)
            }
            // A type parameter that may be instantiated with a nullable type
            // reaches a non-nullable type only through itself.
            (ResolvedType::TypeParameter { bound, .. }, _)
                if !target.nullable() && !bound.is_non_nullable() =>
            {
                !value.nullable() && same_shape(value, target)
            }
            _ => {
                self.compatible(value, target)
                    || self.signatures.subtype(value, target, self.model) == super::Truth::Yes
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
            return ResolvedType::Never;
        }
        match value {
            ResolvedType::Interface { nullable: true, .. }
            | ResolvedType::TypeParameter { nullable: true, .. }
            | ResolvedType::Function { nullable: true, .. }
            | ResolvedType::Record { nullable: true, .. } => {
                let remainder = self.factor(&value.with_nullable(false), excluded);
                if self.subtype(&ResolvedType::Null, excluded) {
                    remainder
                } else {
                    remainder.with_nullable(true)
                }
            }
            ResolvedType::Interface { identity, .. }
                if identity.is_sdk("dart:async", "FutureOr") =>
            {
                let Some(argument) = value.type_argument(0) else {
                    return value.clone();
                };
                let future = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:async", "Future"),
                    arguments: vec![argument.clone()].into(),
                    nullable: false,
                    extension_type: false,
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

    fn promote(&self, environment: &mut TypeEnvironment, expression: &Expr, ty: ResolvedType) {
        if let Expr::Ident(name) = expression {
            environment.record_tested_type(&name.name, ty.clone());
        }
        self.promote_without_test(environment, expression, ty);
    }

    fn promote_without_test(
        &self,
        environment: &mut TypeEnvironment,
        expression: &Expr,
        mut ty: ResolvedType,
    ) {
        let current = self.infer(environment, expression);
        if matches!(ty, ResolvedType::Dynamic | ResolvedType::Unknown) {
            return;
        }
        if self.subtype(&current, &ty) {
            return;
        }
        if current.nullable()
            && !ty.nullable()
            && !matches!(current, ResolvedType::Dynamic | ResolvedType::Unknown)
        {
            let non_null = current.non_null();
            if self.subtype(&non_null, &ty) {
                ty = non_null;
            }
        }
        let compatible = match &current {
            ResolvedType::Dynamic => true,
            ResolvedType::TypeParameter { bound, .. } => self.subtype(&ty, bound),
            _ => self.subtype(&ty, &current),
        };
        if !compatible {
            return;
        }
        match expression.unparenthesized() {
            Expr::Ident(name) if environment.is_bound(&name.name) => {
                environment.promote_without_test(&name.name, ty)
            }
            Expr::Ident(name)
                if self.field_promotions
                    && self.signatures.is_promotable_field(
                        &environment.lookup("this"),
                        &name.name,
                        self.model,
                    ) =>
            {
                environment.promote_this_field(&name.name, ty)
            }
            Expr::Field {
                object,
                field,
                is_null_safe: false,
                ..
            } if self.field_promotions
                && self.signatures.is_promotable_field(
                    &self.infer(environment, object),
                    &field.name,
                    self.model,
                ) =>
            {
                environment.promote_field(object, &field.name, ty)
            }
            _ => {}
        }
    }

    pub fn assume_condition(
        &self,
        environment: &mut TypeEnvironment,
        expression: &Expr,
        truth: bool,
    ) {
        let expression = expression.unparenthesized();
        match expression {
            Expr::Unary {
                op: UnaryOp::Bang,
                operand,
                ..
            } => self.assume_condition(environment, operand, !truth),
            Expr::Binary {
                op: BinaryOp::And,
                left,
                right,
                ..
            } if truth => {
                self.assume_condition(environment, left, true);
                self.assume_condition(environment, right, true);
            }
            Expr::Binary {
                op: BinaryOp::Or,
                left,
                right,
                ..
            } if !truth => {
                self.assume_condition(environment, left, false);
                self.assume_condition(environment, right, false);
            }
            _ => {
                CompletedPromotions {
                    context: self,
                    environment,
                }
                .visit_expr(expression);
                match expression {
                    Expr::Is {
                        expr,
                        dart_type,
                        negated,
                        ..
                    } => {
                        let tested = self.model.resolve_type_in(dart_type, self.type_parameters);
                        if let Expr::Ident(name) = expr.unparenthesized() {
                            environment.record_tested_type(&name.name, tested.clone());
                        }
                        let ty = if truth != *negated {
                            tested
                        } else {
                            self.factor(&self.infer(environment, expr), &tested)
                        };
                        // An impossible type-test branch does not change the
                        // variable's static type to Never.
                        if ty != ResolvedType::Never {
                            self.promote_without_test(environment, expr, ty);
                        }
                    }
                    Expr::Binary {
                        op: BinaryOp::EqEq | BinaryOp::NotEq,
                        left,
                        right,
                        ..
                    } => {
                        let operand = if matches!(right.unparenthesized(), Expr::NullLit { .. }) {
                            left.as_ref()
                        } else if matches!(left.unparenthesized(), Expr::NullLit { .. }) {
                            right.as_ref()
                        } else {
                            return;
                        };
                        let non_null = matches!(
                            expression,
                            Expr::Binary {
                                op: BinaryOp::NotEq,
                                ..
                            }
                        ) == truth;
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
        expression: &Expr,
        pattern: &Pattern,
        truth: bool,
    ) {
        match pattern {
            Pattern::ParenPattern { inner, .. } => {
                self.assume_pattern(environment, expression, inner, truth)
            }
            Pattern::Literal(literal) if matches!(literal.value, LiteralPatternValue::Null) => {
                if !truth {
                    self.promote(
                        environment,
                        expression,
                        self.infer(environment, expression).non_null(),
                    );
                }
            }
            Pattern::NullAssert { inner, .. } => {
                self.promote(
                    environment,
                    expression,
                    self.infer(environment, expression).non_null(),
                );
                self.assume_pattern(environment, expression, inner, truth);
            }
            Pattern::NullCheck { inner, .. } if truth => {
                self.promote(
                    environment,
                    expression,
                    self.infer(environment, expression).non_null(),
                );
                self.assume_pattern(environment, expression, inner, true);
            }
            Pattern::LogicalAnd { left, right, .. } if truth => {
                self.assume_pattern(environment, expression, left, true);
                self.assume_pattern(environment, expression, right, true);
            }
            Pattern::LogicalOr { left, right, .. } if !truth => {
                self.assume_pattern(environment, expression, left, false);
                self.assume_pattern(environment, expression, right, false);
            }
            Pattern::LogicalOr { left, right, .. } if truth => {
                let mut alternative = environment.clone();
                self.assume_pattern(environment, expression, left, true);
                self.assume_pattern(&mut alternative, expression, right, true);
                *environment = environment.join(&alternative);
            }
            Pattern::Wildcard {
                type_: Some(ty), ..
            }
            | Pattern::Variable {
                type_: Some(ty), ..
            }
            | Pattern::Cast { cast_type: ty, .. }
                if truth =>
            {
                self.promote(
                    environment,
                    expression,
                    self.model.resolve_type_in(ty, self.type_parameters),
                )
            }
            Pattern::Object(object) if truth => {
                let ty =
                    self.object_pattern_type(&object.type_, &self.infer(environment, expression));
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

    pub fn statement_exits(&self, environment: &TypeEnvironment, statement: &Stmt) -> bool {
        exits(
            statement,
            &|expression| self.infer(environment, expression) == ResolvedType::Never,
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
            .unwrap_or(ResolvedType::Unknown)
    }

    /// Declares every variable a declaration or matched pattern binds, typed from `matched`.
    pub fn declare_pattern(
        &self,
        environment: &mut TypeEnvironment,
        pattern: &Pattern,
        matched: ResolvedType,
    ) {
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let ty = type_.as_ref().map_or(matched, |ty| {
                    self.model.resolve_type_in(ty, self.type_parameters)
                });
                environment.declare(name.name.clone(), ty);
            }
            Pattern::List(list) => {
                let element = list
                    .type_arg
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, self.type_parameters))
                    .or_else(|| {
                        self.signatures
                            .instantiated_supertype(&matched, "dart:core", "List", self.model)
                            .and_then(|ty| ty.type_argument(0))
                    })
                    .unwrap_or(ResolvedType::Unknown);
                for child in &list.elements {
                    match child {
                        ListPatternElement::Pattern(child) => {
                            self.declare_pattern(environment, child, element.clone())
                        }
                        ListPatternElement::Rest(Some(child), _) => {
                            self.declare_pattern(environment, child, matched.clone())
                        }
                        ListPatternElement::Rest(None, _) => {}
                    }
                }
            }
            Pattern::Record(record) => {
                let mut positional = 0;
                for field in &record.fields {
                    let ty = match &field.name {
                        Some(name) => matched.record_field(&name.name),
                        None => {
                            let ty = matched.record_positional(positional);
                            positional += 1;
                            ty
                        }
                    };
                    self.declare_pattern(
                        environment,
                        &field.pattern,
                        ty.unwrap_or(ResolvedType::Unknown),
                    );
                }
            }
            Pattern::Map(map) => {
                let value = map
                    .type_args
                    .get(1)
                    .map(|ty| self.model.resolve_type_in(ty, self.type_parameters))
                    .or_else(|| {
                        self.signatures
                            .instantiated_supertype(&matched, "dart:core", "Map", self.model)
                            .and_then(|ty| ty.type_argument(1))
                    })
                    .unwrap_or(ResolvedType::Unknown);
                for entry in &map.entries {
                    self.declare_pattern(environment, &entry.pattern, value.clone());
                }
            }
            Pattern::Object(object) => {
                let object_type = self.object_pattern_type(&object.type_, &matched);
                for field in &object.fields {
                    let ty = self
                        .signatures
                        .resolved_field(&object_type, &field.name.name, self.model)
                        .or_else(|| {
                            self.signatures.resolved_extension_getter(
                                &object_type,
                                &field.name.name,
                                self.model,
                            )
                        })
                        .map_or(ResolvedType::Unknown, |(ty, substitutions)| {
                            self.model.substitute(&ty, &substitutions)
                        });
                    match &field.pattern {
                        Some(child) => self.declare_pattern(environment, child, ty),
                        None => environment.declare(field.name.name.clone(), ty),
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.declare_pattern(environment, left, matched.clone());
                self.declare_pattern(environment, right, matched);
            }
            Pattern::Cast {
                inner, cast_type, ..
            } => self.declare_pattern(
                environment,
                inner,
                self.model.resolve_type_in(cast_type, self.type_parameters),
            ),
            Pattern::NullCheck { inner, .. } | Pattern::NullAssert { inner, .. } => {
                self.declare_pattern(environment, inner, matched.non_null())
            }
            Pattern::ParenPattern { inner, .. } => {
                self.declare_pattern(environment, inner, matched)
            }
            Pattern::Wildcard { .. }
            | Pattern::Literal(_)
            | Pattern::Const(_)
            | Pattern::Relational { .. }
            | Pattern::Error { .. } => {}
        }
    }

    /// The type an object pattern matches: written type arguments, or else the
    /// matched value's instantiation of the named declaration (`Box(...)`
    /// matching a `Box<T>` is `Box<T>`), falling back to the raw type.
    pub fn object_pattern_type(&self, written: &DartType, matched: &ResolvedType) -> ResolvedType {
        let resolved = self.model.resolve_type_in(written, self.type_parameters);
        match &resolved {
            ResolvedType::Interface {
                identity,
                arguments,
                ..
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
        expression: &Expr,
        assigned: Option<ResolvedType>,
    ) {
        match expression {
            Expr::As {
                expr, dart_type, ..
            } => {
                let ty = self.model.resolve_type_in(dart_type, self.type_parameters);
                if ty != ResolvedType::Dynamic {
                    self.promote(environment, expr, ty);
                }
            }
            Expr::NullAssert { operand, .. } => {
                let ty = self.infer(environment, operand).non_null();
                self.promote(environment, operand, ty);
            }
            Expr::Assign {
                target, op, value, ..
            } => {
                if let Expr::Ident(name) = target.as_ref() {
                    let value = assigned.unwrap_or_else(|| self.infer(environment, value));
                    match op.binary() {
                        None => self.assign_variable(environment, &name.name, &value),
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
                            self.assign_variable(environment, &name.name, &written);
                            *environment = environment.join(&non_null_path);
                        }
                        Some(binary) => {
                            let written = operator_result(
                                binary,
                                &self.infer(environment, target),
                                &value,
                                self.model,
                            );
                            self.assign_variable(environment, &name.name, &written);
                        }
                    }
                }
            }
            Expr::Unary {
                op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                operand,
                ..
            }
            | Expr::PostfixIncDec { operand, .. } => {
                if let Expr::Ident(name) = operand.as_ref() {
                    let written = operator_result(
                        BinaryOp::Add,
                        &self.infer(environment, operand),
                        &self.model.core_type("int", false),
                        self.model,
                    );
                    self.assign_variable(environment, &name.name, &written);
                }
            }
            _ => {}
        }
    }
}

/// `NonNull(ty)` for values with a resolved non-null refinement.
fn non_null(ty: &ResolvedType) -> Option<ResolvedType> {
    match ty {
        ResolvedType::Dynamic | ResolvedType::Unknown | ResolvedType::Void | ResolvedType::Null => {
            None
        }
        ty => Some(ty.non_null()),
    }
}

fn same_shape(left: &ResolvedType, right: &ResolvedType) -> bool {
    match (left, right) {
        (
            ResolvedType::Interface {
                identity: a,
                arguments: aa,
                extension_type: ae,
                ..
            },
            ResolvedType::Interface {
                identity: b,
                arguments: ba,
                extension_type: be,
                ..
            },
        ) => a == b && aa == ba && ae == be,
        (
            ResolvedType::TypeParameter {
                id: a, bound: ab, ..
            },
            ResolvedType::TypeParameter {
                id: b, bound: bb, ..
            },
        ) => a == b && ab == bb,
        (
            ResolvedType::NonNullTypeParameter { id: a, bound: ab },
            ResolvedType::NonNullTypeParameter { id: b, bound: bb },
        ) => a == b && ab == bb,
        (
            ResolvedType::Function {
                return_type: a,
                positional: ap,
                named: an,
                optional_positional: ao,
                shape: a_shape,
                ..
            },
            ResolvedType::Function {
                return_type: b,
                positional: bp,
                named: bn,
                optional_positional: bo,
                shape: b_shape,
                ..
            },
        ) => a == b && ap == bp && an == bn && ao == bo && a_shape == b_shape,
        (
            ResolvedType::Record {
                positional: a,
                named: an,
                ..
            },
            ResolvedType::Record {
                positional: b,
                named: bn,
                ..
            },
        ) => a == b && an == bn,
        _ => left == right,
    }
}

struct CompletedPromotions<'a, 'facts, 'model> {
    context: &'a FlowContext<'facts, 'model>,
    environment: &'a mut TypeEnvironment,
}

impl Visitor for CompletedPromotions<'_, '_, '_> {
    fn visit_expr(&mut self, expression: &Expr) {
        match expression {
            Expr::FuncExpr { .. } | Expr::Cascade { .. } => return,
            Expr::Binary {
                op: BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce,
                left,
                ..
            } => self.visit_expr(left),
            Expr::Conditional { condition, .. } => self.visit_expr(condition),
            Expr::Switch { subject, .. } => self.visit_expr(subject),
            Expr::Call { callee, .. }
                if matches!(
                    callee.as_ref(),
                    Expr::Field {
                        is_null_safe: true,
                        ..
                    }
                ) =>
            {
                self.visit_expr(callee)
            }
            _ => walk_expr(self, expression),
        }
        if matches!(expression, Expr::As { .. } | Expr::NullAssert { .. }) {
            self.context
                .finish_expression(self.environment, expression, None);
        }
    }
}

/// Whether control can reach the end of `statements`; a reachable block-closure end returns `null`.
pub fn completes_normally(
    statements: &[Stmt],
    never: &impl Fn(&Expr) -> bool,
    exhaustive: &impl Fn(&falcon_syntax::ast::SwitchStmt) -> bool,
) -> bool {
    !statements
        .iter()
        .any(|statement| exits(statement, never, exhaustive, true))
}

fn exits(
    statement: &Stmt,
    never: &impl Fn(&Expr) -> bool,
    exhaustive: &impl Fn(&falcon_syntax::ast::SwitchStmt) -> bool,
    breaks: bool,
) -> bool {
    let block_exits = |statements: &[Stmt]| {
        statements
            .iter()
            .any(|statement| exits(statement, never, exhaustive, breaks))
    };
    match statement {
        Stmt::Return(_) | Stmt::Throw(_) | Stmt::Rethrow(_) | Stmt::Continue(_) => true,
        Stmt::Break(statement) => breaks || statement.label.is_some(),
        Stmt::Expr(statement) => {
            matches!(statement.expr, Expr::Throw { .. }) || never(&statement.expr)
        }
        Stmt::Block(block) => block_exits(&block.stmts),
        Stmt::If(statement) => {
            exits(&statement.then_branch, never, exhaustive, breaks)
                && statement
                    .else_branch
                    .as_deref()
                    .is_some_and(|statement| exits(statement, never, exhaustive, breaks))
        }
        Stmt::Labeled(statement) => {
            !breaks_to(Some(&statement.label.name), &statement.stmt)
                && exits(&statement.stmt, never, exhaustive, breaks)
        }
        // Only an unconditional loop that nothing breaks out of never completes.
        Stmt::While(statement) => {
            always_true(Some(&statement.condition)) && !breaks_to(None, &statement.body)
        }
        Stmt::DoWhile(statement) => {
            always_true(Some(&statement.condition)) && !breaks_to(None, &statement.body)
        }
        Stmt::For(statement) => {
            matches!(
                statement.header.init,
                None | Some(ForInit::VarDecl(_) | ForInit::Exprs(_))
            ) && always_true(statement.header.condition.as_ref())
                && !breaks_to(None, &statement.body)
        }
        Stmt::TryCatch(statement) => {
            (block_exits(&statement.body.stmts)
                && statement
                    .catches
                    .iter()
                    .all(|catch| block_exits(&catch.body.stmts)))
                || statement
                    .finally
                    .as_ref()
                    .is_some_and(|finally| block_exits(&finally.stmts))
        }
        // Cases never fall through; an empty case shares the next body, and `break` leaves the switch.
        Stmt::Switch(statement) => {
            (exhaustive(statement)
                || statement.cases.iter().any(|case| {
                    case.cases
                        .iter()
                        .any(|kind| matches!(kind, SwitchCaseKind::Default))
                }))
                && statement
                    .cases
                    .last()
                    .is_some_and(|case| !case.body.is_empty())
                && statement
                    .cases
                    .iter()
                    .filter(|case| !case.body.is_empty())
                    .all(|case| {
                        case.body
                            .iter()
                            .any(|statement| exits(statement, never, exhaustive, false))
                    })
        }
        _ => false,
    }
}

fn always_true(condition: Option<&Expr>) -> bool {
    condition.is_none_or(|condition| matches!(condition, Expr::BoolLit { value: true, .. }))
}

/// Whether `statement` contains a `break` leaving it: one naming `label`, or an
/// unlabeled one outside nested loops and switches when `label` is `None`.
fn breaks_to(label: Option<&str>, statement: &Stmt) -> bool {
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

impl Visitor for Breaks<'_> {
    fn visit_stmt(&mut self, statement: &Stmt) {
        if self.found {
            return;
        }
        match statement {
            Stmt::Break(statement)
                if statement.label.as_ref().map(|label| label.name.as_str()) == self.label =>
            {
                self.found = true
            }
            Stmt::Labeled(statement) if self.label == Some(statement.label.name.as_str()) => {}
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) | Stmt::Switch(_)
                if self.label.is_none() => {}
            Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_expr(&mut self, _: &Expr) {}
}

/// Whether a loop `body` may leave an iteration early: by `break` when
/// `continues` is false, else by `continue`. Unlabeled jumps inside nested
/// loops (and, for `break`, switches) target those; any labeled jump is
/// assumed to target the loop.
pub fn loop_jumps(body: &Stmt, continues: bool) -> bool {
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

impl Visitor for Jumps {
    fn visit_stmt(&mut self, statement: &Stmt) {
        if self.found {
            return;
        }
        match statement {
            Stmt::Break(jump) if !self.continues => {
                self.found = jump.label.is_some() || self.shielded == 0
            }
            Stmt::Continue(jump) if self.continues => {
                self.found = jump.label.is_some() || self.shielded == 0
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

    fn visit_expr(&mut self, _: &Expr) {}
}

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;
    use falcon_syntax::ast::{FunctionBody, TopLevelDecl};

    use super::completes_normally;

    fn with_flow_context(test: impl FnOnce(&super::FlowContext<'_, '_>)) {
        use std::path::PathBuf;

        use crate::resolve::{IdentityIndex, IdentitySource, TypeIndex};

        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse("import 'dart:async';");
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let model = super::SemanticModel::new(&path, &identities, Some(&types));
        let signatures = super::SignatureIndex::from_program(&program, &model);
        let type_parameters = super::TypeParameterScope::default();
        test(&super::FlowContext {
            model: &model,
            signatures: &signatures,
            type_parameters: &type_parameters,
            field_promotions: false,
        });
    }

    fn async_type(name: &'static str, argument: super::ResolvedType) -> super::ResolvedType {
        super::ResolvedType::Interface {
            occurrence: Default::default(),
            identity: super::DeclarationIdentity::sdk("dart:async", name),
            arguments: vec![argument].into(),
            nullable: false,
            extension_type: false,
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
            let parameter =
                |name: &str, ordinal, bound: ResolvedType| ResolvedType::TypeParameter {
                    id: TypeParameterId {
                        owner: ParameterOwner::Sdk("test"),
                        ordinal,
                        name: name.into(),
                    },
                    bound: bound.into(),
                    nullable: false,
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
    fn null_assert_patterns_promote_on_matched_and_failed_paths() {
        use falcon_syntax::ast::{IfCondition, Stmt};

        for pattern in ["final y!", "1!", "(final y!)"] {
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(&format!(
                "void f(Object? x) {{ if (x case {pattern}) {{}} }}"
            ));
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!("function");
            };
            let FunctionBody::Block(block) = function.body.as_ref().unwrap() else {
                panic!("block");
            };
            let Stmt::If(statement) = &block.stmts[0] else {
                panic!("if");
            };
            let IfCondition::Case(expression, pattern, _) = &statement.condition else {
                panic!("case");
            };
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

        use falcon_syntax::ast::{IfCondition, Stmt};

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
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let path = PathBuf::from("/project/lib/main.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        for declaration in &program.declarations {
            let TopLevelDecl::Function(function) = declaration else {
                continue;
            };
            let mut scope = TypeParameterScope::default();
            scope.push(&function.type_params, &model);
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), &model, &scope);
            let Some(FunctionBody::Block(block)) = &function.body else {
                panic!("expected block");
            };
            let Stmt::If(statement) = &block.stmts[0] else {
                panic!("expected guard");
            };
            let IfCondition::Expr(condition) = &statement.condition else {
                panic!("expected expression condition");
            };
            let context = super::FlowContext {
                model: &model,
                signatures: &signatures,
                type_parameters: &scope,
                field_promotions: false,
            };
            context.assume_condition(&mut environment, condition, function.name.name == "negated");
            let super::Expr::Is { dart_type, .. } = condition else {
                panic!("expected type test");
            };
            let tested = model.resolve_type_in(dart_type, &scope);
            assert_eq!(
                environment.tested_types("value"),
                std::slice::from_ref(&tested)
            );
            if function.name.name == "assignment" {
                context.assign_variable(
                    &mut environment,
                    "value",
                    &model.core_type("String", false),
                );
            }
            let expected = model.resolve_type_in(function.return_type.as_ref().unwrap(), &scope);
            assert_eq!(
                environment.lookup("value"),
                expected,
                "{}",
                function.name.name
            );
            assert_eq!(context.factor(&expected, &expected), ResolvedType::Never);
            assert_eq!(
                context.factor(&ResolvedType::Unknown, &expected),
                ResolvedType::Unknown,
            );
            assert_eq!(context.factor(&expected, &ResolvedType::Unknown), expected);
            let mut unresolved = environment.declared_type("value");
            match &mut unresolved {
                ResolvedType::Interface { arguments, .. } if !arguments.is_empty() => {
                    std::sync::Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                }
                ResolvedType::Function { return_type, .. } => {
                    *std::sync::Arc::make_mut(return_type) = ResolvedType::Unknown;
                }
                ResolvedType::Record { positional, .. } if !positional.is_empty() => {
                    std::sync::Arc::make_mut(positional)[0] = ResolvedType::Unknown;
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

        let parameter = ResolvedType::TypeParameter {
            id: TypeParameterId {
                owner: ParameterOwner::Sdk("test"),
                ordinal: 0,
                name: "T".into(),
            },
            bound: ResolvedType::interface_identity(DeclarationIdentity::OBJECT)
                .with_nullable(true)
                .into(),
            nullable: true,
        };
        let promoted = super::non_null(&parameter).unwrap();
        assert!(matches!(
            promoted,
            ResolvedType::NonNullTypeParameter { .. }
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
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(&format!("f(bool flag) {body}"));
            assert!(errors.is_empty(), "{body}: {errors:?}");
            let Some(TopLevelDecl::Function(function)) = program.declarations.first() else {
                panic!("{body}")
            };
            let Some(FunctionBody::Block(block)) = &function.body else {
                panic!("{body}")
            };
            assert_eq!(
                completes_normally(&block.stmts, &|_| false, &|_| false),
                expected,
                "{body}"
            );
        }
    }
}
