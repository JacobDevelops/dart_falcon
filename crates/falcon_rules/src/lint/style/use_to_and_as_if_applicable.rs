//! Flags conversion methods not named `to…` or `as…`.
//!
//! A parameterless method that just wraps the receiver in a new object —
//! `return Other(this);` — is a conversion. Dart convention names such methods
//! `toX()` when they copy state into a new object and `asX()` when they return a
//! view backed by the original, so readers know what the call costs.
//! The diagnostic covers the method name or operator token.
//!
//! ```dart
//! class Bar {
//!   Foo foo() => Foo.fromBar(this);    // flagged
//!   Foo toFoo() => Foo.fromBar(this);  // preferred
//! }
//! ```
//!
//! Non-getter methods and operators with no parameters, a non-`void` return
//! type, and a body consisting of one constructor call taking only `this` are
//! checked. Methods overriding inherited methods are exempt. Constructor calls
//! cannot be resolved when Dart's newer `new(...)` declaration syntax failed
//! to parse.
//!
//! Adopted from the official `use_to_and_as_if_applicable` lint.

use falcon_analyze::{
    AnalyzeContext, MemberKind, Rule, RulePreparation, SemanticModel, resolve_call_constructor,
};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseToAndAsIfApplicable;

impl Rule for UseToAndAsIfApplicable {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                continue;
            };
            // Extensions have no supertypes to inherit from.
            let owner = match decl {
                TopLevelDecl::Extension(_) => None,
                TopLevelDecl::Class(node) => Some(&node.name.name),
                TopLevelDecl::Mixin(node) => Some(&node.name.name),
                TopLevelDecl::MixinClass(node) => Some(&node.name.name),
                TopLevelDecl::Enum(node) => Some(&node.name.name),
                TopLevelDecl::ExtensionType(node) => Some(&node.name.name),
                _ => continue,
            };
            let owner = match owner {
                Some(name) => match model.resolve_name(name.as_str()) {
                    Some(identity) => Some(identity),
                    None => continue,
                },
                None => None,
            };
            for member in members {
                let (name, params, return_type, body, span) = match member {
                    ClassMember::Method(method) => (
                        method.name.name.as_str(),
                        &method.params,
                        &method.return_type,
                        &method.body,
                        method.name.span,
                    ),
                    ClassMember::Operator(operator) => (
                        operator.op.as_str(),
                        &operator.params,
                        &operator.return_type,
                        &operator.body,
                        crate::lint::declarations::operator_token_span(operator, ctx.syntax),
                    ),
                    _ => continue,
                };
                let Some(body) = body else {
                    continue;
                };
                if !params.params.is_empty()
                    || matches!(return_type, Some(DartType::Void { .. }))
                    || begins_with_as_or_to(name)
                    || !wraps_this(body, &model, signatures)
                {
                    continue;
                }
                let inherited = owner.as_ref().map_or(Some(false), |owner| {
                    signatures
                        .inherited_member_facts(owner, name)
                        .map(|facts| facts.iter().any(|fact| fact.kind == MemberKind::Method))
                });
                if inherited != Some(true) {
                    diags.push(ctx.diagnostic(
                        "use-to-and-as-if-applicable",
                        "Start the name of the method with 'to' or 'as'.",
                        span,
                    ));
                }
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

/// Matches `(to|as|_to|_as)[A-Z]` at the start of `name`.
fn begins_with_as_or_to(name: &str) -> bool {
    let rest = name.strip_prefix('_').unwrap_or(name);
    rest.strip_prefix("to")
        .or_else(|| rest.strip_prefix("as"))
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// Whether the body is a single constructor call taking exactly `this`.
fn wraps_this(
    body: &FunctionBody,
    model: &SemanticModel<'_>,
    signatures: &falcon_analyze::SignatureIndex,
) -> bool {
    let expr = match body {
        FunctionBody::Arrow(expr, _) => &**expr,
        FunctionBody::Block(block) => match block.stmts.as_slice() {
            [
                Stmt::Return(ReturnStmt {
                    value: Some(value), ..
                }),
            ] => value,
            _ => return false,
        },
        FunctionBody::Native(..) => return false,
    };
    let (args, is_creation) = match expr.unparenthesized() {
        Expr::New { args, .. } => (args, true),
        Expr::Call { callee, args, .. } => (
            args,
            // A bare type name can only be invoked as its unnamed
            // constructor; `Type.name` may be a static method, so it must
            // name a known constructor.
            resolve_call_constructor(callee, model, false).is_some_and(
                |(identity, constructor)| {
                    matches!(callee.unparenthesized(), Expr::Ident(_))
                        || signatures.constructor(&identity, constructor).is_some()
                },
            ),
        ),
        _ => return false,
    };
    is_creation
        && args.named.is_empty()
        && matches!(args.positional.as_slice(), [argument] if matches!(argument.unparenthesized(), Expr::This { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference as span_test_support;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn reports_operator_token_and_method_identifier() {
        let source = r"
class Wrapper { Wrapper(Object value); }
class Convert {
  Wrapper operator ~() => Wrapper(this);
  Wrapper wrap() => Wrapper(this);
}
";
        let diagnostics = span_test_support::analyze(
            &UseToAndAsIfApplicable,
            Path::new("test.dart"),
            source,
            &FalconConfig::default(),
        );
        assert_eq!(diagnostics.len(), 2);
        for (diagnostic, expected) in diagnostics.iter().zip(["~", "wrap"]) {
            assert_eq!(diagnostic.span.start, source.find(expected).unwrap());
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }
}
