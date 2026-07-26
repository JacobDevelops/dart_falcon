//! Conservative Dart constant evaluation used by semantic lint rules.

use std::collections::{HashMap, HashSet};

use falcon_syntax::ast::*;

use crate::{DeclarationIdentity, ResolvedType, SemanticModel, SignatureIndex};

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

pub fn evaluate_constant(
    expression: &Expr,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Option<ConstantValue> {
    constant_value(
        expression,
        owner,
        fields,
        model,
        signatures,
        &mut HashSet::new(),
    )
}

fn constant_value(
    expression: &Expr,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    visiting: &mut HashSet<String>,
) -> Option<ConstantValue> {
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
        Expr::Ident(identifier) => resolve_constant_name(
            std::slice::from_ref(&identifier.name),
            owner,
            fields,
            model,
            signatures,
            visiting,
        ),
        Expr::Field { .. } => resolve_constant_name(
            &expression_name(expression)?,
            owner,
            fields,
            model,
            signatures,
            visiting,
        ),
        Expr::Unary { op, operand, .. } => {
            let value = constant_value(operand, owner, fields, model, signatures, visiting)?;
            eval_unary(op, value)
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            let left = constant_value(left, owner, fields, model, signatures, visiting)?;
            if matches!(op, BinaryOp::NullCoalesce | BinaryOp::IfNull) {
                return if left == ConstantValue::Null {
                    constant_value(right, owner, fields, model, signatures, visiting)
                } else {
                    Some(left)
                };
            }
            let right = constant_value(right, owner, fields, model, signatures, visiting)?;
            eval_binary(op, left, right)
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => match constant_value(condition, owner, fields, model, signatures, visiting)? {
            ConstantValue::Bool(true) => {
                constant_value(then_expr, owner, fields, model, signatures, visiting)
            }
            ConstantValue::Bool(false) => {
                constant_value(else_expr, owner, fields, model, signatures, visiting)
            }
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
            let identity = match model.resolve_type(dart_type) {
                ResolvedType::Interface { identity, .. } => identity,
                _ => return None,
            };
            let constructor = constructor_name
                .as_ref()
                .map_or("new", |name| name.name.as_str());
            constant_call(
                const_constructor_identity(identity, constructor, signatures)?,
                args,
                owner,
                fields,
                model,
                signatures,
                visiting,
            )
        }
        Expr::Call { callee, args, .. } => {
            let (identity, constructor) = resolve_call_constructor(callee, model)?;
            constant_call(
                const_constructor_identity(identity, &constructor, signatures)?,
                args,
                owner,
                fields,
                model,
                signatures,
                visiting,
            )
        }
        Expr::List { elements, .. } => {
            plain_collection(elements, owner, fields, model, signatures, visiting)
                .map(ConstantValue::List)
        }
        Expr::Set { elements, .. } => {
            let mut values =
                plain_collection(elements, owner, fields, model, signatures, visiting)?;
            values.sort();
            values.dedup();
            Some(ConstantValue::Set(values))
        }
        Expr::Map {
            entries, elements, ..
        } if elements.is_empty() => {
            let mut values = entries
                .iter()
                .map(|entry| {
                    Some((
                        constant_value(&entry.key, owner, fields, model, signatures, visiting)?,
                        constant_value(&entry.value, owner, fields, model, signatures, visiting)?,
                    ))
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
                        constant_value(&field.value, owner, fields, model, signatures, visiting)?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            let positional = values.iter().take_while(|(name, _)| name.is_none()).count();
            values[positional..].sort();
            Some(ConstantValue::Record(values))
        }
        Expr::NullAssert { operand, .. } => {
            let value = constant_value(operand, owner, fields, model, signatures, visiting)?;
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
        | Expr::Error { .. }
        | Expr::Map { .. } => None,
    }
}

fn decode_string(node: &StringLitNode) -> Option<String> {
    if !node.interpolations.is_empty() {
        return None;
    }
    let mut raw = node.raw.as_str();
    let is_raw = raw.starts_with('r');
    if is_raw {
        raw = &raw[1..];
    }
    let quote_len = if raw.starts_with("'''") || raw.starts_with("\"\"\"") {
        3
    } else {
        1
    };
    if raw.len() < quote_len * 2 {
        return None;
    }
    let content = &raw[quote_len..raw.len() - quote_len];
    if is_raw {
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

fn resolve_constant_name(
    names: &[String],
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    visiting: &mut HashSet<String>,
) -> Option<ConstantValue> {
    let local_name = names.last()?;
    let local_owner = if names.len() == 1 {
        true
    } else {
        model.resolve_name(&names[..names.len() - 1]).as_ref() == Some(owner)
    };
    if local_owner && let Some(expression) = fields.get(local_name.as_str()) {
        let key = format!("field:{local_name}");
        if !visiting.insert(key.clone()) {
            return None;
        }
        let value = constant_value(expression, owner, fields, model, signatures, visiting);
        visiting.remove(&key);
        return value;
    }
    let identity = model.resolve_value(names)?;
    let key = format!("top:{identity:?}");
    if !visiting.insert(key.clone()) {
        return None;
    }
    let value = constant_value(
        signatures.constant_initializer(&identity)?,
        owner,
        fields,
        model,
        signatures,
        visiting,
    );
    visiting.remove(&key);
    value
}

fn plain_collection(
    elements: &[CollectionElement],
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    visiting: &mut HashSet<String>,
) -> Option<Vec<ConstantValue>> {
    elements
        .iter()
        .map(|element| match element {
            CollectionElement::Expr(expression) => {
                constant_value(expression, owner, fields, model, signatures, visiting)
            }
            _ => None,
        })
        .collect()
}

fn resolve_call_constructor(
    callee: &Expr,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, String)> {
    let mut segments = expression_name(callee)?;
    if let Some(identity) = model.resolve_name(&segments) {
        return Some((identity, "new".to_string()));
    }
    let constructor = segments.pop()?;
    model
        .resolve_name(&segments)
        .map(|identity| (identity, constructor))
}

fn const_constructor_identity(
    identity: DeclarationIdentity,
    constructor: &str,
    signatures: &SignatureIndex,
) -> Option<String> {
    signatures
        .declaration(&identity)?
        .constructors
        .iter()
        .find(|facts| facts.name == constructor && facts.is_const)?;
    Some(format!("{identity:?}::{constructor}"))
}

fn constant_call(
    callee: String,
    args: &ArgList,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    visiting: &mut HashSet<String>,
) -> Option<ConstantValue> {
    let positional = args
        .positional
        .iter()
        .map(|argument| constant_value(argument, owner, fields, model, signatures, visiting))
        .collect::<Option<Vec<_>>>()?;
    let mut named = args
        .named
        .iter()
        .map(|argument| {
            constant_value(&argument.value, owner, fields, model, signatures, visiting)
                .map(|value| (argument.name.name.clone(), value))
        })
        .collect::<Option<Vec<_>>>()?;
    named.sort();
    Some(ConstantValue::Instance(callee, positional, named))
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

fn parse_int(value: &str) -> Option<i128> {
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

fn expression_name(expression: &Expr) -> Option<Vec<String>> {
    let mut current = expression;
    let mut names = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                names.push(identifier.name.clone());
                names.reverse();
                return Some(names);
            }
            Expr::Field { object, field, .. } => {
                names.push(field.name.clone());
                current = object;
            }
            Expr::GenericInstantiation { target, .. } => current = target,
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use falcon_dart_parser::parse;

    use super::*;
    use crate::{IdentityIndex, IdentitySource, TypeIndex};

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
        let owner = model.resolve_value(&[name.to_string()])?;
        evaluate_constant(initializer, &owner, &HashMap::new(), model, signatures)
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
