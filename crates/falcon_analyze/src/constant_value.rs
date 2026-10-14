//! Conservative Dart constant evaluation used by semantic lint rules.

use std::collections::HashMap;

use falcon_syntax::ast::*;

use crate::resolve::{expression_segments, resolve_call_constructor};
use crate::{DeclarationIdentity, QualifiedName, ResolvedType, SemanticModel, SignatureIndex};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConstantValue {
    Null,
    Bool(bool),
    /// A numeric value. Models value equality only, not Dart's static or
    /// runtime numeric type: `1.0` folds to the same `Number::Int(1)` as `1`.
    Number(Number),
    String(String),
    List(Vec<ConstantValue>),
    Set(Vec<ConstantValue>),
    Map(Vec<(ConstantValue, ConstantValue)>),
    Record(Vec<(Option<String>, ConstantValue)>),
    Instance(String, Vec<ConstantValue>, Vec<(String, ConstantValue)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Number {
    Int(i128),
    Float(u64),
}

impl Number {
    /// Integral floats collapse to [`Number::Int`] so `2.0 == 2` compares equal;
    /// the double-ness of the source expression is deliberately not preserved.
    fn from_float(value: f64) -> Self {
        if value.is_finite()
            && value.fract() == 0.0
            && value >= i128::MIN as f64
            && value <= i128::MAX as f64
        {
            Self::Int(value as i128)
        } else {
            Self::Float(value.to_bits())
        }
    }

    fn float(self) -> f64 {
        match self {
            Self::Int(value) => value as f64,
            Self::Float(bits) => f64::from_bits(bits),
        }
    }
}

/// Evaluates an expression from the owner's declaring library, using `model` when the owner has no project file.
pub fn evaluate_constant(
    expression: &Expr,
    owner: Option<&DeclarationIdentity>,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Option<ConstantValue> {
    ConstEvaluator::new(owner, fields, model, signatures).evaluate(expression)
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ConstantKey<'a> {
    Field(&'a str),
    TopLevel(DeclarationIdentity),
}

enum ConstantState {
    Evaluating,
    Evaluated(Option<ConstantValue>),
}

struct ConstEvaluator<'a, 'model> {
    owner: Option<&'a DeclarationIdentity>,
    fields: &'a HashMap<&'a str, &'a Expr>,
    owner_model: SemanticModel<'model>,
    model: SemanticModel<'model>,
    unqualified_fields: bool,
    signatures: &'a SignatureIndex,
    constants: HashMap<ConstantKey<'a>, ConstantState>,
}

impl<'a, 'model> ConstEvaluator<'a, 'model> {
    fn new(
        owner: Option<&'a DeclarationIdentity>,
        fields: &'a HashMap<&'a str, &'a Expr>,
        model: &SemanticModel<'model>,
        signatures: &'a SignatureIndex,
    ) -> Self {
        let model = owner
            .and_then(|owner| model.for_declaration(owner))
            .unwrap_or(*model);
        Self {
            owner,
            fields,
            owner_model: model,
            model,
            unqualified_fields: true,
            signatures,
            constants: HashMap::new(),
        }
    }

    fn evaluate(&mut self, expression: &Expr) -> Option<ConstantValue> {
        match expression {
            Expr::IntLit { value, .. } => {
                parse_int(value).map(|value| ConstantValue::Number(Number::Int(value)))
            }
            Expr::DoubleLit { value, .. } => value
                .replace('_', "")
                .parse::<f64>()
                .ok()
                .map(Number::from_float)
                .map(ConstantValue::Number),
            Expr::BoolLit { value, .. } => Some(ConstantValue::Bool(*value)),
            Expr::NullLit { .. } => Some(ConstantValue::Null),
            Expr::StringLit(value) => decode_string(value).map(ConstantValue::String),
            Expr::Ident(identifier) => self.resolve_name(&[&identifier.name]),
            Expr::Field { .. } => self.resolve_name(&expression_segments(expression, true)?),
            Expr::Unary { op, operand, .. } => {
                let value = self.evaluate(operand)?;
                eval_unary(op, value)
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let left = self.evaluate(left)?;
                if matches!(op, BinaryOp::NullCoalesce) {
                    return if left == ConstantValue::Null {
                        self.evaluate(right)
                    } else {
                        Some(left)
                    };
                }
                let right = self.evaluate(right)?;
                eval_binary(op, left, right)
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => match self.evaluate(condition)? {
                ConstantValue::Bool(true) => self.evaluate(then_expr),
                ConstantValue::Bool(false) => self.evaluate(else_expr),
                _ => None,
            },
            Expr::New {
                is_const,
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                if !is_const {
                    return None;
                }
                let identity = match self.model.resolve_type(dart_type) {
                    ResolvedType::Interface { identity, .. } => identity,
                    _ => return None,
                };
                let constructor = constructor_name
                    .as_ref()
                    .map_or("new", |name| name.name.as_str());
                self.call(
                    const_constructor_identity(identity, constructor, self.signatures)?,
                    args,
                )
            }
            Expr::Call { callee, args, .. } => {
                let (identity, constructor) = resolve_call_constructor(callee, &self.model, true)?;
                self.call(
                    const_constructor_identity(identity, constructor, self.signatures)?,
                    args,
                )
            }
            Expr::List { elements, .. } => self.collection(elements).map(ConstantValue::List),
            Expr::Set { elements, .. } => {
                let mut values = self.collection(elements)?;
                values.sort();
                values.dedup();
                Some(ConstantValue::Set(values))
            }
            Expr::Map { elements, .. } => {
                let mut values = elements
                    .iter()
                    .map(|element| {
                        let MapElement::Entry(entry) = element else {
                            return None;
                        };
                        Some((self.evaluate(&entry.key)?, self.evaluate(&entry.value)?))
                    })
                    .collect::<Option<Vec<_>>>()?;
                values.sort();
                Some(ConstantValue::Map(values))
            }
            Expr::Record { fields: values, .. } => {
                let mut values = values
                    .iter()
                    .map(|field| {
                        Some((
                            field.name.as_ref().map(|name| name.name.clone()),
                            self.evaluate(&field.value)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                let positional = values.iter().take_while(|(name, _)| name.is_none()).count();
                values[positional..].sort();
                Some(ConstantValue::Record(values))
            }
            Expr::NullAssert { operand, .. } => {
                let value = self.evaluate(operand)?;
                (value != ConstantValue::Null).then_some(value)
            }
            Expr::This { .. }
            | Expr::Super { .. }
            | Expr::PostfixIncDec { .. }
            | Expr::Assign { .. }
            | Expr::Is { .. }
            | Expr::As { .. }
            | Expr::Index { .. }
            | Expr::Cascade { .. }
            | Expr::FuncExpr { .. }
            | Expr::DotShorthand { .. }
            | Expr::Await { .. }
            | Expr::Throw { .. }
            | Expr::Switch { .. }
            | Expr::SymbolLit { .. }
            | Expr::GenericInstantiation { .. }
            | Expr::Error { .. } => None,
        }
    }

    fn resolve_name(&mut self, names: &[&str]) -> Option<ConstantValue> {
        let local_name = names.last()?;
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
        if local_owner && let Some((&name, &expression)) = self.fields.get_key_value(*local_name) {
            return self.initializer(ConstantKey::Field(name), expression, self.owner_model);
        }
        let identity = self
            .model
            .resolve_value(QualifiedName::from_segments(names)?)?;
        let expression = self.signatures.constant_initializer(&identity)?;
        let model = self.model.for_declaration(&identity)?;
        self.initializer(ConstantKey::TopLevel(identity), expression, model)
    }

    fn initializer(
        &mut self,
        key: ConstantKey<'a>,
        expression: &Expr,
        model: SemanticModel<'model>,
    ) -> Option<ConstantValue> {
        if let Some(state) = self.constants.get(&key) {
            return match state {
                ConstantState::Evaluating => None,
                ConstantState::Evaluated(value) => value.clone(),
            };
        }
        self.constants
            .insert(key.clone(), ConstantState::Evaluating);
        let previous_model = std::mem::replace(&mut self.model, model);
        let previous_fields = std::mem::replace(
            &mut self.unqualified_fields,
            matches!(key, ConstantKey::Field(_)),
        );
        let value = self.evaluate(expression);
        self.model = previous_model;
        self.unqualified_fields = previous_fields;
        self.constants
            .insert(key, ConstantState::Evaluated(value.clone()));
        value
    }

    fn collection(&mut self, elements: &[CollectionElement]) -> Option<Vec<ConstantValue>> {
        elements
            .iter()
            .map(|element| match element {
                CollectionElement::Expr(expression) => self.evaluate(expression),
                _ => None,
            })
            .collect()
    }

    fn call(&mut self, callee: String, args: &ArgList) -> Option<ConstantValue> {
        let positional = args
            .positional
            .iter()
            .map(|argument| self.evaluate(argument))
            .collect::<Option<Vec<_>>>()?;
        let mut named = args
            .named
            .iter()
            .map(|argument| {
                self.evaluate(&argument.value)
                    .map(|value| (argument.name.name.clone(), value))
            })
            .collect::<Option<Vec<_>>>()?;
        named.sort();
        Some(ConstantValue::Instance(callee, positional, named))
    }
}

fn decode_string(node: &StringLitNode) -> Option<String> {
    if !node.interpolations.is_empty() {
        return None;
    }
    let literal = node.delimiter()?;
    let quote_len = if literal.triple { 3 } else { 1 };
    let content_end = node.raw.len().checked_sub(quote_len)?;
    let content = node.raw.get(literal.content_offset..content_end)?;
    if literal.is_raw {
        return Some(content.to_string());
    }
    let mut output = String::with_capacity(content.len());
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
                let digits = [chars.next()?, chars.next()?];
                let value = u32::from_str_radix(&digits.iter().collect::<String>(), 16).ok()?;
                output.push(char::from_u32(value)?);
            }
            'u' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut digits = String::new();
                loop {
                    let next = chars.next()?;
                    if next == '}' {
                        break;
                    }
                    digits.push(next);
                }
                output.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
            }
            'u' => {
                let digits = (0..4).map(|_| chars.next()).collect::<Option<String>>()?;
                output.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
            }
            other => output.push(other),
        }
    }
    Some(output)
}

fn const_constructor_identity(
    identity: DeclarationIdentity,
    constructor: &str,
    signatures: &SignatureIndex,
) -> Option<String> {
    if !signatures.is_const_constructor(&identity, constructor) {
        return None;
    }
    Some(format!("{identity:?}::{constructor}"))
}

fn eval_unary(op: &UnaryOp, value: ConstantValue) -> Option<ConstantValue> {
    match (op, value) {
        (UnaryOp::Minus, ConstantValue::Number(Number::Int(value))) => value
            .checked_neg()
            .map(Number::Int)
            .map(ConstantValue::Number),
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

fn eval_binary(op: &BinaryOp, left: ConstantValue, right: ConstantValue) -> Option<ConstantValue> {
    match op {
        BinaryOp::EqEq => return Some(ConstantValue::Bool(left == right)),
        BinaryOp::NotEq => return Some(ConstantValue::Bool(left != right)),
        BinaryOp::And => return bool_pair(left, right).map(|(l, r)| ConstantValue::Bool(l && r)),
        BinaryOp::Or => return bool_pair(left, right).map(|(l, r)| ConstantValue::Bool(l || r)),
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
    // Dart's `int` is 64-bit signed. Operands or results outside that range are
    // not values Dart would produce, so refuse to fold rather than guess.
    if let (Number::Int(left), Number::Int(right)) = (left, right)
        && (i64::try_from(left).is_err() || i64::try_from(right).is_err())
    {
        return None;
    }
    let integer = |value: i128| {
        i64::try_from(value)
            .ok()
            .map(|value| ConstantValue::Number(Number::Int(i128::from(value))))
    };
    match (op, left, right) {
        (BinaryOp::Add, Number::Int(left), Number::Int(right)) => integer(left.checked_add(right)?),
        (BinaryOp::Sub, Number::Int(left), Number::Int(right)) => integer(left.checked_sub(right)?),
        (BinaryOp::Mul, Number::Int(left), Number::Int(right)) => integer(left.checked_mul(right)?),
        (BinaryOp::IntDiv, Number::Int(left), Number::Int(right)) => {
            integer(left.checked_div(right)?)
        }
        (BinaryOp::Mod, Number::Int(left), Number::Int(right)) => integer(left.checked_rem(right)?),
        (BinaryOp::BitAnd, Number::Int(left), Number::Int(right)) => integer(left & right),
        (BinaryOp::BitOr, Number::Int(left), Number::Int(right)) => integer(left | right),
        (BinaryOp::BitXor, Number::Int(left), Number::Int(right)) => integer(left ^ right),
        (BinaryOp::Shl, Number::Int(left), Number::Int(right)) => {
            integer(left.checked_shl(right.try_into().ok()?)?)
        }
        (BinaryOp::Shr, Number::Int(left), Number::Int(right)) => {
            integer(left.checked_shr(right.try_into().ok()?)?)
        }
        (BinaryOp::UShr, Number::Int(left), Number::Int(right)) => {
            let shift = u32::try_from(right).ok()?;
            if shift == 0 {
                return integer(left);
            }
            let left = u64::from_ne_bytes(i64::try_from(left).ok()?.to_ne_bytes());
            integer(if shift >= u64::BITS {
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
        (BinaryOp::Mod, left, right) => Some(ConstantValue::Number(Number::from_float(
            left.float() % right.float(),
        ))),
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

    use falcon_dart_parser::parse;

    use super::*;
    use crate::{IdentityIndex, IdentitySource, TypeIndex};

    #[test]
    fn string_decoding_preserves_recovery_and_rejects_adjacent_literals() {
        for (raw, expected) in [
            ("'hello", Some("hell")),
            ("'a'\"b\"", None),
            ("r'\\n'", Some("\\n")),
            ("'\\n'", Some("\n")),
            ("'", None),
        ] {
            let node = StringLitNode {
                raw: raw.to_string(),
                is_adjacent: raw == "'a'\"b\"",
                segment_spans: Vec::new(),
                has_raw_segment: raw.starts_with('r'),
                value: String::new(),
                span: Span::default(),
                interpolations: Vec::new(),
            };
            assert_eq!(decode_string(&node).as_deref(), expected, "{raw}");
        }
    }

    fn with_constants(test: impl FnOnce(&Program, &SemanticModel<'_>, &SignatureIndex)) {
        let source = r#"
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
"#;
        let path = PathBuf::from("/project/lib/main.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let sources = [IdentitySource {
            path: &path,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        test(&program, &model, &signatures);
    }

    fn evaluate_named(
        program: &Program,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        name: &str,
    ) -> Option<ConstantValue> {
        let initializer = program.declarations.iter().find_map(|declaration| {
            let TopLevelDecl::Variable(variable) = declaration else {
                return None;
            };
            variable
                .declarators
                .iter()
                .find(|declarator| declarator.name.name == name)
                .and_then(|declarator| declarator.initializer.as_ref())
        })?;
        let owner = model.resolve_value(name)?;
        evaluate_constant(
            initializer,
            Some(&owner),
            &HashMap::new(),
            model,
            signatures,
        )
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
    fn const_generic_constructor_forms_remain_evaluable() {
        with_constants(|program, model, signatures| {
            for name in ["explicitConst", "implicitConst"] {
                assert!(matches!(
                    evaluate_named(program, model, signatures, name),
                    Some(ConstantValue::Instance(_, values, _))
                        if values == vec![ConstantValue::Number(Number::Int(1))]
                ));
            }
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
                .declarations
                .iter()
                .find_map(|declaration| {
                    let TopLevelDecl::Class(class) = declaration else {
                        return None;
                    };
                    if class.name.name != "Foo" {
                        return None;
                    }
                    class.members.iter().find_map(|member| {
                        let ClassMember::Field(field) = member else {
                            return None;
                        };
                        field.declarators.first()?.initializer.as_ref()
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
    fn imported_constants_resolve_private_names_in_their_declaring_library() {
        let inputs = [
            (
                PathBuf::from("/project/lib/origin.dart"),
                "import 'box.dart' as box; const _y = 2; const x = _y; class Foo { static const a = _y; static const boxed = box.ConstBox(_y); static const c = qualified; } const qualified = Foo.a;",
            ),
            (
                PathBuf::from("/project/lib/main.dart"),
                "import 'origin.dart' as origin; const _y = 1; const value = origin.x;",
            ),
            (
                PathBuf::from("/project/lib/box.dart"),
                "class ConstBox { final int value; const ConstBox(this.value); }",
            ),
        ];
        let programs: Vec<_> = inputs
            .iter()
            .map(|(_, source)| {
                let (program, errors) = parse(source);
                assert!(errors.is_empty(), "{errors:?}");
                program
            })
            .collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&programs)
            .map(|((path, _), program)| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(&programs[0]);
        let files: Vec<_> = inputs
            .iter()
            .zip(&programs)
            .map(|((path, _), program)| (path.clone(), program))
            .collect();
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&inputs[1].0, &identities, Some(&types));
        assert_eq!(
            evaluate_named(&programs[1], &model, &signatures, "value"),
            Some(ConstantValue::Number(Number::Int(2)))
        );
        let owner = model
            .resolve_name(QualifiedName::prefixed("origin", "Foo"))
            .unwrap();
        let class = programs[0]
            .declarations
            .iter()
            .find_map(|declaration| {
                let TopLevelDecl::Class(class) = declaration else {
                    return None;
                };
                (class.name.name == "Foo").then_some(class)
            })
            .unwrap();
        let fields: HashMap<_, _> = class
            .members
            .iter()
            .filter_map(|member| {
                let ClassMember::Field(field) = member else {
                    return None;
                };
                Some(field.declarators.iter().filter_map(|declarator| {
                    Some((
                        declarator.name.name.as_str(),
                        declarator.initializer.as_ref()?,
                    ))
                }))
            })
            .flatten()
            .collect();
        for name in ["a", "c"] {
            assert_eq!(
                evaluate_constant(fields[name], Some(&owner), &fields, &model, &signatures),
                Some(ConstantValue::Number(Number::Int(2)))
            );
        }
        assert!(matches!(
            evaluate_constant(fields["boxed"], Some(&owner), &fields, &model, &signatures),
            Some(ConstantValue::Instance(..))
        ));
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
