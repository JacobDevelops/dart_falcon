//! Conservative Dart constant evaluation used by semantic lint rules.
//!
//! Canonical type literals instantiate omitted arguments to bounds. Redirected
//! constructors resolve target arguments in the factory owner's lexical scope;
//! raw targets infer them from the instantiated factory return contract.
//! Equality folding rejects user-defined operators; callable tear-offs retain
//! their declaration identity and type, and enum shorthands use declared context.
//! Integer operations use native signed-64-bit wrapping and strings count UTF-16
//! code units; type-test conditions use the same source-proven types as casts.
//! Constructor type tests and casts resolve instantiated owner type parameters.
//! Indexed initializers and constructors retain parsed sources through the signature index.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use falcon_syntax::cst::*;

use crate::resolve::ConstConstructorBody;
use crate::resolve::{expression_segments, resolve_call_constructor};
use crate::{
    DeclarationIdentity, QualifiedName, ResolvedSignature, ResolvedType, SemanticModel,
    SignatureIndex, TypeEnvironment, TypeParameterId, TypeParameterScope,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConstantValue {
    Null,
    Bool(bool),
    /// Preserve integer versus double identity, including integral doubles.
    Number(Number),
    String(String),
    List(Vec<ConstantType>, Vec<ConstantValue>),
    Set(Vec<ConstantType>, Vec<ConstantValue>),
    Map(Vec<ConstantType>, Vec<(ConstantValue, ConstantValue)>),
    Record(Vec<(Option<String>, ConstantValue)>),
    /// A canonical instance: its class, runtime type arguments, and every
    /// field's value keyed by declaring class and field name.
    Instance(
        DeclarationIdentity,
        Vec<ConstantType>,
        Vec<(DeclarationIdentity, String, ConstantValue)>,
    ),
    EnumValue(DeclarationIdentity, String),
    /// A type literal (`int`, `List`), compared after instantiate-to-bounds.
    Type(ConstantType),
    /// A canonical function or constructor tear-off and its callable type.
    TearOff(DeclarationIdentity, String, ConstantType),
}

/// Exact argument types borrowed from values already evaluated for a call.
pub struct ArgumentTypeFallback<'a> {
    args: ArgList<'a>,
    positional: &'a [Option<ConstantValue>],
    named: &'a [(String, Option<ConstantValue>)],
}

impl ArgumentTypeFallback<'_> {
    pub(crate) fn lookup(&self, argument: Expr<'_>) -> Option<ResolvedType> {
        self.args
            .positional()
            .zip(self.positional)
            .chain(
                self.args
                    .named()
                    .map(|argument| argument.value())
                    .zip(self.named.iter().map(|(_, value)| value)),
            )
            .find(|(expression, _)| expression.syntax().id() == argument.syntax().id())
            .and_then(|(_, value)| value.as_ref())
            .and_then(value_type)
            .map(|ty| resolved_constant_type(&ty))
    }
}

/// Resolved type identity retained in explicitly typed constants.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConstantType {
    Dynamic,
    Void,
    Never,
    Null,
    Interface(DeclarationIdentity, Vec<ConstantType>, bool),
    Function(
        Box<ConstantType>,
        Vec<ConstantType>,
        Vec<(String, ConstantType)>,
        usize,
        bool,
        Option<Vec<String>>,
    ),
    Record(Vec<ConstantType>, Vec<(String, ConstantType)>, bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Number {
    Int(i128),
    Float(u64),
}

impl Number {
    fn from_float(value: f64) -> Self {
        Self::Float(value.to_bits())
    }

    fn float(self) -> f64 {
        match self {
            Self::Int(value) => value as f64,
            Self::Float(bits) => f64::from_bits(bits),
        }
    }
}

/// Whether instances use primitive equality rather than a custom `==`.
/// Enums and `Object` qualify; incomplete class hierarchies do not.
pub fn class_has_primitive_equality(
    identity: &DeclarationIdentity,
    signatures: &SignatureIndex,
) -> bool {
    if signatures.is_enum(identity) || *identity == DeclarationIdentity::OBJECT {
        return true;
    }
    let custom = |facts: &crate::MemberFacts| {
        !facts.is_static
            && !facts.is_abstract
            && facts.declaring_type != DeclarationIdentity::OBJECT
    };
    !signatures
        .declared_member_facts(identity, "==")
        .iter()
        .any(custom)
        && signatures
            .inherited_member_facts(identity, "==")
            .is_some_and(|facts| !facts.iter().any(custom))
}

/// Checks every syntactic operand of a constant expression, including operands
/// in unevaluated branches. Stops checking after the first rejected operand.
pub fn constant_operands_all<'a>(
    expression: Expr<'a>,
    allowed: impl FnMut(Expr<'a>) -> bool,
) -> bool {
    use falcon_syntax::cst::visit::{Visitor, walk_expr};

    struct Operands<F> {
        allowed: F,
        accepted: bool,
    }
    impl<'a, F: FnMut(Expr<'a>) -> bool> Visitor<'a> for Operands<F> {
        fn visit_expr(&mut self, expression: Expr<'a>) {
            if self.accepted {
                self.accepted = (self.allowed)(expression);
                if self.accepted {
                    walk_expr(self, expression);
                }
            }
        }
    }
    let mut operands = Operands {
        allowed,
        accepted: true,
    };
    operands.visit_expr(expression);
    operands.accepted
}

/// Immutable options for one constant query. An owner selects its declaring
/// library before resolving members; `library` supplies the fallback scope.
/// Captured unknown locals still shadow globals. Stable queries reject values
/// whose build-dependency provenance is unknown.
#[derive(Clone, Copy)]
pub struct ConstantContext<'a, 'model> {
    pub model: SemanticModel<'model>,
    pub signatures: &'a SignatureIndex,
    pub owner: Option<&'a DeclarationIdentity>,
    pub library: Option<crate::LibraryId>,
    pub fields: Option<&'a HashMap<&'a str, Expr<'a>>>,
    pub locals: Option<&'a LocalConstants<'a>>,
    pub expected: Option<&'a ResolvedType>,
    /// Declaration cache for ordinary queries; stable queries cannot reuse
    /// results whose build-dependency provenance was discarded.
    pub cache: Option<&'a ConstantCache>,
    pub stable: bool,
}

impl<'a, 'model> ConstantContext<'a, 'model> {
    pub fn new(model: &SemanticModel<'model>, signatures: &'a SignatureIndex) -> Self {
        Self {
            model: *model,
            signatures,
            owner: None,
            library: None,
            fields: None,
            locals: None,
            expected: None,
            cache: None,
            stable: false,
        }
    }

    fn evaluator(self, empty_fields: &'a HashMap<&'a str, Expr<'a>>) -> ConstEvaluator<'a, 'model> {
        let model = self
            .library
            .map_or(self.model, |library| self.model.for_library(library));
        let mut evaluator = ConstEvaluator::new(
            self.owner,
            self.fields.unwrap_or(empty_fields),
            &model,
            self.signatures,
        );
        evaluator.local_constants = self.locals;
        evaluator.shared_cache = self.cache.filter(|_| !self.stable);
        evaluator.stable_evaluation = self.stable;
        evaluator
    }
}

/// Evaluates an expression, including its optional declared type context.
pub fn evaluate_constant(
    expression: Expr<'_>,
    context: ConstantContext<'_, '_>,
) -> Option<ConstantValue> {
    context
        .evaluator(&HashMap::new())
        .evaluate_with_context(expression, context.expected)
}

/// Reuses declaration values across constant queries for one analysis file.
#[derive(Default)]
pub struct ConstantCache {
    values: Mutex<SharedConstants>,
}

struct SharedConstants {
    values: HashMap<SharedConstantKey, CachedConstant>,
    retained: HashSet<usize>,
    remaining_bytes: usize,
}

impl Default for SharedConstants {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
            retained: HashSet::new(),
            remaining_bytes: 1024 * 1024,
        }
    }
}

impl SharedConstants {
    fn insert(&mut self, key: SharedConstantKey, cached: CachedConstant) {
        if self.values.contains_key(&key) {
            return;
        }
        if let Some(value) = &cached.value {
            let pointer = Arc::as_ptr(value) as usize;
            if !self.retained.contains(&pointer) {
                let Some(bytes) = value_size(value, self.remaining_bytes) else {
                    return;
                };
                self.remaining_bytes -= bytes;
                self.retained.insert(pointer);
            }
        }
        self.values.insert(key, cached);
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum SharedConstantKey {
    TopLevel(DeclarationIdentity),
    Static(DeclarationIdentity, String),
}

#[derive(Clone)]
struct CachedConstant {
    value: Option<Arc<ConstantValue>>,
    failed: bool,
}

/// Evaluates literal and constant patterns without synthesizing expression nodes.
pub fn evaluate_pattern_constant(
    pattern: Pattern<'_>,
    context: ConstantContext<'_, '_>,
) -> Option<ConstantValue> {
    let fields = HashMap::new();
    let mut evaluator = context.evaluator(&fields);
    match pattern {
        Pattern::Const(pattern) => match pattern.value() {
            ConstPatternValue::Expr(expression) => {
                evaluator.evaluate_with_context(expression, context.expected)
            }
            ConstPatternValue::Name(names) => {
                let names: Vec<_> = names.map(|name| name.name()).collect();
                evaluator.resolve_name(&names)
            }
        },
        Pattern::Literal(pattern) => match pattern.value() {
            LiteralPatternValue::Null => Some(ConstantValue::Null),
            LiteralPatternValue::Bool(value) => Some(ConstantValue::Bool(value)),
            LiteralPatternValue::String(value) => decode_string(value).map(ConstantValue::String),
            LiteralPatternValue::Int(value) => {
                Some(ConstantValue::Number(Number::Int(parse_int(value)?)))
            }
            LiteralPatternValue::NegInt(value) => eval_unary(
                &UnaryOp::Minus,
                ConstantValue::Number(Number::Int(parse_int(value)?)),
            ),
            value @ (LiteralPatternValue::Double(_) | LiteralPatternValue::NegDouble(_)) => {
                let (text, negative) = match value {
                    LiteralPatternValue::Double(text) => (text, false),
                    LiteralPatternValue::NegDouble(text) => (text, true),
                    _ => unreachable!(),
                };
                let value = ConstantValue::Number(Number::from_float(
                    text.replace('_', "").parse::<f64>().ok()?,
                ));
                if negative {
                    eval_unary(&UnaryOp::Minus, value)
                } else {
                    Some(value)
                }
            }
        },
        _ => None,
    }
}

/// Captured lexical constant values and their static expression types.
/// Each entry describes one visible, shadowing binding.
pub type LocalConstants<'a> = HashMap<&'a str, (&'a Option<ConstantValue>, ResolvedType)>;

/// Evaluates a constant name without manufacturing an expression node.
pub fn evaluate_constant_name<'syntax>(
    names: impl Iterator<Item = Identifier<'syntax>>,
    context: ConstantContext<'_, '_>,
) -> Option<ConstantValue> {
    let names: Vec<_> = names.map(|name| name.name()).collect();
    context.evaluator(&HashMap::new()).resolve_name(&names)
}

/// Whether a known constant value provably passes or fails a runtime cast.
/// `None` means its runtime type or the target's subtype relation is incomplete.
pub fn constant_cast_validity(
    value: &ConstantValue,
    target: &ResolvedType,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Option<bool> {
    ConstEvaluator::new(None, &HashMap::new(), model, signatures).cast_accepts(value, target)
}

/// The outcome of running a constant constructor invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construction {
    /// The canonical instance, when every argument and field is known.
    pub value: Option<ConstantValue>,
    /// Whether the invocation provably is not a constant: an assertion fails,
    /// here or in a nested constant.
    pub fails: bool,
}

/// Runs a constant constructor. Arguments use the query's lexical scope;
/// constructor bodies use their own declaring scope.
pub fn evaluate_construction(
    identity: &DeclarationIdentity,
    constructor: &str,
    type_args: &[ResolvedType],
    args: ArgList<'_>,
    context: ConstantContext<'_, '_>,
) -> Construction {
    let fields = HashMap::new();
    let mut evaluator = context.evaluator(&fields);
    let value = evaluator.invoke(identity.clone(), constructor, type_args.to_vec(), args);
    Construction {
        fails: evaluator.failed,
        value: value.filter(|_| !evaluator.failed),
    }
}

/// The explicit type arguments of a constructor call: on the call itself
/// (`Box<int>(1)`) or on the class name of a named constructor
/// (`Box<int>.named(1)`).
pub fn invocation_type_args<'e>(
    callee: Expr<'e>,
    type_args: AstChildren<'e, DartType<'e>>,
) -> AstChildren<'e, DartType<'e>> {
    if !type_args.is_empty() {
        return type_args;
    }
    let class = match callee {
        Expr::Field(field) => field.object(),
        callee => callee,
    };
    if let Expr::GenericInstantiation(instantiation) = class {
        instantiation.type_args()
    } else {
        type_args
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ConstantKey<'a> {
    Field(&'a str),
    TopLevel(DeclarationIdentity),
    Static(DeclarationIdentity, &'a str),
}

// Visit every eager operand even when a missing dependency defers evaluation.
fn collect_eager<T>(values: impl Iterator<Item = Option<T>>) -> Option<Vec<T>> {
    let mut result = Some(Vec::new());
    for value in values {
        if let (Some(values), Some(value)) = (&mut result, value) {
            values.push(value);
        } else {
            result = None;
        }
    }
    result
}

enum ConstantState {
    Evaluating,
    Evaluated(CachedConstant),
}

struct ConstEvaluator<'a, 'model> {
    owner: Option<&'a DeclarationIdentity>,
    fields: &'a HashMap<&'a str, Expr<'a>>,
    local_constants: Option<&'a LocalConstants<'a>>,
    /// Reject environment defaults, conditional library paths, and captured
    /// locals whose dependency provenance was discarded.
    stable_evaluation: bool,
    owner_model: SemanticModel<'model>,
    model: SemanticModel<'model>,
    unqualified_fields: bool,
    /// The type whose static constants and enum values are in unqualified
    /// scope while evaluating one of its static constant initializers.
    static_scope: Option<DeclarationIdentity>,
    signatures: &'a SignatureIndex,
    constants: HashMap<ConstantKey<'a>, ConstantState>,
    shared_cache: Option<&'a ConstantCache>,
    /// Parameters of the constant constructor being run; `None` values are
    /// arguments that could not be evaluated.
    parameters: Vec<HashMap<String, Option<ConstantValue>>>,
    parameter_types: HashMap<String, ResolvedType>,
    constructor_depth: usize,
    dependency_worklist: bool,
    pending_constants: Vec<(ConstantKey<'a>, SemanticModel<'model>)>,
    // Charge expanded values cumulatively, including discarded materialization work.
    remaining_bytes: usize,
    type_parameters: TypeParameterScope,
    /// Set once evaluation proves a constant-evaluation error: a failing
    /// assertion.
    failed: bool,
}

impl<'a, 'model> ConstEvaluator<'a, 'model> {
    fn new(
        owner: Option<&'a DeclarationIdentity>,
        fields: &'a HashMap<&'a str, Expr<'a>>,
        model: &SemanticModel<'model>,
        signatures: &'a SignatureIndex,
    ) -> Self {
        let model = owner
            .and_then(|owner| model.for_declaration(owner))
            .unwrap_or(*model);
        Self {
            owner,
            fields,
            local_constants: None,
            stable_evaluation: false,
            owner_model: model,
            model,
            unqualified_fields: true,
            static_scope: None,
            signatures,
            constants: HashMap::new(),
            shared_cache: None,
            parameters: Vec::new(),
            parameter_types: HashMap::new(),
            constructor_depth: 0,
            dependency_worklist: false,
            pending_constants: Vec::new(),
            remaining_bytes: 1024 * 1024,
            type_parameters: TypeParameterScope::default(),
            failed: false,
        }
    }

    fn evaluate(&mut self, expression: Expr<'_>) -> Option<ConstantValue> {
        self.evaluate_with_context(expression, None)
    }

    fn evaluate_with_context(
        &mut self,
        expression: Expr<'_>,
        expected: Option<&ResolvedType>,
    ) -> Option<ConstantValue> {
        let value = match expression {
            Expr::Ident(identifier) => {
                let value = self.resolve_name_shared(&[identifier.name()])?;
                Self::charge_value(&value, &mut self.remaining_bytes)?;
                return Some(Arc::unwrap_or_clone(value));
            }
            Expr::Paren(paren) => return self.evaluate_with_context(paren.expr(), expected),
            _ => self.evaluate_inner(expression, expected)?,
        };
        Self::charge_value(&value, &mut self.remaining_bytes)?;
        Some(value)
    }

    fn charge_value(value: &ConstantValue, remaining_bytes: &mut usize) -> Option<()> {
        let Some(bytes) = value_size(value, *remaining_bytes) else {
            *remaining_bytes = 0;
            return None;
        };
        *remaining_bytes -= bytes;
        Some(())
    }

    fn evaluate_shared(
        &mut self,
        expression: Expr<'_>,
        expected: Option<&ResolvedType>,
    ) -> Option<Arc<ConstantValue>> {
        if self.remaining_bytes == 0 {
            return None;
        }
        match expression {
            Expr::Ident(identifier) => return self.resolve_name_shared(&[identifier.name()]),
            Expr::Paren(paren) => return self.evaluate_shared(paren.expr(), expected),
            _ => {}
        }
        self.evaluate_with_context(expression, expected)
            .map(Arc::new)
    }

    fn evaluate_inner(
        &mut self,
        expression: Expr<'_>,
        expected: Option<&ResolvedType>,
    ) -> Option<ConstantValue> {
        match expression {
            Expr::Paren(paren) => self.evaluate_with_context(paren.expr(), expected),
            Expr::IntLit(literal) => Some(contextual_number(
                ConstantValue::Number(Number::Int(parse_int(literal.value())?)),
                expression,
                expected.is_some_and(|ty| ty.interface("dart:core", "double")),
            )),
            Expr::DoubleLit(literal) => literal
                .value()
                .replace('_', "")
                .parse::<f64>()
                .ok()
                .map(Number::from_float)
                .map(ConstantValue::Number),
            Expr::BoolLit(literal) => Some(ConstantValue::Bool(literal.value())),
            Expr::NullLit(_) => Some(ConstantValue::Null),
            Expr::StringLit(value) => decode_string(value).map(ConstantValue::String),
            Expr::Ident(identifier) => self.resolve_name(&[identifier.name()]),
            Expr::Field(field_expr) => {
                let object = field_expr.object();
                let field = field_expr.field();
                let is_null_safe = field_expr.is_null_safe();
                if !is_null_safe
                    && field.name() == "length"
                    && let Some(value) = self.evaluate_shared(object, None)
                    && let ConstantValue::String(value) = value.as_ref()
                {
                    return Some(ConstantValue::Number(Number::Int(
                        value.encode_utf16().count() as i128,
                    )));
                }
                self.resolve_name(&expression_segments(expression, true)?)
            }
            Expr::Unary(unary) => {
                let op = unary.op();
                let operand = unary.operand();
                let value = self.evaluate_with_context(operand, expected)?;
                eval_unary(&op, value)
            }
            Expr::Binary(binary) => self.evaluate_binary(binary),
            Expr::Conditional(conditional) => match self.evaluate(conditional.condition())? {
                ConstantValue::Bool(true) => {
                    self.evaluate_with_context(conditional.then_expr(), expected)
                }
                ConstantValue::Bool(false) => {
                    self.evaluate_with_context(conditional.else_expr(), expected)
                }
                _ => None,
            },
            Expr::New(new) if new.is_const() => {
                let (identity, constructor, type_args) =
                    self.new_target(new.dart_type(), new.constructor_name())?;
                self.invoke(identity, constructor, type_args, new.args())
            }
            Expr::New(_) => None,
            Expr::Call(call) => {
                let callee = call.callee();
                let type_args = call.type_args();
                let args = call.args();
                let (identity, constructor) = resolve_call_constructor(callee, &self.model, true)?;
                let type_args = invocation_type_args(callee, type_args)
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .collect();
                self.invoke(identity, constructor, type_args, args)
            }
            Expr::List(list) => {
                let elements = list.elements();
                let arguments = self.collection_arguments(expression, expected)?;
                let values = self.collection(elements, arguments.first())?;
                let types = arguments.iter().map(constant_type).collect::<Option<_>>()?;
                Some(ConstantValue::List(types, values))
            }
            Expr::Set(set) => {
                let elements = set.elements();
                let arguments = self.collection_arguments(expression, expected)?;
                let values = self.collection(elements, arguments.first())?;
                let types = arguments.iter().map(constant_type).collect::<Option<_>>()?;
                Some(ConstantValue::Set(types, values))
            }
            Expr::Map(map) => {
                let elements = map.elements();
                let arguments = self.collection_arguments(expression, expected)?;
                let types = arguments.iter().map(constant_type).collect::<Option<_>>()?;
                let mut values = collect_eager(elements.map(|element| {
                    let MapElement::Entry(entry) = element else {
                        return None;
                    };
                    Some((
                        self.evaluate_with_context(entry.key(), arguments.first())?,
                        self.evaluate_with_context(entry.value(), arguments.get(1))?,
                    ))
                }))?;
                values.sort();
                Some(ConstantValue::Map(types, values))
            }
            Expr::Record(record) => {
                let values = record.fields();
                let mut values = collect_eager(values.map(|field| {
                    Some((
                        field.name().map(|name| name.name().to_owned()),
                        self.evaluate(field.value())?,
                    ))
                }))?;
                let positional = values.iter().take_while(|(name, _)| name.is_none()).count();
                values[positional..].sort();
                Some(ConstantValue::Record(values))
            }
            Expr::NullAssert(assertion) => {
                let value = self.evaluate(assertion.operand())?;
                (value != ConstantValue::Null).then_some(value)
            }
            Expr::Is(test) => {
                let expr = test.expr();
                let dart_type = test.dart_type();
                let negated = test.negated();
                let value = self.evaluate_shared(expr, None)?;
                let target = self.model.resolve_type_in(dart_type, &self.type_parameters);
                Some(ConstantValue::Bool(
                    self.cast_accepts(&value, &target)? != negated,
                ))
            }
            Expr::As(cast) => {
                let expr = cast.expr();
                let dart_type = cast.dart_type();
                let value = self.evaluate(expr)?;
                let target = self.model.resolve_type_in(dart_type, &self.type_parameters);
                (self.cast_accepts(&value, &target) == Some(true)).then_some(value)
            }
            Expr::DotShorthand(shorthand) => {
                let name = shorthand.name();
                let ResolvedType::Interface { identity, .. } = expected? else {
                    return None;
                };
                self.signatures
                    .is_enum_value(identity, name.name())
                    .then(|| ConstantValue::EnumValue(identity.clone(), name.name().to_owned()))
            }
            Expr::This(_)
            | Expr::Super(_)
            | Expr::PostfixIncDec(_)
            | Expr::Assign(_)
            | Expr::Index(_)
            | Expr::Cascade(_)
            | Expr::FuncExpr(_)
            | Expr::Await(_)
            | Expr::Throw(_)
            | Expr::Switch(_)
            | Expr::SymbolLit(_)
            | Expr::GenericInstantiation(_)
            | Expr::Error(_) => None,
        }
    }

    fn evaluate_binary(&mut self, binary: BinaryExpr<'_>) -> Option<ConstantValue> {
        let op = binary.op();
        let left = binary.left();
        let right = binary.right();
        let Some(left) = self.evaluate_shared(left, None) else {
            if !self.pending_constants.is_empty()
                && !matches!(op, BinaryOp::NullCoalesce | BinaryOp::And | BinaryOp::Or)
            {
                let _ = self.evaluate_shared(right, None);
            }
            return None;
        };
        if matches!(op, BinaryOp::NullCoalesce) {
            return if *left == ConstantValue::Null {
                self.evaluate(right)
            } else {
                Some(Arc::unwrap_or_clone(left))
            };
        }
        if matches!(
            (op, left.as_ref()),
            (BinaryOp::And, ConstantValue::Bool(false)) | (BinaryOp::Or, ConstantValue::Bool(true))
        ) {
            return Some(Arc::unwrap_or_clone(left));
        }
        let right = self.evaluate_shared(right, None)?;
        if matches!(op, BinaryOp::Add)
            && let (ConstantValue::String(left), ConstantValue::String(right)) =
                (left.as_ref(), right.as_ref())
        {
            if left.len().saturating_add(right.len()) > self.remaining_bytes {
                self.remaining_bytes = 0;
                return None;
            }
            return Some(ConstantValue::String(format!("{left}{right}")));
        }
        if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq)
            && *left != ConstantValue::Null
            && *right != ConstantValue::Null
        {
            let ConstantValue::Instance(class, ..) = left.as_ref() else {
                return Some(eval_equality(&op, &left, &right));
            };
            if !class_has_primitive_equality(class, self.signatures) {
                return None;
            }
        }
        if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) {
            return Some(eval_equality(&op, &left, &right));
        }
        if !matches!(
            left.as_ref(),
            ConstantValue::Bool(_) | ConstantValue::Number(_)
        ) || !matches!(
            right.as_ref(),
            ConstantValue::Bool(_) | ConstantValue::Number(_)
        ) {
            return None;
        }
        eval_binary(&op, Arc::unwrap_or_clone(left), Arc::unwrap_or_clone(right))
    }

    fn cast_accepts(&self, value: &ConstantValue, target: &ResolvedType) -> Option<bool> {
        if matches!(
            target,
            ResolvedType::Unknown | ResolvedType::TypeParameter { .. }
        ) {
            return None;
        }
        if matches!(target, ResolvedType::Dynamic | ResolvedType::Void) {
            return Some(true);
        }
        // FutureOr can admit null through its argument without being nullable.
        // Its union semantics are not supported by this evaluator.
        if target.interface("dart:async", "FutureOr") {
            return None;
        }
        if matches!(value, ConstantValue::Null) {
            return Some(target.nullable() || matches!(target, ResolvedType::Null));
        }
        if target.interface("dart:core", "Object") {
            return Some(true);
        }
        self.type_cast_accepts(&value_type(value)?, target)
    }

    fn type_cast_accepts(&self, runtime: &ConstantType, target: &ResolvedType) -> Option<bool> {
        if matches!(target, ResolvedType::Dynamic | ResolvedType::Void) {
            return Some(true);
        }
        // Check the unsupported union before Null, including generic arguments.
        if target.interface("dart:async", "FutureOr") {
            return None;
        }
        if matches!(runtime, ConstantType::Null) {
            return Some(target.nullable());
        }
        let ResolvedType::Interface {
            identity,
            arguments: target_arguments,
            ..
        } = target
        else {
            return (constant_type(target).as_ref() == Some(runtime)).then_some(true);
        };
        if identity.is_sdk("dart:core", "Object") {
            return Some(true);
        }
        if let Some(ConstantType::Interface(target_class, args, _)) = constant_type(target)
            && matches!(runtime, ConstantType::Interface(class, arguments, _)
                if *class == target_class && *arguments == args)
        {
            return Some(true);
        }
        let ConstantType::Interface(class, arguments, nullable) = runtime else {
            // A function's interface supertypes come from core Function, while
            // its exact signature remains available for function-type casts.
            return if matches!(runtime, ConstantType::Function(..)) {
                self.type_cast_accepts(
                    &constant_type(&self.model.core_type("Function", false))?,
                    target,
                )
            } else {
                None
            };
        };
        if *nullable && !target.nullable() {
            return Some(false);
        }
        if class == identity && arguments.len() == target_arguments.len() {
            let mut unknown = false;
            for (argument, target) in arguments.iter().zip(target_arguments.iter()) {
                match self.type_cast_accepts(argument, target) {
                    Some(false) => return Some(false),
                    None => unknown = true,
                    Some(true) => {}
                }
            }
            return (!unknown).then_some(true);
        }
        let source = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: class.clone(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        };
        if target_arguments.is_empty()
            && arguments.is_empty()
            && self
                .signatures
                .as_instance_of(&source, identity, &self.model)
                .is_some()
        {
            return Some(true);
        }
        if let DeclarationIdentity::Sdk { library, name } = identity {
            return match self
                .signatures
                .implements_sdk(&source, library.as_str(), name.as_str())
            {
                crate::Truth::No => Some(false),
                crate::Truth::Yes if target_arguments.is_empty() => Some(true),
                _ => None,
            };
        }
        None
    }

    fn resolve_name(&mut self, names: &[&str]) -> Option<ConstantValue> {
        self.resolve_name_shared(names).map(Arc::unwrap_or_clone)
    }

    fn resolve_name_shared(&mut self, names: &[&str]) -> Option<Arc<ConstantValue>> {
        let local_name = names.last()?;
        if names.len() == 1
            && let Some(value) = self
                .parameters
                .last()
                .and_then(|parameters| parameters.get(*local_name))
        {
            let value = value.as_ref()?;
            Self::charge_value(value, &mut self.remaining_bytes)?;
            return Some(Arc::new(value.clone()));
        }
        if names.len() == 1
            && self.parameters.is_empty()
            && let Some((value, _)) = self
                .local_constants
                .and_then(|locals| locals.get(*local_name))
        {
            return if self.stable_evaluation {
                None
            } else {
                let value = (*value).as_ref()?;
                Self::charge_value(value, &mut self.remaining_bytes)?;
                Some(Arc::new(value.clone()))
            };
        }
        let local_owner = if names.len() == 1 {
            self.unqualified_fields
        } else {
            self.owner.is_some_and(|owner| {
                self.model
                    .resolve_dotted(&names[..names.len() - 1])
                    .as_ref()
                    == Some(owner)
            })
        };
        if local_owner && let Some((&name, _)) = self.fields.get_key_value(*local_name) {
            return self.initializer(ConstantKey::Field(name));
        }
        if names.len() == 1 {
            let scope = if self.unqualified_fields {
                self.owner.cloned()
            } else {
                self.static_scope.clone()
            };
            if self.stable_evaluation
                && let Some(owner) = scope.as_ref()
            {
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: owner.clone(),
                    arguments: Default::default(),
                    nullable: false,
                    extension_type: false,
                };
                if self
                    .signatures
                    .resolved_readable_member_owner(&receiver, local_name, &self.model)
                    .is_some()
                {
                    return None;
                }
            }
            if let Some(scope) = scope
                && let Some(value) = self.static_member(scope, local_name, false)
            {
                return value;
            }
        } else if let Some(owner) = self.model.resolve_dotted(&names[..names.len() - 1]) {
            if self.stable_evaluation && self.model.configurable_constant_path(&owner) {
                return None;
            }
            // A private static member is invisible outside its library.
            if !self.model.member_visible(&owner, local_name) {
                return None;
            }
            if let Some(value) = self.static_member(owner, local_name, true) {
                return value;
            }
        }
        let Some(identity) =
            QualifiedName::from_segments(names).and_then(|name| self.model.resolve_value(name))
        else {
            // A type literal names a declared type rather than a value.
            let ty = self.model.resolve_type_name(names)?;
            if self.stable_evaluation
                && let ResolvedType::Interface { identity, .. } = &ty
                && self.model.configurable_constant_path(identity)
            {
                return None;
            }
            return constant_type(&self.signatures.instantiate_to_bounds(&ty))
                .map(ConstantValue::Type)
                .map(Arc::new);
        };
        if self.stable_evaluation && self.model.configurable_constant_path(&identity) {
            return None;
        }
        if let Some(signature) = self.signatures.function(&identity) {
            return Some(Arc::new(ConstantValue::TearOff(
                identity.clone(),
                String::new(),
                constant_type(&signature.function_type())?,
            )));
        }
        self.initializer(ConstantKey::TopLevel(identity))
    }

    /// The value of `owner`'s enum value or static constant `name`; `None`
    /// when `owner` declares neither, `Some(None)` when it is not evaluable.
    fn static_member(
        &mut self,
        owner: DeclarationIdentity,
        name: &str,
        include_constructors: bool,
    ) -> Option<Option<Arc<ConstantValue>>> {
        if self.signatures.is_enum_value(&owner, name) {
            return Some(Some(Arc::new(ConstantValue::EnumValue(
                owner.clone(),
                name.to_owned(),
            ))));
        }
        if include_constructors
            && let Some(ty) = self
                .signatures
                .constructor_tearoff_type(&owner, name, &self.model)
        {
            return Some(constant_type(&ty).and_then(|ty| {
                let ConstantType::Function(result, ..) = &ty else {
                    return None;
                };
                let ConstantType::Interface(identity, ..) = result.as_ref() else {
                    return None;
                };
                Some(Arc::new(ConstantValue::TearOff(
                    identity.clone(),
                    name.to_owned(),
                    ty,
                )))
            }));
        }
        if self
            .signatures
            .declared_member_facts(&owner, name)
            .iter()
            .any(|fact| fact.is_static && fact.kind == crate::MemberKind::Method)
        {
            return Some(self.signatures.member(&owner, name).and_then(|signature| {
                constant_type(&signature.function_type())
                    .map(|ty| Arc::new(ConstantValue::TearOff(owner.clone(), name.to_owned(), ty)))
            }));
        }
        let signatures = self.signatures;
        let constant = signatures
            .declaration(&owner)?
            .static_consts
            .iter()
            .find(|constant| constant.name == name)?;
        Some(self.initializer(ConstantKey::Static(owner, &constant.name)))
    }

    fn initializer(&mut self, key: ConstantKey<'a>) -> Option<Arc<ConstantValue>> {
        if let Some(state) = self.constants.get(&key) {
            return match state {
                ConstantState::Evaluating => None,
                ConstantState::Evaluated(cached) => {
                    self.failed |= cached.failed;
                    cached.value.clone()
                }
            };
        }
        if let Some(cached) = self.cached_initializer(&key) {
            self.failed |= cached.failed;
            self.constants
                .insert(key, ConstantState::Evaluated(cached.clone()));
            return cached.value;
        }
        if self.dependency_worklist {
            self.pending_constants.push((key, self.model));
            return None;
        }
        self.dependency_worklist = true;
        let depth = self.constructor_depth;
        let mut worklist = vec![(key.clone(), self.model)];
        while let Some((current, model)) = worklist.last().cloned() {
            if matches!(
                self.constants.get(&current),
                Some(ConstantState::Evaluated(_))
            ) {
                worklist.pop();
                continue;
            }
            self.constructor_depth = 0;
            self.constants
                .insert(current.clone(), ConstantState::Evaluating);
            let previous_failure = std::mem::take(&mut self.failed);
            let value = self.evaluate_initializer(&current, model);
            let failed = self.failed;
            self.failed |= previous_failure;
            if self.pending_constants.is_empty() {
                let cached = CachedConstant { value, failed };
                if let Some(cache) = self.shared_cache
                    && self.remaining_bytes != 0
                    && let Some(key) = Self::shared_key(&current)
                {
                    cache
                        .values
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(key, cached.clone());
                }
                self.constants
                    .insert(current, ConstantState::Evaluated(cached));
                worklist.pop();
            } else {
                worklist.append(&mut self.pending_constants);
            }
        }
        self.constructor_depth = depth;
        self.dependency_worklist = false;
        match self.constants.get(&key)? {
            ConstantState::Evaluated(cached) => {
                self.failed |= cached.failed;
                cached.value.clone()
            }
            ConstantState::Evaluating => None,
        }
    }

    fn shared_key(key: &ConstantKey<'_>) -> Option<SharedConstantKey> {
        match key {
            ConstantKey::Field(_) => None,
            ConstantKey::TopLevel(owner) => Some(SharedConstantKey::TopLevel(owner.clone())),
            ConstantKey::Static(owner, name) => {
                Some(SharedConstantKey::Static(owner.clone(), (*name).to_owned()))
            }
        }
    }

    fn cached_initializer(&self, key: &ConstantKey<'_>) -> Option<CachedConstant> {
        let cache = self.shared_cache?;
        let key = Self::shared_key(key)?;
        cache
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values
            .get(&key)
            .cloned()
    }

    fn evaluate_initializer(
        &mut self,
        key: &ConstantKey<'a>,
        model: SemanticModel<'model>,
    ) -> Option<Arc<ConstantValue>> {
        let (expression, model, expected) = match key {
            ConstantKey::Field(name) => (
                *self.fields.get(name)?,
                self.owner_model,
                self.owner
                    .and_then(|owner| self.signatures.field(owner, name).map(|(_, ty)| ty)),
            ),
            ConstantKey::TopLevel(identity) => (
                self.signatures.constant_initializer(identity)?,
                model.for_declaration(identity)?,
                self.signatures.value_type(identity),
            ),
            ConstantKey::Static(owner, name) => {
                let constant = self
                    .signatures
                    .declaration(owner)?
                    .static_consts
                    .iter()
                    .find(|constant| constant.name == *name)?;
                (
                    constant.initializer.expr(),
                    model.for_declaration(owner)?,
                    Some(&constant.ty),
                )
            }
        };
        let previous_model = std::mem::replace(&mut self.model, model);
        let previous_fields = std::mem::replace(
            &mut self.unqualified_fields,
            matches!(key, ConstantKey::Field(_)),
        );
        let previous_scope = std::mem::replace(
            &mut self.static_scope,
            match &key {
                ConstantKey::Static(owner, _) => Some(owner.clone()),
                ConstantKey::Field(_) | ConstantKey::TopLevel(_) => None,
            },
        );
        // A constant's initializer does not see the parameters of a
        // constructor being run.
        self.parameters.push(HashMap::new());
        let previous_parameter_types = std::mem::take(&mut self.parameter_types);
        let previous_type_parameters = std::mem::take(&mut self.type_parameters);
        let value = self.evaluate_shared(expression, expected);
        self.parameters.pop();
        self.parameter_types = previous_parameter_types;
        self.type_parameters = previous_type_parameters;
        self.model = previous_model;
        self.unqualified_fields = previous_fields;
        self.static_scope = previous_scope;
        value
    }

    fn collection(
        &mut self,
        elements: AstChildren<'_, CollectionElement<'_>>,
        expected: Option<&ResolvedType>,
    ) -> Option<Vec<ConstantValue>> {
        collect_eager(elements.map(|element| match element {
            CollectionElement::Expr(expression) => self.evaluate_with_context(expression, expected),
            _ => None,
        }))
    }

    fn collection_arguments(
        &mut self,
        expression: Expr<'_>,
        expected: Option<&ResolvedType>,
    ) -> Option<std::sync::Arc<[ResolvedType]>> {
        let single = match expression {
            Expr::List(list) => list.type_arg(),
            Expr::Set(set) => set.type_arg(),
            Expr::Map(map) => {
                let arguments = map.type_args();
                if !arguments.is_empty() {
                    return Some(
                        arguments
                            .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                            .collect(),
                    );
                }
                None
            }
            _ => return None,
        };
        if let Some(ty) = single {
            return Some([self.model.resolve_type_in(ty, &self.type_parameters)].into());
        }
        let ty = self.infer_expression(expression, expected);
        match &ty {
            ResolvedType::Interface { .. } => Some(ty.type_arguments().collect()),
            _ => None,
        }
    }

    /// Inference must use exactly the evaluator's lexical lookup, including
    /// unknown shadowing bindings and the active constructor's parameters.
    fn infer_expression(
        &mut self,
        expression: Expr<'_>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        let model = self.model;
        let signatures = self.signatures;
        let evaluator = std::cell::RefCell::new(self);
        TypeEnvironment::new().infer_with_context(
            expression,
            &model,
            signatures,
            &TypeParameterScope::default(),
            expected,
            &|name| evaluator.borrow_mut().inference_name(name),
        )
    }

    fn inference_name(&mut self, name: &str) -> Option<ResolvedType> {
        if let Some(ty) = self.parameter_types.get(name) {
            return Some(ty.clone());
        }
        if self.parameters.is_empty()
            && let Some((_, ty)) = self.local_constants.and_then(|locals| locals.get(name))
        {
            return Some(ty.clone());
        }
        let parameter = self
            .parameters
            .last()
            .is_some_and(|scope| scope.contains_key(name));
        if !parameter {
            let scope = if self.unqualified_fields {
                self.owner
            } else {
                self.static_scope.as_ref()
            };
            if let Some(constant) = scope
                .and_then(|owner| self.signatures.declaration(owner))
                .and_then(|declaration| {
                    declaration
                        .static_consts
                        .iter()
                        .find(|constant| constant.name == name)
                })
                && constant.ty != ResolvedType::Unknown
            {
                return Some(constant.ty.clone());
            }
            if let Some(ty) = scope
                .and_then(|owner| self.signatures.field(owner, name))
                .map(|(_, ty)| ty)
                && *ty != ResolvedType::Unknown
            {
                return Some(ty.clone());
            }
            if let Some(owner) = scope
                && self
                    .signatures
                    .declared_member_facts(owner, name)
                    .iter()
                    .any(|fact| fact.is_static && fact.kind == crate::MemberKind::Method)
            {
                return Some(
                    self.signatures
                        .member(owner, name)
                        .map_or(ResolvedType::Unknown, |signature| signature.function_type()),
                );
            }
            if let Some(identity) = self.model.resolve_value(name)
                && let Some(ty) = self.signatures.value_type(&identity)
                && *ty != ResolvedType::Unknown
            {
                return Some(ty.clone());
            }
        }
        let shadowed = self
            .parameters
            .last()
            .is_some_and(|scope| scope.contains_key(name))
            || (self.parameters.is_empty()
                && self
                    .local_constants
                    .is_some_and(|locals| locals.contains_key(name)));
        let value = self
            .resolve_name(&[name])
            .and_then(|value| value_type(&value))
            .map(|ty| resolved_constant_type(&ty));
        value.or_else(|| shadowed.then_some(ResolvedType::Unknown))
    }

    /// The target of `new`/`const` `dart_type[.constructor]`: the parser may
    /// fold a named constructor into the type's segments.
    fn new_target(
        &self,
        dart_type: DartType<'_>,
        constructor_name: Option<Identifier<'_>>,
    ) -> Option<(DeclarationIdentity, String, Vec<ResolvedType>)> {
        let DartType::Named(named) = dart_type else {
            return None;
        };
        let segments: Vec<&str> = named.segments().map(|segment| segment.name()).collect();
        let (identity, constructor) = match constructor_name {
            Some(name) => (self.model.resolve_dotted(&segments)?, name.name()),
            None => match self.model.resolve_dotted(&segments) {
                Some(identity) => (identity, "new"),
                None => {
                    let (constructor, class) = segments.split_last()?;
                    (self.model.resolve_dotted(class)?, *constructor)
                }
            },
        };
        let type_args = named
            .type_args()
            .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
            .collect();
        Some((identity, constructor.to_string(), type_args))
    }

    /// Evaluates `args` here, then runs the constructor.
    fn invoke(
        &mut self,
        identity: DeclarationIdentity,
        constructor: impl AsRef<str>,
        type_args: Vec<ResolvedType>,
        args: ArgList<'_>,
    ) -> Option<ConstantValue> {
        if self.stable_evaluation && self.model.configurable_constant_path(&identity) {
            return None;
        }
        if let Some(value) = self.environment_constant(&identity, constructor.as_ref(), args) {
            return value;
        }
        let signatures = self.signatures;
        let mut signature = signatures
            .constructor(&identity, constructor.as_ref())
            .cloned();
        let mut type_args = type_args;
        let inference = signature.as_ref().and_then(|signature| {
            (type_args.is_empty() && !signature.owner_parameters.is_empty()).then(|| {
                let mut inference = signature.clone();
                inference.call_parameters = inference.owner_parameters.clone();
                inference
            })
        });
        if let Some(signature) = &mut signature {
            let substitutions = signature
                .owner_parameters
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    (
                        id.clone(),
                        type_args
                            .get(index)
                            .cloned()
                            .unwrap_or(ResolvedType::Unknown),
                    )
                })
                .collect();
            for ty in signature
                .positional
                .iter_mut()
                .chain(signature.named.values_mut())
            {
                *ty = self.model.substitute(ty, &substitutions);
            }
        }
        // Capture argument values once. Inference may request their exact
        // types, but must not recursively execute inline constructors again.
        let (mut positional, mut named) = self.arguments(args, signature.as_ref(), None);
        if let Some(inference) = inference {
            let model = self.model;
            let scope = TypeParameterScope::default();
            let evaluator = std::cell::RefCell::new(&mut *self);
            let lookup = |name: &str| evaluator.borrow_mut().inference_name(name);
            let argument_fallback = ArgumentTypeFallback {
                args,
                positional: &positional,
                named: &named,
            };
            let context = crate::InferenceContext {
                model: &model,
                signatures,
                type_parameters: &scope,
                lookup: &lookup,
                argument_fallback: Some(&argument_fallback),
            };
            let inferred = TypeEnvironment::new().infer_signature_substitutions(
                &inference,
                args,
                AstChildren::empty(),
                &context,
                None,
                &HashMap::new(),
            );
            type_args = inference
                .owner_parameters
                .iter()
                .map(|id| inferred.get(id).cloned().unwrap_or(ResolvedType::Unknown))
                .collect();
            // Applying a newly inferred double context to an integer literal
            // changes only its numeric identity, not its evaluated expression.
            for (argument, value, parameter) in args
                .positional()
                .zip(&mut positional)
                .zip(&inference.positional)
                .map(|((argument, value), parameter)| (argument, value, parameter))
                .chain(
                    args.named()
                        .zip(&mut named)
                        .filter_map(|(argument, (_, value))| {
                            inference
                                .named
                                .get(argument.name().name())
                                .map(|parameter| (argument.value(), value, parameter))
                        }),
                )
            {
                if self
                    .model
                    .substitute(parameter, &inferred)
                    .interface("dart:core", "double")
                    && let Some(evaluated) = value.take()
                {
                    *value = Some(contextual_number(evaluated, argument, true));
                }
            }
        }
        self.construct(identity, constructor.as_ref(), type_args, positional, named)
    }

    /// `bool`/`int`/`String.fromEnvironment` and `bool.hasEnvironment`.
    /// Stable evaluation leaves them unknown; ordinary evaluation folds as the
    /// CLI analyzer does with an empty declared-variable map. Web bool declarations
    /// remain unknown. `None` means this is not an environment constructor.
    fn environment_constant(
        &mut self,
        identity: &DeclarationIdentity,
        constructor: &str,
        args: ArgList<'_>,
    ) -> Option<Option<ConstantValue>> {
        let is_core = |name| identity.is_sdk("dart:core", name);
        if self.stable_evaluation
            && ((constructor == "fromEnvironment"
                && (is_core("bool") || is_core("int") || is_core("String")))
                || (constructor == "hasEnvironment" && is_core("bool")))
        {
            return Some(None);
        }
        let fallback = match constructor {
            "hasEnvironment" if is_core("bool") => return Some(Some(ConstantValue::Bool(false))),
            "fromEnvironment" if is_core("bool") => ConstantValue::Bool(false),
            "fromEnvironment" if is_core("int") => ConstantValue::Number(Number::Int(0)),
            "fromEnvironment" if is_core("String") => ConstantValue::String(String::new()),
            _ => return None,
        };
        let Some(Some(ConstantValue::String(name))) =
            args.positional().first().map(|name| self.evaluate(name))
        else {
            return Some(None);
        };
        // Analyzer's evaluateFactoryConstructorCall deliberately uses
        // BoolState.UNKNOWN_VALUE for these two declarations (SDK issue 50045),
        // regardless of defaultValue. This is not an SDK-availability table.
        if is_core("bool")
            && matches!(
                name.as_str(),
                "dart.library.js_util" | "dart.library.js_interop"
            )
        {
            return Some(None);
        }
        Some(
            match args
                .named()
                .find(|argument| argument.name().name() == "defaultValue")
            {
                Some(argument) => self.evaluate(argument.value()),
                None => Some(fallback),
            },
        )
    }

    #[allow(clippy::type_complexity)]
    fn arguments(
        &mut self,
        args: ArgList<'_>,
        signature: Option<&ResolvedSignature>,
        substitutions: Option<&HashMap<TypeParameterId, ResolvedType>>,
    ) -> (
        Vec<Option<ConstantValue>>,
        Vec<(String, Option<ConstantValue>)>,
    ) {
        let positional = args
            .positional()
            .enumerate()
            .map(|(index, argument)| match substitutions {
                Some(substitutions) => {
                    let expected = signature
                        .and_then(|signature| signature.positional.get(index))
                        .map(|ty| self.model.substitute(ty, substitutions));
                    self.evaluate_with_context(argument, expected.as_ref())
                }
                None => self.evaluate_with_context(
                    argument,
                    signature.and_then(|signature| signature.positional.get(index)),
                ),
            })
            .collect();
        let named = args
            .named()
            .map(|argument| {
                let expected =
                    signature.and_then(|signature| signature.named.get(argument.name().name()));
                let value = match substitutions {
                    Some(substitutions) => {
                        let expected = expected.map(|ty| self.model.substitute(ty, substitutions));
                        self.evaluate_with_context(argument.value(), expected.as_ref())
                    }
                    None => self.evaluate_with_context(argument.value(), expected),
                };
                (argument.name().name().to_owned(), value)
            })
            .collect();
        (positional, named)
    }

    /// Runs a `const` constructor of a project class on evaluated arguments
    /// (`None` for unknown ones), yielding the canonical instance: its runtime
    /// type and every field's value.
    fn construct(
        &mut self,
        identity: DeclarationIdentity,
        constructor: &str,
        type_args: Vec<ResolvedType>,
        positional: Vec<Option<ConstantValue>>,
        named: Vec<(String, Option<ConstantValue>)>,
    ) -> Option<ConstantValue> {
        const MAX_DEPTH: usize = 32;
        let signatures = self.signatures;
        if self.constructor_depth >= MAX_DEPTH
            || !signatures.is_const_constructor(&identity, constructor)
        {
            return None;
        }
        let body = signatures.const_constructor(&identity, constructor)?;
        let model = self.model.for_declaration(&identity)?;
        self.constructor_depth += 1;
        let previous_model = std::mem::replace(&mut self.model, model);
        let previous_fields = std::mem::replace(&mut self.unqualified_fields, false);
        let previous_scope = self.static_scope.replace(identity.clone());
        // Defaults see no caller's parameters; field initializers only see
        // this constructor's parameters when it is primary.
        self.parameters.push(HashMap::new());
        let previous_parameter_types = std::mem::take(&mut self.parameter_types);
        let previous_type_parameters = std::mem::take(&mut self.type_parameters);
        self.type_parameters.push_bindings(
            signatures
                .type_parameters(&identity)
                .unwrap_or(&[])
                .iter()
                .zip(&type_args)
                .map(|(parameter, ty)| (parameter.name.clone(), ty.clone())),
        );
        let value =
            self.run_constructor(&identity, constructor, body, type_args, positional, named);
        self.parameters.pop();
        self.constructor_depth -= 1;
        self.parameter_types = previous_parameter_types;
        self.type_parameters = previous_type_parameters;
        self.model = previous_model;
        self.unqualified_fields = previous_fields;
        self.static_scope = previous_scope;
        value
    }

    fn run_constructor(
        &mut self,
        identity: &DeclarationIdentity,
        constructor: &str,
        body: &ConstConstructorBody,
        type_args: Vec<ResolvedType>,
        positional: Vec<Option<ConstantValue>>,
        named: Vec<(String, Option<ConstantValue>)>,
    ) -> Option<ConstantValue> {
        let mut substitutions = self
            .signatures
            .type_parameters(identity)
            .unwrap_or(&[])
            .iter()
            .cloned()
            .zip(type_args.iter().cloned())
            .collect();
        let signature = self.signatures.constructor(identity, constructor)?;
        let superclass = self
            .signatures
            .superclass(identity)
            .map(|ty| self.model.substitute(ty, &substitutions));
        let mut environment = HashMap::with_capacity(body.params.len());
        let mut parameter_types = HashMap::with_capacity(body.params.len());
        let mut fields = Vec::new();
        let mut super_positional = Vec::new();
        let mut super_named = Vec::new();
        let mut remaining = positional.iter();
        let mut positional_types = signature.positional.iter();
        for param in &body.params {
            let param: FormalParam<'_> = param.cast();
            let ty = match param.kind() {
                ParamKind::Positional | ParamKind::OptionalPositional => positional_types.next(),
                ParamKind::Named { .. } => signature.named.get(param.name().name()),
            }
            .map(|ty| self.model.substitute(ty, &substitutions));
            if let Some(ty) = &ty {
                parameter_types.insert(param.name().name().to_owned(), ty.clone());
            }
            let supplied = match param.kind() {
                ParamKind::Positional | ParamKind::OptionalPositional => remaining.next().cloned(),
                ParamKind::Named { .. } => named
                    .iter()
                    .find(|(name, _)| *name == param.name().name())
                    .map(|(_, value)| value.clone()),
            };
            let value = match supplied {
                Some(value) => value,
                // A missing required argument is a compile error the
                // analyzer's constant evaluation does not report; its value
                // is unknown.
                None if matches!(
                    param.kind(),
                    ParamKind::Positional | ParamKind::Named { required: true }
                ) =>
                {
                    None
                }
                None => param
                    .default_value()
                    .map_or(Some(ConstantValue::Null), |default| {
                        // Defaults are evaluated before the constructor's
                        // parameter bindings become visible.
                        self.evaluate_with_context(default, ty.as_ref())
                    }),
            };
            let initializing = param.initializing().or_else(|| {
                (body.is_primary && (body.representation || param.is_var() || param.is_final()))
                    .then_some(InitializingFormal::This)
            });
            match initializing {
                Some(InitializingFormal::This) => {
                    fields.push((param.name().name().to_owned(), value.clone()))
                }
                Some(InitializingFormal::Super) => match param.kind() {
                    ParamKind::Named { .. } => {
                        super_named.push((param.name().name().to_owned(), value.clone()));
                    }
                    ParamKind::Positional | ParamKind::OptionalPositional => {
                        super_positional.push(value.clone());
                    }
                },
                None => {}
            }
            environment.insert(param.name().name().to_owned(), value);
        }
        if let Some(redirect) = &body.redirect {
            let redirect: RedirectedConstructor<'_> = redirect.cast();
            let (target, target_constructor, _) =
                self.new_target(redirect.type_(), redirect.constructor_name())?;
            // Conservative expression inference may miss the source type even
            // though its evaluated arguments determine it (for example NaN
            // without SDK source). Recover that source contract before
            // substituting the redirect, not from the target's parameters.
            let mut type_args = type_args;
            if type_args.iter().any(|ty| constant_type(ty).is_none())
                && self
                    .signatures
                    .constructor(&target, &target_constructor)
                    .is_some_and(|target| !target.owner_parameters.is_empty())
            {
                let (_, inferred) =
                    self.runtime_type(identity, constructor, body, &type_args, &environment)?;
                type_args = inferred.iter().map(resolved_constant_type).collect();
                substitutions.extend(
                    signature
                        .owner_parameters
                        .iter()
                        .cloned()
                        .zip(type_args.iter().cloned()),
                );
            }
            let mut scope = TypeParameterScope::default();
            scope.push_bindings(
                signature
                    .owner_parameters
                    .iter()
                    .zip(&type_args)
                    .map(|(parameter, ty)| (parameter.name.clone(), ty.clone())),
            );
            let DartType::Named(named_type) = redirect.type_() else {
                return None;
            };
            let mut target_arguments: Vec<_> = named_type
                .type_args()
                .map(|ty| self.model.resolve_type_in(ty, &scope))
                .collect();
            if target_arguments.is_empty()
                && let Some(target_signature) =
                    self.signatures.constructor(&target, &target_constructor)
                && !target_signature.owner_parameters.is_empty()
            {
                let mut inference = target_signature.clone();
                inference.call_parameters = inference.owner_parameters.clone();
                inference.return_type = self.signatures.as_instance_of(
                    &self.signatures.self_instantiation(
                        target.clone(),
                        &inference.owner_parameters,
                        &self.model,
                    ),
                    identity,
                    &self.model,
                )?;
                let expected = self.model.substitute(
                    &self.signatures.self_instantiation(
                        identity.clone(),
                        &signature.owner_parameters,
                        &self.model,
                    ),
                    &substitutions,
                );
                let context = crate::InferenceContext {
                    model: &self.model,
                    signatures: self.signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let inferred = TypeEnvironment::new().infer_signature_substitutions(
                    &inference,
                    None,
                    AstChildren::empty(),
                    &context,
                    Some(&expected),
                    &HashMap::new(),
                );
                let defaults = std::cell::OnceCell::new();
                target_arguments = inference
                    .owner_parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| {
                        inferred
                            .get(parameter)
                            .filter(|ty| constant_type(ty).is_some())
                            .cloned()
                            .or_else(|| {
                                defaults
                                    .get_or_init(|| {
                                        self.signatures.instantiate_to_bounds(
                                            &self.model.resolve_type_in(redirect.type_(), &scope),
                                        )
                                    })
                                    .type_argument(index)
                            })
                    })
                    .collect::<Option<Vec<_>>>()?;
            }
            return self.construct(
                target,
                &target_constructor,
                target_arguments,
                positional,
                named,
            );
        }
        if body.is_primary
            && let Some(scope) = self.parameters.last_mut()
        {
            std::mem::swap(scope, &mut environment);
            std::mem::swap(&mut self.parameter_types, &mut parameter_types);
        }
        let signatures = self.signatures;
        for (name, initializer) in signatures.const_field_initializers(identity) {
            let expected = signatures
                .field(identity, name)
                .map(|(_, ty)| self.model.substitute(ty, &substitutions));
            fields.push((
                name.to_owned(),
                self.evaluate_with_context(initializer, expected.as_ref()),
            ));
        }
        if !body.is_primary
            && let Some(scope) = self.parameters.last_mut()
        {
            std::mem::swap(scope, &mut environment);
            std::mem::swap(&mut self.parameter_types, &mut parameter_types);
        }
        let mut super_constructor = "new";
        for initializer in &body.initializers {
            match initializer.cast::<ConstructorInitializer<'_>>() {
                ConstructorInitializer::FieldInit(initializer) => {
                    let field = initializer.field();
                    let value = initializer.value();
                    let signatures = self.signatures;
                    let expected = signatures
                        .field(identity, field.name())
                        .map(|(_, ty)| self.model.substitute(ty, &substitutions));
                    let value = self.evaluate_with_context(value, expected.as_ref());
                    fields.push((field.name().to_owned(), value));
                }
                ConstructorInitializer::Assert(initializer) => {
                    if self.evaluate(initializer.condition()) == Some(ConstantValue::Bool(false)) {
                        self.failed = true;
                    }
                }
                ConstructorInitializer::ThisCall(initializer) => {
                    let target = initializer.call_name().map_or("new", |name| name.name());
                    let args = initializer.args();
                    let signatures = self.signatures;
                    let mut signature = signatures.constructor(identity, target).cloned();
                    if let Some(signature) = &mut signature {
                        for ty in signature
                            .positional
                            .iter_mut()
                            .chain(signature.named.values_mut())
                        {
                            *ty = self.model.substitute(ty, &substitutions);
                        }
                    }
                    let (positional, named) = self.arguments(args, signature.as_ref(), None);
                    return self.construct(identity.clone(), target, type_args, positional, named);
                }
                ConstructorInitializer::SuperCall(initializer) => {
                    super_constructor = initializer.call_name().map_or("new", |name| name.name());
                    let args = initializer.args();
                    let signatures = self.signatures;
                    let signature = superclass
                        .as_ref()
                        .and_then(|ty| signatures.resolved_constructor(ty, super_constructor));
                    let (positional, named) = match signature {
                        Some((signature, substitutions)) => {
                            self.arguments(args, Some(signature), Some(&substitutions))
                        }
                        None => self.arguments(args, None, None),
                    };
                    super_positional.extend(positional);
                    super_named.extend(named);
                }
            }
        }
        let environment = self
            .parameters
            .last_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        if self.failed {
            return None;
        }
        if self
            .signatures
            .declaration(identity)
            .is_some_and(|facts| facts.extension_representation.is_some())
        {
            return (fields.len() == 1)
                .then(|| fields.into_iter().next().and_then(|(_, value)| value))
                .flatten();
        }
        let mut known = true;
        let mut inherited = Vec::new();
        match superclass.as_ref() {
            None => {}
            Some(ResolvedType::Interface {
                identity: superclass,
                ..
            }) if *superclass == DeclarationIdentity::OBJECT => {}
            Some(
                ty @ ResolvedType::Interface {
                    identity: superclass,
                    ..
                },
            ) => {
                match self.construct(
                    superclass.clone(),
                    super_constructor,
                    ty.type_arguments().collect(),
                    super_positional,
                    super_named,
                ) {
                    Some(ConstantValue::Instance(_, _, values)) => inherited = values,
                    _ => known = false,
                }
            }
            Some(_) => known = false,
        }
        let runtime_type = self.runtime_type(identity, constructor, body, &type_args, &environment);
        if !known || self.failed {
            return None;
        }
        let class = identity.clone();
        let mut fields = fields
            .into_iter()
            .map(|(name, value)| Some((class.clone(), name, value?)))
            .chain(inherited.into_iter().map(Some))
            .collect::<Option<Vec<_>>>()?;
        fields.sort();
        let (class, arguments) = runtime_type?;
        Some(ConstantValue::Instance(class, arguments, fields))
    }

    /// The canonical runtime type of an instance: the class and its type
    /// arguments, explicit or inferred from parameters typed by a class type
    /// parameter. `None` when a type argument is unknown.
    fn runtime_type(
        &self,
        identity: &DeclarationIdentity,
        constructor: &str,
        body: &ConstConstructorBody,
        type_args: &[ResolvedType],
        environment: &HashMap<String, Option<ConstantValue>>,
    ) -> Option<(DeclarationIdentity, Vec<ConstantType>)> {
        let signature = self.signatures.constructor(identity, constructor)?;
        if !type_args.is_empty()
            && let Some(arguments) = type_args
                .iter()
                .map(constant_type)
                .collect::<Option<Vec<_>>>()
        {
            return Some((identity.clone(), arguments));
        }
        let parameter_types = body
            .params
            .iter()
            .map(|param| param.cast::<FormalParam<'_>>())
            .filter(|param| !matches!(param.kind(), ParamKind::Named { .. }))
            .zip(&signature.positional)
            .map(|(param, ty)| (param.name().name(), ty))
            .chain(signature.named.iter().map(|(name, ty)| (name.as_str(), ty)));
        let arguments = signature
            .owner_parameters
            .iter()
            .enumerate()
            .map(|(index, owner_parameter)| {
                if let Some(argument) = type_args.get(index).and_then(constant_type) {
                    return Some(argument);
                }
                let (name, _) = parameter_types.clone().find(|(_, ty)| {
                    matches!(ty, ResolvedType::TypeParameter { id, nullable: false, .. } if id == owner_parameter)
                })?;
                value_type(environment.get(name)?.as_ref()?)
            })
            .collect::<Option<Vec<_>>>()?;
        Some((identity.clone(), arguments))
    }
}

fn type_size(ty: &ConstantType, limit: usize) -> Option<usize> {
    let mut remaining = limit.checked_sub(std::mem::size_of::<ConstantType>())?;
    let (positional, named) = match ty {
        ConstantType::Interface(_, arguments, _) => (arguments.as_slice(), &[][..]),
        ConstantType::Function(result, positional, named, _, _, parameters) => {
            remaining = remaining.checked_sub(type_size(result, remaining)?)?;
            if let Some(parameters) = parameters {
                for name in parameters {
                    remaining = remaining
                        .checked_sub(std::mem::size_of::<String>())?
                        .checked_sub(name.len())?;
                }
            }
            (positional.as_slice(), named.as_slice())
        }
        ConstantType::Record(positional, named, _) => (positional.as_slice(), named.as_slice()),
        _ => (&[][..], &[][..]),
    };
    for ty in positional {
        remaining = remaining.checked_sub(type_size(ty, remaining)?)?;
    }
    for (name, ty) in named {
        remaining = remaining
            .checked_sub(std::mem::size_of::<String>())?
            .checked_sub(name.len())?;
        remaining = remaining.checked_sub(type_size(ty, remaining)?)?;
    }
    Some(limit - remaining)
}

fn value_size(value: &ConstantValue, limit: usize) -> Option<usize> {
    let mut remaining = limit.checked_sub(std::mem::size_of::<ConstantValue>())?;
    let types = match value {
        ConstantValue::List(types, _)
        | ConstantValue::Set(types, _)
        | ConstantValue::Map(types, _)
        | ConstantValue::Instance(_, types, _) => types.as_slice(),
        _ => &[],
    };
    for ty in types {
        remaining = remaining.checked_sub(type_size(ty, remaining)?)?;
    }
    match value {
        ConstantValue::String(value) => remaining = remaining.checked_sub(value.len())?,
        ConstantValue::List(_, values) | ConstantValue::Set(_, values) => {
            for value in values {
                remaining = remaining.checked_sub(value_size(value, remaining)?)?;
            }
        }
        ConstantValue::Map(_, values) => {
            for (key, value) in values {
                remaining = remaining.checked_sub(value_size(key, remaining)?)?;
                remaining = remaining.checked_sub(value_size(value, remaining)?)?;
            }
        }
        ConstantValue::Record(values) => {
            for (name, value) in values {
                remaining = remaining
                    .checked_sub(std::mem::size_of::<Option<String>>())?
                    .checked_sub(name.as_ref().map_or(0, String::len))?;
                remaining = remaining.checked_sub(value_size(value, remaining)?)?;
            }
        }
        ConstantValue::Instance(_, _, values) => {
            for (_, name, value) in values {
                remaining = remaining
                    .checked_sub(std::mem::size_of::<DeclarationIdentity>())?
                    .checked_sub(std::mem::size_of::<String>())?
                    .checked_sub(name.len())?;
                remaining = remaining.checked_sub(value_size(value, remaining)?)?;
            }
        }
        ConstantValue::EnumValue(_, name) => {
            remaining = remaining.checked_sub(name.len())?;
        }
        ConstantValue::TearOff(_, name, ty) => {
            remaining = remaining.checked_sub(name.len())?;
            remaining = remaining.checked_sub(type_size(ty, remaining)?)?;
        }
        ConstantValue::Type(ty) => remaining = remaining.checked_sub(type_size(ty, remaining)?)?,
        _ => {}
    }
    Some(limit - remaining)
}

/// The runtime type of a constant value, when its type is exact.
fn value_type(value: &ConstantValue) -> Option<ConstantType> {
    let core = |name, arguments: &[ConstantType]| {
        Some(ConstantType::Interface(
            DeclarationIdentity::sdk("dart:core", name),
            arguments.to_vec(),
            false,
        ))
    };
    match value {
        ConstantValue::Null => Some(ConstantType::Null),
        ConstantValue::Bool(_) => core("bool", &[]),
        ConstantValue::String(_) => core("String", &[]),
        ConstantValue::Number(Number::Int(_)) => core("int", &[]),
        ConstantValue::Number(Number::Float(_)) => core("double", &[]),
        ConstantValue::Instance(class, arguments, _) => Some(ConstantType::Interface(
            class.clone(),
            arguments.clone(),
            false,
        )),
        // Both explicit and inferred collection runtime type arguments are retained.
        ConstantValue::List(types, _) => core("List", types),
        ConstantValue::Set(types, _) => core("Set", types),
        ConstantValue::Map(types, _) => core("Map", types),
        ConstantValue::Record(_) => None,
        // An enum's type arguments are not retained.
        ConstantValue::EnumValue(class, _) => {
            Some(ConstantType::Interface(class.clone(), vec![], false))
        }
        ConstantValue::TearOff(_, _, ty) => Some(ty.clone()),
        ConstantValue::Type(_) => core("Type", &[]),
    }
}

/// Integer literals (including unary negation) acquire double identity in a
/// double source context. Other integer expressions retain their runtime type.
/// Inspect the source before converting: integer evaluation alone loses `-0`.
pub fn contextual_number(
    value: ConstantValue,
    expression: Expr<'_>,
    double_context: bool,
) -> ConstantValue {
    if !double_context {
        return value;
    }
    let mut literal = expression;
    let mut negative = false;
    while let Expr::Unary(unary) = literal
        && unary.op() == UnaryOp::Minus
    {
        negative = !negative;
        literal = unary.operand();
    }
    if matches!(literal, Expr::IntLit(_))
        && let ConstantValue::Number(Number::Int(integer)) = value
    {
        let number = if integer == 0 && negative {
            -0.0
        } else {
            integer as f64
        };
        return ConstantValue::Number(Number::from_float(number));
    }
    value
}

fn decode_string(node: StringLitNode<'_>) -> Option<String> {
    if node.interpolations().next().is_some() {
        return None;
    }
    let mut output = String::with_capacity(node.syntax().text().len());
    if !node.is_adjacent() {
        let literal = node.delimiter()?;
        if !literal.terminated {
            return None;
        }
        decode_string_content(literal.content, literal.is_raw, literal.triple, &mut output)?;
        return Some(output);
    }
    let mut count = 0;
    for (literal, _) in node.segments() {
        if !literal.terminated {
            return None;
        }
        decode_string_content(literal.content, literal.is_raw, literal.triple, &mut output)?;
        count += 1;
    }
    (count == node.segment_spans().count().max(1)).then_some(output)
}

fn decode_string_content(
    content: &str,
    is_raw: bool,
    triple: bool,
    output: &mut String,
) -> Option<()> {
    let content = if triple
        && let Some(line_end) = content.find(['\r', '\n'])
        && {
            let mut chars = content[..line_end].chars();
            let mut whitespace = true;
            while let Some(character) = chars.next() {
                if !(matches!(character, ' ' | '\t')
                    || (character == '\\'
                        && chars.next().is_none_or(|next| matches!(next, ' ' | '\t'))))
                {
                    whitespace = false;
                    break;
                }
            }
            whitespace
        } {
        let mut start = line_end + 1;
        if content.as_bytes()[line_end] == b'\r' && content.as_bytes().get(start) == Some(&b'\n') {
            start += 1;
        }
        &content[start..]
    } else {
        content
    };
    if is_raw {
        output.push_str(content);
        return Some(());
    }
    let mut chars = content.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = chars.next()?;
        match escaped {
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            'b' => output.push('\u{0008}'),
            'f' => output.push('\u{000C}'),
            'v' => output.push('\u{000B}'),
            'x' => {
                let high = chars.next()?.to_digit(16)?;
                let low = chars.next()?.to_digit(16)?;
                output.push(char::from_u32(high * 16 + low)?);
            }
            'u' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut value = 0u32;
                let mut digits = 0;
                loop {
                    let next = chars.next()?;
                    if next == '}' {
                        break;
                    }
                    value = value.checked_mul(16)?.checked_add(next.to_digit(16)?)?;
                    digits += 1;
                }
                if digits == 0 {
                    return None;
                }
                output.push(char::from_u32(value)?);
            }
            'u' => {
                let mut value = 0u32;
                for _ in 0..4 {
                    value = value * 16 + chars.next()?.to_digit(16)?;
                }
                output.push(char::from_u32(value)?);
            }
            other => output.push(other),
        }
    }
    Some(())
}

/// The identity of a resolved type; `None` when it is unknown or names a
/// type parameter.
fn constant_type(ty: &ResolvedType) -> Option<ConstantType> {
    let positional =
        |types: &[ResolvedType]| types.iter().map(constant_type).collect::<Option<Vec<_>>>();
    let named = |types: &[(String, ResolvedType)]| {
        let mut result = types
            .iter()
            .map(|(name, ty)| Some((name.clone(), constant_type(ty)?)))
            .collect::<Option<Vec<_>>>()?;
        result.sort();
        Some(result)
    };
    Some(match ty {
        ResolvedType::Dynamic => ConstantType::Dynamic,
        ResolvedType::Void => ConstantType::Void,
        ResolvedType::Never => ConstantType::Never,
        ResolvedType::Null => ConstantType::Null,
        ResolvedType::Interface {
            identity,
            arguments,
            nullable,
            ..
        } => ConstantType::Interface(identity.clone(), positional(arguments)?, *nullable),
        ResolvedType::Function {
            return_type,
            positional: parameters,
            named: names,
            nullable,
            optional_positional,
            shape,
            ..
        } => ConstantType::Function(
            Box::new(constant_type(return_type)?),
            positional(parameters)?,
            named(names)?,
            *optional_positional,
            *nullable,
            shape
                .as_ref()
                .filter(|shape| shape.type_parameters.is_empty())
                .map(|shape| shape.required_named.to_vec()),
        ),
        ResolvedType::Record {
            positional: fields,
            named: names,
            nullable,
            ..
        } => ConstantType::Record(positional(fields)?, named(names)?, *nullable),
        ResolvedType::Unknown
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::NonNullTypeParameter { .. } => return None,
    })
}

fn resolved_constant_type(ty: &ConstantType) -> ResolvedType {
    let named = |types: &[(String, ConstantType)]| {
        types
            .iter()
            .map(|(name, ty)| (name.clone(), resolved_constant_type(ty)))
            .collect()
    };
    match ty {
        ConstantType::Dynamic => ResolvedType::Dynamic,
        ConstantType::Void => ResolvedType::Void,
        ConstantType::Never => ResolvedType::Never,
        ConstantType::Null => ResolvedType::Null,
        ConstantType::Interface(declaration, arguments, nullable) => ResolvedType::Interface {
            occurrence: Default::default(),
            identity: declaration.clone(),
            arguments: arguments.iter().map(resolved_constant_type).collect(),
            nullable: *nullable,
            extension_type: false,
        },
        ConstantType::Function(
            result,
            positional,
            names,
            optional_positional,
            nullable,
            required_named,
        ) => ResolvedType::Function {
            shape: required_named
                .as_ref()
                .map(|names| crate::resolve::FunctionTypeShape::new(names.clone(), Vec::new())),
            optional_positional: *optional_positional,
            alias: Default::default(),
            return_type: std::sync::Arc::new(resolved_constant_type(result)),
            positional: positional.iter().map(resolved_constant_type).collect(),
            named: named(names),
            nullable: *nullable,
        },
        ConstantType::Record(positional, names, nullable) => ResolvedType::Record {
            occurrence: Default::default(),
            positional: positional.iter().map(resolved_constant_type).collect(),
            named: named(names),
            nullable: *nullable,
        },
    }
}
fn eval_unary(op: &UnaryOp, value: ConstantValue) -> Option<ConstantValue> {
    match (op, value) {
        (UnaryOp::Minus, ConstantValue::Number(Number::Int(value))) => {
            let value = i64::try_from(value)
                .ok()
                .map(i64::wrapping_neg)
                .or_else(|| (value == i128::from(i64::MAX) + 1).then_some(i64::MIN))?;
            Some(ConstantValue::Number(Number::Int(i128::from(value))))
        }
        (UnaryOp::Minus, ConstantValue::Number(value)) => {
            Some(ConstantValue::Number(Number::from_float(-value.float())))
        }
        (UnaryOp::Bang, ConstantValue::Bool(value)) => Some(ConstantValue::Bool(!value)),
        (UnaryOp::Tilde, ConstantValue::Number(Number::Int(value))) => {
            Some(ConstantValue::Number(Number::Int(!value)))
        }
        _ => None,
    }
}

fn eval_equality(op: &BinaryOp, left: &ConstantValue, right: &ConstantValue) -> ConstantValue {
    let equal = match (left, right) {
        (ConstantValue::Number(Number::Int(left)), ConstantValue::Number(Number::Int(right))) => {
            left == right
        }
        (ConstantValue::Number(left), ConstantValue::Number(right)) => {
            left.float() == right.float()
        }
        _ => left == right,
    };
    ConstantValue::Bool(if matches!(op, BinaryOp::EqEq) {
        equal
    } else {
        !equal
    })
}

fn eval_binary(op: &BinaryOp, left: ConstantValue, right: ConstantValue) -> Option<ConstantValue> {
    match op {
        BinaryOp::EqEq | BinaryOp::NotEq => return Some(eval_equality(op, &left, &right)),
        BinaryOp::And => return bool_pair(left, right).map(|(l, r)| ConstantValue::Bool(l && r)),
        BinaryOp::Or => return bool_pair(left, right).map(|(l, r)| ConstantValue::Bool(l || r)),
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
            if matches!(
                (&left, &right),
                (ConstantValue::Bool(_), ConstantValue::Bool(_))
            ) =>
        {
            return bool_pair(left, right).map(|(left, right)| {
                ConstantValue::Bool(match op {
                    BinaryOp::BitAnd => left & right,
                    BinaryOp::BitOr => left | right,
                    _ => left ^ right,
                })
            });
        }
        BinaryOp::Add => {
            if let (ConstantValue::String(left), ConstantValue::String(right)) = (&left, &right) {
                return Some(ConstantValue::String(format!("{left}{right}")));
            }
        }
        _ => {}
    }
    let (ConstantValue::Number(left), ConstantValue::Number(right)) = (left, right) else {
        return None;
    };
    // Native Dart integer arithmetic wraps to signed 64 bits. Reject unknown
    // out-of-range operands, but retain the wrapped results of valid operations.
    if let (Number::Int(left), Number::Int(right)) = (left, right)
        && (i64::try_from(left).is_err() || i64::try_from(right).is_err())
    {
        return None;
    }
    let integer = |value: i128| Some(ConstantValue::Number(Number::Int(i128::from(value as i64))));
    match (op, left, right) {
        (BinaryOp::Add, Number::Int(left), Number::Int(right)) => integer(left.checked_add(right)?),
        (BinaryOp::Sub, Number::Int(left), Number::Int(right)) => integer(left.checked_sub(right)?),
        (BinaryOp::Mul, Number::Int(left), Number::Int(right)) => integer(left.checked_mul(right)?),
        (BinaryOp::IntDiv, Number::Int(left), Number::Int(right)) => {
            integer(left.checked_div(right)?)
        }
        (BinaryOp::Mod, Number::Int(left), Number::Int(right)) => {
            integer(left.checked_rem_euclid(right)?)
        }
        (BinaryOp::BitAnd, Number::Int(left), Number::Int(right)) => integer(left & right),
        (BinaryOp::BitOr, Number::Int(left), Number::Int(right)) => integer(left | right),
        (BinaryOp::BitXor, Number::Int(left), Number::Int(right)) => integer(left ^ right),
        (BinaryOp::Shl, Number::Int(left), Number::Int(right)) => {
            let shift = u64::try_from(right).ok()?;
            integer(if shift >= u64::BITS.into() {
                0
            } else {
                left << shift
            })
        }
        (BinaryOp::Shr, Number::Int(left), Number::Int(right)) => {
            let shift = u64::try_from(right).ok()?;
            integer(left >> shift.min(u64::from(u64::BITS - 1)))
        }
        (BinaryOp::UShr, Number::Int(left), Number::Int(right)) => {
            let shift = u64::try_from(right).ok()?;
            if shift == 0 {
                return integer(left);
            }
            let left = u64::from_ne_bytes(i64::try_from(left).ok()?.to_ne_bytes());
            integer(if shift >= u64::BITS.into() {
                0
            } else {
                i128::from(left >> shift)
            })
        }
        (BinaryOp::Div, left, right) => Some(ConstantValue::Number(Number::from_float(
            left.float() / right.float(),
        ))),
        (BinaryOp::Add, left, right) => Some(ConstantValue::Number(Number::from_float(
            left.float() + right.float(),
        ))),
        (BinaryOp::Sub, left, right) => Some(ConstantValue::Number(Number::from_float(
            left.float() - right.float(),
        ))),
        (BinaryOp::Mul, left, right) => Some(ConstantValue::Number(Number::from_float(
            left.float() * right.float(),
        ))),
        (BinaryOp::Mod, left, right) => {
            let remainder = left.float().rem_euclid(right.float());
            // Dart modulo has a nonnegative result, including positive zero.
            Some(ConstantValue::Number(Number::from_float(
                if remainder == 0.0 { 0.0 } else { remainder },
            )))
        }
        (BinaryOp::Lt, left, right) => Some(ConstantValue::Bool(left.float() < right.float())),
        (BinaryOp::Gt, left, right) => Some(ConstantValue::Bool(left.float() > right.float())),
        (BinaryOp::LtEq, left, right) => Some(ConstantValue::Bool(left.float() <= right.float())),
        (BinaryOp::GtEq, left, right) => Some(ConstantValue::Bool(left.float() >= right.float())),
        _ => None,
    }
}

fn bool_pair(left: ConstantValue, right: ConstantValue) -> Option<(bool, bool)> {
    match (left, right) {
        (ConstantValue::Bool(left), ConstantValue::Bool(right)) => Some((left, right)),
        _ => None,
    }
}

/// Parse a Dart integer literal's source spelling, honouring `0x`/`0X` prefixes and
/// `_` digit separators.
pub fn parse_int(value: &str) -> Option<i128> {
    let value = value.replace('_', "");
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        i128::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use falcon_dart_parser::parse;

    use super::*;
    use crate::{IdentityIndex, IdentitySource, ParsedFile, QualifiedName, TypeIndex};

    #[test]
    fn argument_type_fallback_matches_source_nodes_to_borrowed_values() {
        let source =
            "const result = make(value, value, first: value, missing: value, inexact: value);";
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let program = tree.program(source);
        let TopLevelDecl::Variable(declaration) = program.declarations().next().unwrap() else {
            panic!("expected a variable declaration");
        };
        let Some(Expr::Call(call)) = declaration.declarators().next().unwrap().initializer() else {
            panic!("expected a call initializer");
        };
        let positional = [Some(ConstantValue::Null), None];
        let named = [
            ("first".to_owned(), Some(ConstantValue::Bool(true))),
            ("missing".to_owned(), None),
            (
                "inexact".to_owned(),
                Some(ConstantValue::Record(Vec::new())),
            ),
        ];
        let fallback = ArgumentTypeFallback {
            args: call.args(),
            positional: &positional,
            named: &named,
        };
        let mut positional_arguments = call.args().positional();
        assert_eq!(
            fallback.lookup(positional_arguments.next().unwrap()),
            Some(ResolvedType::Null)
        );
        assert_eq!(fallback.lookup(positional_arguments.next().unwrap()), None);
        let mut named_arguments = call.args().named();
        assert_eq!(
            fallback.lookup(named_arguments.next().unwrap().value()),
            Some(resolved_constant_type(&ConstantType::Interface(
                DeclarationIdentity::sdk("dart:core", "bool"),
                Vec::new(),
                false,
            )))
        );
        assert_eq!(
            fallback.lookup(named_arguments.next().unwrap().value()),
            None
        );
        assert_eq!(
            fallback.lookup(named_arguments.next().unwrap().value()),
            None
        );
        assert_eq!(fallback.lookup(call.callee()), None);
    }

    #[test]
    fn resolved_constant_type_preserves_interned_library_names() {
        with_constants(|_, model, _| {
            let identity = model.resolve_name("ConstBox").unwrap();
            let DeclarationIdentity::Library {
                name: crate::InternedString::Shared(name),
                ..
            } = &identity
            else {
                panic!("expected a shared library name");
            };
            let constant = constant_type(&ResolvedType::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: Vec::new().into(),
                nullable: false,
                extension_type: false,
            })
            .unwrap();
            let ResolvedType::Interface {
                identity:
                    DeclarationIdentity::Library {
                        name: crate::InternedString::Shared(restored),
                        ..
                    },
                ..
            } = resolved_constant_type(&constant)
            else {
                panic!("expected a shared library name");
            };
            assert!(Arc::ptr_eq(name, &restored));
        });
    }

    #[test]
    fn string_decoding_rejects_unterminated_or_unsegmented_literals() {
        for (raw, expected) in [
            ("'hello", None),
            ("'a'\"b\"", None),
            ("r'\\n'", Some("\\n")),
            ("'\\n'", Some("\n")),
            ("'", None),
        ] {
            use falcon_syntax::cst::{Event, NodeKind, SyntaxTree};
            let tree = SyntaxTree::build(
                raw,
                &[],
                vec![
                    Event::Start {
                        kind: if raw == "'a'\"b\"" {
                            NodeKind::AdjacentStrings
                        } else {
                            NodeKind::StringLitNode
                        },
                        forward_parent: None,
                        offset: 0,
                    },
                    Event::RecoverySpan(Span::new(0, raw.len())),
                    Event::Finish,
                ],
            );
            let node = StringLitNode::cast(tree.root(raw)).unwrap();
            assert_eq!(decode_string(node).as_deref(), expected, "{raw}");
        }
    }

    #[test]
    fn string_decoding_concatenates_decoded_adjacent_segments() {
        let source = r#"const value = 'a\n' r'\b' "\x63";"#;
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Variable(declaration) = program.declarations().next().unwrap() else {
            panic!()
        };
        let Some(Expr::StringLit(node)) = declaration.declarators().next().unwrap().initializer()
        else {
            panic!()
        };
        assert_eq!(decode_string(node).as_deref(), Some("a\n\\bc"));
    }

    #[test]
    fn constant_operands_check_dead_branches_and_stop_at_rejection() {
        for source in [
            "const value = true ? 1 : blocked + later;",
            "const value = false && blocked && later;",
        ] {
            let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Variable(declaration) =
                tree.program(source).declarations().next().unwrap()
            else {
                panic!("expected a constant declaration");
            };
            let expression = declaration
                .declarators()
                .next()
                .unwrap()
                .initializer()
                .unwrap();
            let mut names = Vec::new();
            assert!(!constant_operands_all(expression, |operand| {
                if let Expr::Ident(identifier) = operand {
                    names.push(identifier.name());
                    return identifier.name() != "blocked";
                }
                true
            }));
            assert_eq!(names, ["blocked"]);
            assert!(constant_operands_all(expression, |_| true));
        }
    }

    #[test]
    fn primitive_equality_rejects_custom_and_unknown_classes() {
        with_constants(|_, model, signatures| {
            assert!(class_has_primitive_equality(
                &DeclarationIdentity::OBJECT,
                signatures
            ));
            assert!(class_has_primitive_equality(
                &model.resolve_name("Shade").unwrap(),
                signatures,
            ));
            assert!(class_has_primitive_equality(
                &model.resolve_name("ConstBox").unwrap(),
                signatures,
            ));
            for name in ["CustomEquality", "InheritedEquality"] {
                assert!(!class_has_primitive_equality(
                    &model.resolve_name(name).unwrap(),
                    signatures,
                ));
            }
            assert!(!class_has_primitive_equality(
                &DeclarationIdentity::sdk("dart:core", "UnknownClass"),
                signatures,
            ));
        });
    }

    fn with_constants(test: impl FnOnce(Program<'_>, &SemanticModel<'_>, &SignatureIndex)) {
        let source = r#"
import 'dart:async';
class ConstBox<T> {
  final T value;
  const ConstBox(this.value);
}
class MutableBox<T> {
  final T value;
  MutableBox(this.value);
}
final explicitNonConst = new ConstBox<int>(1);
const implicitNonConst = MutableBox<int>(1);
final explicitConst = const ConstBox<int>(1);
const implicitConst = ConstBox<int>(1);
const cycleA = cycleB;
const cycleB = cycleA;
const leaf = 2;
const repeated = leaf + leaf;
class Foo { static const a = 1; }
const a = 2;
const b = a;
const qualifiedOwner = Foo.a;
const intOne = 1;
const doubleOne = 1.0;
const doubleSum = doubleOne + intOne;
const doubleDivision = 2 / 2;
const numericEquality = intOne == doubleOne;
const intList = <int>[1];
const numList = <num>[1];
const intListAgain = <int>[1];
const intSet = <int>{1};
const numSet = <num>{1};
const intMap = <String, int>{'a': 1};
const numMap = <String, num>{'a': 1};
const numBox = ConstBox<num>(1);
const v = 0;
class const PrimaryBox(int v) {
  final int w = v;
}
class OrdinaryBox {
  final int w = v;
  const OrdinaryBox(int v);
}
class const PrimaryOuter(int v) {
  final OrdinaryBox inner = OrdinaryBox(v);
  final int w = v;
}
const primaryOne = PrimaryBox(1);
const primaryTwo = PrimaryBox(2);
const ordinaryOne = OrdinaryBox(1);
const ordinaryTwo = OrdinaryBox(2);
const primaryOuter = PrimaryOuter(3);
class const FailedPrimary(int v) {
  final int w = v;
  this : assert(v == 0);
}
const failedPrimary = FailedPrimary(1);
class ListBox<T> {
  final List<T> items;
  const ListBox(this.items);
  const ListBox.named({required this.items});
  const ListBox.empty() : items = const [];
  const ListBox.redirect(List<T> values) : this(values);
}
class ListChild<T> extends ListBox<T> {
  const ListChild() : super(const []);
}
class InitializedList<T> {
  final List<T> items = const [];
  const InitializedList();
}
class ConstantScope {
  static const one = 1;
  static const List<double> unit = [1];
  static const List<double> negative = [-0];
  static const inferred = [one];
}
const genericList = ListBox([1]);
const explicitList = ListBox<int>(const []);
const namedList = ListBox<double>.named(items: const [1]);
const initializedList = InitializedList<int>();
const emptyList = ListBox<int>.empty();
const redirectedList = ListBox<int>.redirect(const []);
const inheritedList = ListChild<int>();
const doubleBox = ConstBox<double>(1);
const integerBox = ConstBox<double>(intOne);
const List<double> declaredUnit = [1];
const List<double> declaredNegative = [-0];
const capturedList = [a];
const parameterList = ListBox([v]);
const num declaredNum = 1;
const int? declaredNullable = 1;
const numValues = [declaredNum];
const nullableValues = [declaredNullable];
const numInstance = ConstBox(declaredNum);
class NumScope {
  static const num number = 1;
  static const values = [number];
}
class ParameterBox {
  final Object value;
  const ParameterBox(num number) : value = ConstBox(number);
}
const parameterBox = ParameterBox(1);
const orderedSet = <int>{1, 2};
const reorderedSet = <int>{2, 1};
const negativeModulo = -7 % 3;
const negativeDivisorModulo = -7 % -3;
const negativeDoubleModulo = -7.5 % 2;
const negativeZeroModulo = -4.0 % 2;
const signedZeroModulo = -0.0 % 5;
const shortAnd = false && 1 ~/ 0 == 0;
const shortOr = true || 1 ~/ 0 == 0;
const boolBits = (true ^ false) & true | false;
const constructorEquality = CustomEquality.new == CustomEquality.new;
class CustomEquality {
  const CustomEquality();
  bool operator ==(Object other) => true;
}
class InheritedEquality extends CustomEquality {}
const customEquality = CustomEquality() == CustomEquality();
enum Shade { red }
const Shade shade = .red;
const invalidShadeCast = shade as int;
class ImplicitConstructor {}
const implicitConstructor = ImplicitConstructor.new;
const implicitConstructorEquality = ImplicitConstructor.new == ImplicitConstructor.new;
typedef ImplicitAlias = ImplicitConstructor;
const aliasConstructor = ImplicitAlias.new;
const aliasConstructorEquality = ImplicitAlias.new == ImplicitConstructor.new;
unannotated() {}
const unannotatedFunction = unannotated;
const all = 1;
class ConstructorNameScope {
  const ConstructorNameScope.all();
  static const value = all as int;
}
const constructorNameValue = ConstructorNameScope.value;
const shadow = 1;
class MethodNameScope {
  static void shadow() {}
  static const list = [shadow];
}
const methodNameList = MethodNameScope.list;
const namedText = 'x';
const namedTextLength = namedText.length;
const utf16TextLength = '😀'.length;
const shiftedBeyondWord = 1 << 70;
const shiftedSignBit = 1 << 63;
const negatedSignBit = -(1 << 63);
const overflowSum = 9223372036854775807 + 1;
const overflowDifference = -9223372036854775808 - 1;
const overflowProduct = 9223372036854775807 * 2;
const shiftedNegativeBeyondWord = -1 >> 70;
const shiftedUnsignedBeyondWord = -1 >>> 70;
const typeTestBranch = ConstBox(1) is String ? 1 : 2;
const negatedTypeTestBranch = ConstBox(1) is! String ? 1 : 2;
const positiveTypeTestBranch = leaf is int ? leaf : 0;
const nullIsNull = null is Null;
const nullIsNullable = null is int?;
const nullIsNonNullable = null is int;
const nullIsNotNullable = null is! int?;
const nullListIsNullable = <Null>[] is List<int?>;
const nullListIsNonNullable = <Null>[] is List<int>;
const nullIsFutureOrNullable = null is FutureOr<int?>;
const nullIsNotFutureOrNullable = null is! FutureOr<int?>;
const nullIsFutureOrDynamic = null is FutureOr<dynamic>;
const nullIsFutureOrNull = null is FutureOr<Null>;
const nullIsFutureOrInt = null is FutureOr<int>;
const nullIsNullableFutureOr = null is FutureOr<int>?;
const intIsFutureOr = 1 is FutureOr<int>;
const nullListIsFutureOrNullable = <Null>[] is List<FutureOr<int?>>;
const nullListIsNotFutureOrNullable = <Null>[] is! List<FutureOr<int?>>;
const nullListIsFutureOrDynamic = <Null>[] is List<FutureOr<dynamic>>;
const nullListIsFutureOrNull = <Null>[] is List<FutureOr<Null>>;
const intListIsFutureOr = <int>[] is List<FutureOr<int>>;
const nullableListIsFutureOr = <int?>[] is List<FutureOr<int?>>;
const nullableCast = null as int?;
const nonNullableCast = null as int;
const futureOrCast = null as FutureOr<int?>;
"#;
        with_source(source, test);
    }

    fn with_source(
        source: &str,
        test: impl FnOnce(Program<'_>, &SemanticModel<'_>, &SignatureIndex),
    ) {
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.tree.program(&parsed.source);
        let sources = [IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(program);
        let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(program, &model);
        test(program, &model, &signatures);
    }

    fn evaluate_named(
        program: Program<'_>,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        name: &str,
    ) -> Option<ConstantValue> {
        let initializer = program.declarations().find_map(|declaration| {
            let TopLevelDecl::Variable(variable) = declaration else {
                return None;
            };
            variable
                .declarators()
                .find(|declarator| declarator.name().name() == name)
                .and_then(|declarator| declarator.initializer())
        })?;
        let owner = model.resolve_value(name)?;
        evaluate_constant(
            initializer,
            ConstantContext {
                owner: Some(&owner),
                ..ConstantContext::new(model, signatures)
            },
        )
    }

    #[test]
    fn exponential_constants_exhaust_size_budget() {
        use std::fmt::Write;

        let mut source = String::from("const s0 = 'x'; const l0 = [0];\n");
        for index in 1..=64 {
            let previous = index - 1;
            writeln!(source, "const s{index} = s{previous} + s{previous};").unwrap();
            writeln!(source, "const l{index} = [l{previous}, l{previous}];").unwrap();
        }
        with_source(&source, |program, model, signatures| {
            assert_eq!(
                evaluate_named(program, model, signatures, "s8"),
                Some(ConstantValue::String("x".repeat(256)))
            );
            assert!(evaluate_named(program, model, signatures, "l4").is_some());
            for name in ["s24", "s64", "l24", "l64"] {
                assert_eq!(
                    evaluate_named(program, model, signatures, name),
                    None,
                    "{name}"
                );
            }
        });
    }

    #[test]
    fn failed_dependencies_are_cached_in_diamond_chains() {
        use std::fmt::Write;

        let mut source = String::from(
            "class Pair { final Object a, b; const Pair(this.a, this.b); }\nconst f0 = #unknown;\n",
        );
        for index in 1..=24 {
            writeln!(
                source,
                "const f{index} = Pair(f{}, f{});",
                index - 1,
                index - 1
            )
            .unwrap();
        }
        crate::with_rules_stack(|| {
            with_source(&source, |_, model, signatures| {
                let fields = HashMap::new();
                let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                assert!(evaluator.resolve_name_shared(&["f24"]).is_none());
                assert!(evaluator.pending_constants.is_empty());
                assert!(
                    evaluator
                        .constants
                        .values()
                        .all(|state| matches!(state, ConstantState::Evaluated(_)))
                );
            });
        })
        .unwrap();
    }

    #[test]
    fn wide_constant_dependencies_are_evaluated_before_materialization() {
        use std::fmt::Write;

        let mut source = String::new();
        for index in 0..1_000 {
            writeln!(source, "const c{index} = {index};").unwrap();
        }
        source.push_str("const all = <int>[");
        for index in 0..1_000 {
            write!(source, "c{index},").unwrap();
        }
        source.push_str("]; const alias = all;");
        with_source(&source, |_, model, signatures| {
            let fields = HashMap::new();
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            let ConstantValue::List(_, values) = evaluator.resolve_name(&["alias"]).unwrap() else {
                panic!();
            };
            assert_eq!(values.len(), 1_000);
            assert_eq!(values[999], ConstantValue::Number(Number::Int(999)));
        });
    }

    #[test]
    fn strict_binary_dependencies_are_evaluated_eagerly() {
        use std::fmt::Write;

        let mut source = String::new();
        for index in 0..200 {
            writeln!(source, "const c{index} = 1;").unwrap();
        }
        source.push_str("const value = c0");
        for index in 1..200 {
            write!(source, " | c{index}").unwrap();
        }
        source.push(';');
        crate::with_rules_stack(|| {
            with_source(&source, |_, model, signatures| {
                let fields = HashMap::new();
                let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                assert_eq!(
                    evaluator.resolve_name(&["value"]),
                    Some(ConstantValue::Number(Number::Int(1)))
                );
            });
        })
        .unwrap();
    }

    #[test]
    fn deferred_materialization_still_consumes_the_work_budget() {
        use std::fmt::Write;

        let mut source = format!("const big = '{}';\n", "x".repeat(512));
        for index in 0..32 {
            writeln!(source, "const n{index} = null;").unwrap();
        }
        source.push_str("const value = [big, ");
        for index in 0..32 {
            write!(source, "n{index} ?? ").unwrap();
        }
        source.push_str("0];");
        with_source(&source, |_, model, signatures| {
            let fields = HashMap::new();
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            evaluator.remaining_bytes = 8192;
            assert!(evaluator.resolve_name_shared(&["value"]).is_none());
            assert!(evaluator.pending_constants.is_empty());
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            assert!(evaluator.resolve_name_shared(&["value"]).is_some());
        });
    }

    #[test]
    fn exhausted_budget_does_not_poison_shared_constants() {
        with_source("const value = [1, 2, 3];", |_, model, signatures| {
            let fields = HashMap::new();
            let cache = ConstantCache::default();
            let mut first = ConstEvaluator::new(None, &fields, model, signatures);
            first.shared_cache = Some(&cache);
            first.remaining_bytes = 1;
            assert!(first.resolve_name_shared(&["value"]).is_none());
            assert!(cache.values.lock().unwrap().values.is_empty());
            let mut second = ConstEvaluator::new(None, &fields, model, signatures);
            second.shared_cache = Some(&cache);
            assert!(second.resolve_name_shared(&["value"]).is_some());
        });
    }

    #[test]
    fn stable_queries_and_shared_cache_are_order_independent() {
        with_source(
            "const dependency = int.fromEnvironment('FALCON_TEST', defaultValue: 7); \
             const value = dependency;",
            |program, model, signatures| {
                let expression = program
                    .declarations()
                    .find_map(|declaration| {
                        let TopLevelDecl::Variable(variable) = declaration else {
                            return None;
                        };
                        variable
                            .declarators()
                            .find(|declarator| declarator.name().name() == "value")
                            .and_then(|declarator| declarator.initializer())
                    })
                    .unwrap();
                for modes in [[false, true, false, true], [true, false, true, false]] {
                    let cache = ConstantCache::default();
                    for stable in modes {
                        assert_eq!(
                            evaluate_constant(
                                expression,
                                ConstantContext {
                                    cache: Some(&cache),
                                    stable,
                                    ..ConstantContext::new(model, signatures)
                                },
                            ),
                            if stable {
                                None
                            } else {
                                Some(ConstantValue::Number(Number::Int(7)))
                            },
                        );
                    }
                }
            },
        );
    }

    #[test]
    fn deferred_constants_have_independent_constructor_depth() {
        let source = "class Leaf { final Object x; const Leaf(int n): x = n == 0 ? 0 : Leaf(n - 1); }\n\
            class Ignore { const Ignore(Object? ignored); }\n\
            class D { final Object c; const D(int n): c = n == 0 ? Ignore(leaf) : D(n - 1); }\n\
            const leaf = Leaf(1); const deep = D(30);\n\
            const first = [leaf, deep]; const second = [deep, leaf];";
        crate::with_rules_stack(|| {
            with_source(source, |_, model, signatures| {
                for name in ["first", "second"] {
                    let fields = HashMap::new();
                    let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                    assert!(evaluator.resolve_name_shared(&[name]).is_some(), "{name}");
                }
            });
        })
        .unwrap();
    }

    #[test]
    fn file_cache_reuses_alias_values_across_independent_evaluators() {
        use std::fmt::Write;

        let mut source = String::from("const a0 = true;\n");
        for index in 1..=1000 {
            writeln!(source, "const a{index} = a{};", index - 1).unwrap();
        }
        with_source(&source, |_, model, signatures| {
            let cache = ConstantCache::default();
            let fields = HashMap::new();
            let mut first = ConstEvaluator::new(None, &fields, model, signatures);
            first.shared_cache = Some(&cache);
            let value = first.resolve_name_shared(&["a1000"]).unwrap();
            assert_eq!(first.constants.len(), 1001);
            assert_eq!(cache.values.lock().unwrap().values.len(), 1001);
            assert_eq!(
                cache.values.lock().unwrap().remaining_bytes,
                1024 * 1024 - value_size(&value, usize::MAX).unwrap()
            );
            for _ in 0..1000 {
                let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                evaluator.shared_cache = Some(&cache);
                let cached = evaluator.resolve_name_shared(&["a1000"]).unwrap();
                assert!(Arc::ptr_eq(&value, &cached));
                assert_eq!(evaluator.constants.len(), 1);
            }
        });
    }

    #[test]
    fn file_cache_bounds_distinct_materialized_values_without_changing_results() {
        use std::fmt::Write;

        let mut source = format!("const base = '{}';\n", "x".repeat(4096));
        for index in 0..512 {
            writeln!(source, "const value{index} = [base, {index}];").unwrap();
        }
        with_source(&source, |_, model, signatures| {
            let cache = ConstantCache::default();
            let fields = HashMap::new();
            for index in 0..512 {
                let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                evaluator.shared_cache = Some(&cache);
                assert!(
                    evaluator
                        .resolve_name_shared(&[&format!("value{index}")])
                        .is_some()
                );
            }
            let cached = cache.values.lock().unwrap();
            assert!(cached.remaining_bytes < 4096);
            assert!(cached.values.len() < 512);
            assert_eq!(cached.retained.len(), cached.values.len());
        });
    }

    #[test]
    fn file_cache_preserves_cycles_and_constructor_assertion_failures() {
        let source = "class Bad { const Bad(): assert(false); }\n\
            class Ignore { const Ignore(Object? ignored); }\n\
            const bad = Bad(); const ignored = Ignore(bad);\n\
            const cycle = other; const other = cycle; const good = true;";
        crate::with_rules_stack(|| {
            with_source(source, |_, model, signatures| {
                let cache = ConstantCache::default();
                let fields = HashMap::new();
                for name in ["bad", "ignored", "cycle", "other", "bad", "ignored", "good"] {
                    let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                    evaluator.shared_cache = Some(&cache);
                    let value = evaluator.resolve_name_shared(&[name]);
                    if name == "good" {
                        assert_eq!(value.as_deref(), Some(&ConstantValue::Bool(true)));
                        assert!(!evaluator.failed);
                    } else {
                        assert!(value.is_none(), "{name}");
                        if matches!(name, "bad" | "ignored") {
                            assert!(evaluator.failed);
                        }
                    }
                }
            });
        })
        .unwrap();
    }

    #[test]
    fn aliases_share_cached_values_and_have_independent_depth_limits() {
        use std::fmt::Write;

        let mut source = String::from(
            "const a0 = 0;\nclass Node {\nfinal Object child;\n\
             const Node(int n): child = n == 0 ? a110 : Node(n - 1);\n}\n\
             const nested32 = Node(31);\nconst nested33 = Node(32);\n",
        );
        for index in 1..=512 {
            writeln!(source, "const a{index} = a{};", index - 1).unwrap();
        }
        crate::with_rules_stack(|| {
            with_source(&source, |program, model, signatures| {
                assert_eq!(
                    evaluate_named(program, model, signatures, "a110"),
                    Some(ConstantValue::Number(Number::Int(0)))
                );
                assert_eq!(
                    evaluate_named(program, model, signatures, "a512"),
                    Some(ConstantValue::Number(Number::Int(0)))
                );
                assert!(evaluate_named(program, model, signatures, "nested32").is_some());
                assert_eq!(evaluate_named(program, model, signatures, "nested33"), None);
                let fields = HashMap::new();
                let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
                assert!(evaluator.resolve_name_shared(&["a512"]).is_some());
                let first = evaluator.resolve_name_shared(&["a110"]).unwrap();
                let second = evaluator.resolve_name_shared(&["a109"]).unwrap();
                assert!(Arc::ptr_eq(&first, &second));
            })
        })
        .unwrap();
    }

    #[test]
    fn hundred_thousand_aliases_use_a_dependency_worklist() {
        use std::fmt::Write;

        let mut source = String::with_capacity(3_000_000);
        source.push_str("const a0 = 0;\n");
        for index in 1..=100_000 {
            writeln!(source, "const a{index} = a{};", index - 1).unwrap();
        }
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty());
        let fields: HashMap<_, _> = parsed
            .tree
            .program(&source)
            .declarations()
            .filter_map(|declaration| match declaration {
                TopLevelDecl::Variable(variable) => Some(variable),
                _ => None,
            })
            .flat_map(|variable| variable.declarators())
            .map(|declarator| (declarator.name().name(), declarator.initializer().unwrap()))
            .collect();
        with_source("", |_, model, signatures| {
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            assert_eq!(
                evaluator.resolve_name(&["a100000"]),
                Some(ConstantValue::Number(Number::Int(0)))
            );
            assert_eq!(evaluator.constants.len(), 100_001);
        });
    }

    #[test]
    fn type_tests_preserve_nullability_and_leave_future_or_unknown() {
        with_constants(|program, model, signatures| {
            for (name, expected) in [
                ("nullIsNull", true),
                ("nullIsNullable", true),
                ("nullIsNonNullable", false),
                ("nullIsNotNullable", false),
                ("nullListIsNullable", true),
                ("nullListIsNonNullable", false),
            ] {
                assert_eq!(
                    evaluate_named(program, model, signatures, name),
                    Some(ConstantValue::Bool(expected)),
                    "{name}"
                );
            }
            for name in [
                "nullIsFutureOrNullable",
                "nullIsNotFutureOrNullable",
                "nullIsFutureOrDynamic",
                "nullIsFutureOrNull",
                "nullIsFutureOrInt",
                "nullIsNullableFutureOr",
                "intIsFutureOr",
                "nullListIsFutureOrNullable",
                "nullListIsNotFutureOrNullable",
                "nullListIsFutureOrDynamic",
                "nullListIsFutureOrNull",
                "intListIsFutureOr",
                "nullableListIsFutureOr",
                "nonNullableCast",
                "futureOrCast",
            ] {
                assert_eq!(
                    evaluate_named(program, model, signatures, name),
                    None,
                    "{name}"
                );
            }
            assert_eq!(
                evaluate_named(program, model, signatures, "nullableCast"),
                Some(ConstantValue::Null)
            );
        });
    }

    #[test]
    fn explicit_new_is_not_a_constant_constructor_invocation() {
        with_constants(|program, model, signatures| {
            assert_eq!(
                evaluate_named(program, model, signatures, "explicitNonConst"),
                None
            );
        });
    }

    #[test]
    fn constructor_call_requires_a_const_constructor() {
        with_constants(|program, model, signatures| {
            assert_eq!(
                evaluate_named(program, model, signatures, "implicitNonConst"),
                None
            );
        });
    }

    #[test]
    fn contextual_double_conversion_requires_an_integer_literal() {
        for (source, integer, expected) in [
            ("const value = 1;", 1, Number::from_float(1.0)),
            ("const value = -0;", 0, Number::from_float(-0.0)),
            ("const value = 0;", 0, Number::from_float(0.0)),
            ("const value = 1 + 0;", 1, Number::Int(1)),
            ("const value = integer;", 1, Number::Int(1)),
        ] {
            let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
            assert!(errors.is_empty(), "{errors:?}");
            let program = tree.program(source);
            let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
                panic!()
            };
            let expression = variable
                .declarators()
                .next()
                .unwrap()
                .initializer()
                .unwrap();
            let value = ConstantValue::Number(Number::Int(integer));
            assert_eq!(
                contextual_number(value.clone(), expression, true),
                ConstantValue::Number(expected),
                "{source}"
            );
            assert_eq!(contextual_number(value.clone(), expression, false), value);
        }
    }

    #[test]
    fn const_generic_constructor_forms_remain_evaluable() {
        with_constants(|program, model, signatures| {
            for name in ["explicitConst", "implicitConst"] {
                assert!(matches!(
                    evaluate_named(program, model, signatures, name),
                    Some(ConstantValue::Instance(_, _, fields))
                        if fields.iter().map(|(_, _, value)| value).eq([&ConstantValue::Number(Number::Int(1))])
                ));
            }
        });
    }

    #[test]
    fn primary_field_initializers_see_parameters_but_ordinary_ones_do_not() {
        with_constants(|program, model, signatures| {
            let value = |name| evaluate_named(program, model, signatures, name);
            for (name, expected) in [("primaryOne", 1), ("primaryTwo", 2), ("ordinaryOne", 0)] {
                assert!(
                    matches!(
                        value(name),
                        Some(ConstantValue::Instance(_, _, fields))
                            if fields.iter().map(|(_, _, value)| value)
                                .eq([&ConstantValue::Number(Number::Int(expected))])
                    ),
                    "{name}"
                );
            }
            assert_ne!(value("primaryOne"), value("primaryTwo"));
            assert_eq!(value("ordinaryOne"), value("ordinaryTwo"));
            let ordinary_one = value("ordinaryOne");
            assert!(matches!(
                value("primaryOuter"),
                Some(ConstantValue::Instance(_, _, fields))
                    if fields.iter().any(|(_, name, value)|
                        name == "w" && *value == ConstantValue::Number(Number::Int(3)))
                    && fields.iter().any(|(_, name, value)|
                        name == "inner" && Some(value) == ordinary_one.as_ref())
            ));
        });
    }

    #[test]
    fn failed_primary_constructor_restores_the_callers_scope() {
        with_constants(|_, model, signatures| {
            let identity = model.resolve_name("FailedPrimary").unwrap();
            let fields = HashMap::new();
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            let scope =
                HashMap::from([("v".to_string(), Some(ConstantValue::Number(Number::Int(7))))]);
            evaluator.parameters.push(scope.clone());
            assert_eq!(
                evaluator.construct(
                    identity,
                    "new",
                    Vec::new(),
                    vec![Some(ConstantValue::Number(Number::Int(1)))],
                    Vec::new(),
                ),
                None
            );
            assert_eq!(evaluator.parameters, vec![scope]);
        });
    }

    #[test]
    fn constant_cycles_stop_without_rejecting_repeated_references() {
        with_constants(|program, model, signatures| {
            assert_eq!(evaluate_named(program, model, signatures, "cycleA"), None);
            assert_eq!(
                evaluate_named(program, model, signatures, "repeated"),
                Some(ConstantValue::Number(Number::Int(4)))
            );
        });
    }

    #[test]
    fn top_level_initializers_do_not_capture_class_fields() {
        with_constants(|program, model, signatures| {
            let field = program
                .declarations()
                .find_map(|declaration| {
                    let TopLevelDecl::Class(class) = declaration else {
                        return None;
                    };
                    if class.name().name() != "Foo" {
                        return None;
                    }
                    class.members().find_map(|member| {
                        let ClassMember::Field(field) = member else {
                            return None;
                        };
                        field.declarators().next()?.initializer()
                    })
                })
                .unwrap();
            let owner = model.resolve_name("Foo").unwrap();
            let fields = HashMap::from([("a", field)]);
            let mut evaluator = ConstEvaluator::new(Some(&owner), &fields, model, signatures);
            assert_eq!(
                evaluator.resolve_name(&["b"]),
                Some(ConstantValue::Number(Number::Int(2)))
            );
            assert_eq!(
                evaluator.resolve_name(&["qualifiedOwner"]),
                Some(ConstantValue::Number(Number::Int(1)))
            );
            assert_eq!(
                evaluator.resolve_name(&["a"]),
                Some(ConstantValue::Number(Number::Int(1)))
            );
        });
    }

    #[test]
    fn member_context_sees_the_owners_static_namespace() {
        with_constants(|_, model, signatures| {
            let owner = model.resolve_name("Foo").unwrap();
            let source = "const expression = a;";
            let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
            let program = tree.program(source);
            assert!(errors.is_empty());
            let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
                panic!()
            };
            let expression = variable
                .declarators()
                .next()
                .unwrap()
                .initializer()
                .unwrap();
            assert_eq!(
                evaluate_constant(expression, ConstantContext::new(model, signatures)),
                Some(ConstantValue::Number(Number::Int(2)))
            );
            assert_eq!(
                evaluate_constant(
                    expression,
                    ConstantContext {
                        owner: Some(&owner),
                        ..ConstantContext::new(model, signatures)
                    }
                ),
                Some(ConstantValue::Number(Number::Int(1)))
            );
        });
    }

    #[test]
    fn imported_constants_resolve_private_names_in_their_declaring_library() {
        let inputs = [
            (
                PathBuf::from("/project/lib/origin.dart"),
                "import 'box.dart' as box; const _y = 2; const x = _y; class Foo { static const a = _y; static const boxed = box.ConstBox(_y); static const c = qualified; static const chain = a + a; static const cycleA = cycleB; static const cycleB = cycleA; static const _private = 3; static final notConst = 2; final instance = 2; } const qualified = Foo.a; enum Mode { first, second; static const defaultMode = first; }",
            ),
            (
                PathBuf::from("/project/lib/main.dart"),
                "import 'origin.dart' as origin; const _y = 1; const value = origin.x; const fieldValue = origin.Foo.chain; const boxedValue = origin.Foo.boxed; const cyclic = origin.Foo.cycleA; const mode = origin.Mode.first; const defaultMode = origin.Mode.defaultMode; const otherMode = origin.Mode.second; const privateValue = origin.Foo._private; const mutableValue = origin.Foo.notConst; const instanceValue = origin.Foo.instance;",
            ),
            (
                PathBuf::from("/project/lib/box.dart"),
                "class ConstBox { final int value; const ConstBox(this.value); }",
            ),
        ];
        let parsed_files: Vec<_> = inputs
            .iter()
            .map(|(_, source)| {
                let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
                assert!(errors.is_empty(), "{errors:?}");
                Arc::new(ParsedFile {
                    source: (*source).into(),
                    tree,
                })
            })
            .collect();
        let programs: Vec<_> = parsed_files
            .iter()
            .map(|parsed| parsed.tree.program(&parsed.source))
            .collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed_files)
            .map(|((path, _), parsed)| IdentitySource {
                path,
                parsed,
                has_parse_errors: false,
            })
            .collect();
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(programs[0]);
        let files: Vec<_> = inputs
            .iter()
            .zip(&parsed_files)
            .map(|((path, _), parsed)| (path.clone(), parsed.clone()))
            .collect();
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&inputs[1].0, &parsed_files[1], &identities, Some(&types));
        assert_eq!(
            evaluate_named(programs[1], &model, &signatures, "value"),
            Some(ConstantValue::Number(Number::Int(2)))
        );
        assert_eq!(
            evaluate_named(programs[1], &model, &signatures, "fieldValue"),
            Some(ConstantValue::Number(Number::Int(4)))
        );
        assert!(matches!(
            evaluate_named(programs[1], &model, &signatures, "boxedValue"),
            Some(ConstantValue::Instance(..))
        ));
        for name in ["cyclic", "privateValue", "mutableValue", "instanceValue"] {
            assert_eq!(
                evaluate_named(programs[1], &model, &signatures, name),
                None,
                "{name}"
            );
        }
        let mode = evaluate_named(programs[1], &model, &signatures, "mode").unwrap();
        assert_eq!(
            evaluate_named(programs[1], &model, &signatures, "defaultMode"),
            Some(mode.clone())
        );
        assert!(matches!(
            evaluate_named(programs[1], &model, &signatures, "otherMode"),
            Some(ConstantValue::EnumValue(..))
        ));
        assert_ne!(
            evaluate_named(programs[1], &model, &signatures, "otherMode"),
            Some(mode)
        );
        let owner = model
            .resolve_name(QualifiedName::prefixed("origin", "Foo"))
            .unwrap();
        let class = programs[0]
            .declarations()
            .find_map(|declaration| {
                let TopLevelDecl::Class(class) = declaration else {
                    return None;
                };
                (class.name().name() == "Foo").then_some(class)
            })
            .unwrap();
        let fields: HashMap<_, _> = class
            .members()
            .filter_map(|member| {
                let ClassMember::Field(field) = member else {
                    return None;
                };
                Some(field.declarators().filter_map(|declarator| {
                    Some((declarator.name().name(), declarator.initializer()?))
                }))
            })
            .flatten()
            .collect();
        for name in ["a", "c"] {
            assert_eq!(
                evaluate_constant(
                    fields[name],
                    ConstantContext {
                        owner: Some(&owner),
                        fields: Some(&fields),
                        ..ConstantContext::new(&model, &signatures)
                    }
                ),
                Some(ConstantValue::Number(Number::Int(2)))
            );
        }
        assert!(matches!(
            evaluate_constant(
                fields["boxed"],
                ConstantContext {
                    owner: Some(&owner),
                    fields: Some(&fields),
                    ..ConstantContext::new(&model, &signatures)
                }
            ),
            Some(ConstantValue::Instance(..))
        ));
    }

    #[test]
    fn numeric_identity_preserves_double_but_operator_equality_is_numeric() {
        with_constants(|program, model, signatures| {
            let value = |name| evaluate_named(program, model, signatures, name);
            assert_ne!(value("intOne"), value("doubleOne"));
            assert_eq!(
                value("doubleOne"),
                Some(ConstantValue::Number(Number::Float(1.0_f64.to_bits())))
            );
            assert_eq!(
                value("doubleSum"),
                Some(ConstantValue::Number(Number::Float(2.0_f64.to_bits())))
            );
            assert_eq!(value("doubleDivision"), value("doubleOne"));
            assert_eq!(value("numericEquality"), Some(ConstantValue::Bool(true)));
        });
    }

    #[test]
    fn boolean_bits_tearoffs_and_nonprimitive_equality() {
        with_constants(|program, model, signatures| {
            let value = |name| evaluate_named(program, model, signatures, name);
            assert_eq!(value("boolBits"), Some(ConstantValue::Bool(true)));
            assert_eq!(
                value("constructorEquality"),
                Some(ConstantValue::Bool(true))
            );
            assert_eq!(value("customEquality"), None);
            assert_eq!(value("invalidShadeCast"), None);
            assert_eq!(
                value("implicitConstructorEquality"),
                Some(ConstantValue::Bool(true))
            );
            assert_eq!(
                value("aliasConstructorEquality"),
                Some(ConstantValue::Bool(true))
            );
            assert_eq!(value("aliasConstructor"), value("implicitConstructor"));
            assert_eq!(
                value("constructorNameValue"),
                Some(ConstantValue::Number(Number::Int(1)))
            );
            let Some(ConstantValue::TearOff(_, _, ConstantType::Function(result, ..))) =
                value("implicitConstructor")
            else {
                panic!("missing implicit constructor tear-off");
            };
            assert_eq!(
                *result,
                ConstantType::Interface(
                    model.resolve_dotted(&["ImplicitConstructor"]).unwrap(),
                    Vec::new(),
                    false,
                ),
            );
            let Some(ConstantValue::TearOff(_, _, ConstantType::Function(result, ..))) =
                value("unannotatedFunction")
            else {
                panic!("missing unannotated function tear-off");
            };
            assert_eq!(*result, ConstantType::Dynamic);
            let Some(ConstantValue::List(arguments, _)) = value("methodNameList") else {
                panic!("missing static method list");
            };
            assert!(matches!(arguments.as_slice(), [ConstantType::Function(..)]));
        });
    }

    #[test]
    fn explicitly_typed_constants_preserve_resolved_type_arguments() {
        with_constants(|program, model, signatures| {
            let value = |name| evaluate_named(program, model, signatures, name);
            for (left, right) in [
                ("intList", "numList"),
                ("intSet", "numSet"),
                ("intMap", "numMap"),
                ("implicitConst", "numBox"),
            ] {
                assert!(value(left).is_some(), "{left}");
                assert!(value(right).is_some(), "{right}");
                assert_ne!(value(left), value(right), "{left} versus {right}");
            }
            assert_eq!(value("intList"), value("intListAgain"));
            assert_eq!(value("explicitConst"), value("implicitConst"));
        });
    }

    #[test]
    fn generic_constructor_contexts_substitute_owner_parameters() {
        with_constants(|program, model, signatures| {
            let int_type = constant_type(&model.core_type("int", false)).unwrap();
            let double_type = constant_type(&model.core_type("double", false)).unwrap();
            for (name, ty, values) in [
                (
                    "genericList",
                    int_type.clone(),
                    vec![ConstantValue::Number(Number::Int(1))],
                ),
                ("explicitList", int_type.clone(), vec![]),
                ("emptyList", int_type.clone(), vec![]),
                ("initializedList", int_type.clone(), vec![]),
                ("redirectedList", int_type.clone(), vec![]),
                ("inheritedList", int_type, vec![]),
                (
                    "namedList",
                    double_type,
                    vec![ConstantValue::Number(Number::from_float(1.0))],
                ),
            ] {
                let value = evaluate_named(program, model, signatures, name);
                assert!(
                    matches!(value, Some(ConstantValue::Instance(_, _, ref fields))
                    if fields.iter().any(|(_, _, value)|
                        value == &ConstantValue::List(vec![ty.clone()], values.clone()))),
                    "{name}: {value:?}"
                );
            }
            for (name, expected) in [
                ("doubleBox", Number::from_float(1.0)),
                ("integerBox", Number::Int(1)),
            ] {
                assert!(
                    matches!(evaluate_named(program, model, signatures, name),
                    Some(ConstantValue::Instance(_, _, fields))
                    if fields.iter().any(|(_, _, value)| value == &ConstantValue::Number(expected))),
                    "{name}"
                );
            }
        });
    }

    #[test]
    fn collection_inference_uses_lexical_values_and_unknown_shadowing() {
        with_constants(|program, model, signatures| {
            let expression = |name| {
                program
                    .declarations()
                    .find_map(|declaration| {
                        let TopLevelDecl::Variable(variable) = declaration else {
                            return None;
                        };
                        variable
                            .declarators()
                            .find(|declarator| declarator.name().name() == name)
                            .and_then(|declarator| declarator.initializer())
                    })
                    .unwrap()
            };
            let local = Some(ConstantValue::String("local".to_owned()));
            let locals = HashMap::from([("a", (&local, model.core_type("String", false)))]);
            assert_eq!(
                evaluate_constant(
                    expression("capturedList"),
                    ConstantContext {
                        locals: Some(&locals),
                        ..ConstantContext::new(model, signatures)
                    },
                ),
                Some(ConstantValue::List(
                    vec![constant_type(&model.core_type("String", false)).unwrap()],
                    vec![ConstantValue::String("local".to_owned())],
                )),
            );
            assert_eq!(
                evaluate_constant(
                    expression("capturedList"),
                    ConstantContext {
                        locals: Some(&locals),
                        stable: true,
                        ..ConstantContext::new(model, signatures)
                    },
                ),
                None,
            );
            let unknown = None;
            assert_eq!(
                evaluate_constant(
                    expression("capturedList"),
                    ConstantContext {
                        locals: Some(&HashMap::from([("a", (&unknown, ResolvedType::Unknown))])),
                        ..ConstantContext::new(model, signatures)
                    },
                ),
                None,
            );
            let fields = HashMap::new();
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            evaluator.parameters.push(HashMap::from([(
                "v".to_owned(),
                Some(ConstantValue::String("parameter".to_owned())),
            )]));
            let parameter_value = evaluator.evaluate(expression("parameterList"));
            assert!(
                matches!(parameter_value,
                Some(ConstantValue::Instance(_, _, ref fields))
                    if fields.iter().any(|(_, _, value)| matches!(value,
                        ConstantValue::List(types, values)
                        if types == &vec![constant_type(&model.core_type("String", false)).unwrap()]
                        && values == &vec![ConstantValue::String("parameter".to_owned())]))),
                "{parameter_value:?}"
            );
            assert_eq!(
                evaluator.resolve_name(&["ConstantScope", "inferred"]),
                Some(ConstantValue::List(
                    vec![constant_type(&model.core_type("int", false)).unwrap()],
                    vec![ConstantValue::Number(Number::Int(1))],
                ))
            );
        });
    }

    #[test]
    fn collection_and_constructor_inference_preserve_static_constant_types() {
        with_constants(|program, model, signatures| {
            let num_type = constant_type(&model.core_type("num", false)).unwrap();
            let nullable_int = constant_type(&model.core_type("int", true)).unwrap();
            let fields = HashMap::new();
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            let num_values = Some(ConstantValue::List(
                vec![num_type.clone()],
                vec![ConstantValue::Number(Number::Int(1))],
            ));
            assert_eq!(
                evaluate_named(program, model, signatures, "numValues"),
                num_values
            );
            assert_eq!(evaluator.resolve_name(&["NumScope", "values"]), num_values);
            assert_eq!(
                evaluate_named(program, model, signatures, "nullableValues"),
                Some(ConstantValue::List(
                    vec![nullable_int],
                    vec![ConstantValue::Number(Number::Int(1))]
                ))
            );
            assert!(
                matches!(evaluate_named(program, model, signatures, "numInstance"),
                Some(ConstantValue::Instance(_, types, _)) if types == vec![num_type.clone()])
            );
            let parameter_box = evaluate_named(program, model, signatures, "parameterBox");
            assert!(
                matches!(parameter_box,
                Some(ConstantValue::Instance(_, _, ref fields))
                    if fields.iter().any(|(_, _, value)| matches!(value,
                        ConstantValue::Instance(_, types, _) if types == &vec![num_type.clone()]))),
                "{parameter_box:?}"
            );
            let local_value = Some(ConstantValue::Number(Number::Int(1)));
            let locals =
                HashMap::from([("declaredNum", (&local_value, model.core_type("num", false)))]);
            let expression = program
                .declarations()
                .find_map(|declaration| {
                    let TopLevelDecl::Variable(variable) = declaration else {
                        return None;
                    };
                    variable
                        .declarators()
                        .find(|declarator| declarator.name().name() == "numValues")
                        .and_then(|declarator| declarator.initializer())
                })
                .unwrap();
            assert_eq!(
                evaluate_constant(
                    expression,
                    ConstantContext {
                        locals: Some(&locals),
                        ..ConstantContext::new(model, signatures)
                    }
                ),
                num_values
            );
        });
    }

    #[test]
    fn declared_collection_contexts_preserve_double_identity() {
        with_constants(|program, model, signatures| {
            let fields = HashMap::new();
            let mut evaluator = ConstEvaluator::new(None, &fields, model, signatures);
            for (name, member, number) in [
                ("declaredUnit", "unit", Number::from_float(1.0)),
                ("declaredNegative", "negative", Number::from_float(-0.0)),
            ] {
                let expected = Some(ConstantValue::List(
                    vec![constant_type(&model.core_type("double", false)).unwrap()],
                    vec![ConstantValue::Number(number)],
                ));
                assert_eq!(evaluator.resolve_name(&[name]), expected, "{name}");
                assert_eq!(
                    evaluator.resolve_name(&["ConstantScope", member]),
                    expected,
                    "{member}"
                );
            }
            let TopLevelDecl::Variable(variable) = program.declarations()
                .find(|declaration| matches!(declaration, TopLevelDecl::Variable(variable)
                    if variable.declarators().any(|declarator| declarator.name().name() == "declaredUnit")))
                .unwrap() else { panic!() };
            let identity = model.resolve_value("declaredUnit").unwrap();
            let expected = signatures.value_type(&identity).unwrap();
            assert_eq!(
                evaluate_constant(
                    variable
                        .declarators()
                        .next()
                        .unwrap()
                        .initializer()
                        .unwrap(),
                    ConstantContext {
                        expected: Some(expected),
                        ..ConstantContext::new(model, signatures)
                    },
                ),
                evaluator.resolve_name(&["declaredUnit"]),
            );
        });
    }

    #[test]
    fn constant_sets_preserve_order_and_operations_follow_dart_semantics() {
        with_constants(|program, model, signatures| {
            let evaluate = |name| evaluate_named(program, model, signatures, name);
            assert_ne!(evaluate("orderedSet"), evaluate("reorderedSet"));
            for name in ["negativeModulo", "negativeDivisorModulo"] {
                assert_eq!(evaluate(name), Some(ConstantValue::Number(Number::Int(2))));
            }
            assert_eq!(
                evaluate("negativeDoubleModulo"),
                Some(ConstantValue::Number(Number::from_float(0.5)))
            );
            for name in ["negativeZeroModulo", "signedZeroModulo"] {
                assert_eq!(
                    evaluate(name),
                    Some(ConstantValue::Number(Number::from_float(0.0)))
                );
            }
            assert_eq!(evaluate("shortAnd"), Some(ConstantValue::Bool(false)));
            assert_eq!(evaluate("shortOr"), Some(ConstantValue::Bool(true)));
        });
    }

    #[test]
    fn string_lengths_wrapping_integers_and_type_tests_fold() {
        with_constants(|program, model, signatures| {
            for (name, expected) in [
                ("namedTextLength", 1),
                ("utf16TextLength", 2),
                ("shiftedBeyondWord", 0),
                ("shiftedSignBit", i128::from(i64::MIN)),
                ("negatedSignBit", i128::from(i64::MIN)),
                ("overflowSum", i128::from(i64::MIN)),
                ("overflowDifference", i128::from(i64::MAX)),
                ("overflowProduct", -2),
                ("shiftedNegativeBeyondWord", -1),
                ("shiftedUnsignedBeyondWord", 0),
                ("typeTestBranch", 2),
                ("negatedTypeTestBranch", 1),
                ("positiveTypeTestBranch", 2),
            ] {
                assert_eq!(
                    evaluate_named(program, model, signatures, name),
                    Some(ConstantValue::Number(Number::Int(expected))),
                    "{name}",
                );
            }
        });
    }

    #[test]
    fn unsigned_shift_uses_a_zero_filling_word() {
        let left = ConstantValue::Number(Number::Int(-1));
        let shift = ConstantValue::Number(Number::Int(1));
        assert_eq!(
            eval_binary(&BinaryOp::Shr, left.clone(), shift.clone()),
            Some(ConstantValue::Number(Number::Int(-1)))
        );
        assert_eq!(
            eval_binary(&BinaryOp::UShr, left.clone(), shift),
            Some(ConstantValue::Number(Number::Int(i128::from(i64::MAX))))
        );
        assert_eq!(
            eval_binary(&BinaryOp::UShr, left, ConstantValue::Number(Number::Int(0))),
            Some(ConstantValue::Number(Number::Int(-1)))
        );
    }
}
