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
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseToAndAsIfApplicable;

impl Rule for UseToAndAsIfApplicable {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            signatures,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            owner: None,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    signatures: &'a SignatureIndex,
    model: SemanticModel<'a>,
    owner: Option<Option<DeclarationIdentity>>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        self.owner = None;
            // Extensions have no supertypes to inherit from.
            let owner = match decl {
                TopLevelDecl::Extension(_) => None,
                TopLevelDecl::Class(node) => Some(node.name().name()),
                TopLevelDecl::Mixin(node) => Some(node.name().name()),
                TopLevelDecl::MixinClass(node) => Some(node.name().name()),
                TopLevelDecl::Enum(node) => Some(node.name().name()),
                TopLevelDecl::ExtensionType(node) => Some(node.name().name()),
                _ => return StructuralTraversal::Continue,
            };
            let owner = match owner {
                Some(name) => match self.model.resolve_name(name) {
                    Some(identity) => Some(identity),
                    None => return StructuralTraversal::Continue,
                },
                None => None,
            };
        self.owner = Some(owner);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let Some(owner) = &self.owner else {
            return StructuralTraversal::Continue;
        };
                let (name, params, return_type, body, span) = match member {
                    ClassMember::Method(method) => (
                        method.name().name(),
                        method.params(),
                        method.return_type(),
                        method.body(),
                        method.name().span(),
                    ),
                    ClassMember::Operator(operator) => (
                        operator.op(),
                        operator.params(),
                        operator.return_type(),
                        operator.body(),
                        crate::lint::declarations::operator_token_span(operator),
                    ),
                    _ => return StructuralTraversal::Continue,
                };
                let Some(body) = body else {
                    return StructuralTraversal::Continue;
                };
                if !params.params().is_empty()
                    || matches!(return_type, Some(DartType::Void(_)))
                    || begins_with_as_or_to(name)
                    || !wraps_this(body, &self.model, self.signatures)
                {
                    return StructuralTraversal::Continue;
                }
                let inherited = owner.as_ref().map_or(Some(false), |owner| {
                    self.signatures
                        .inherited_member_facts(owner, name)
                        .map(|facts| facts.iter().any(|fact| fact.kind == MemberKind::Method))
                });
                if inherited != Some(true) {
                    self.diags.push(self.ctx.diagnostic(
                        "Start the name of the method with `to` or `as`.",
                        span,
                    ));
                }
        StructuralTraversal::Continue
    }
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
    body: FunctionBody<'_>,
    model: &SemanticModel<'_>,
    signatures: &falcon_analyze::SignatureIndex,
) -> bool {
    let expr = match body {
        FunctionBody::Arrow(arrow) => arrow.expr(),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            match (statements.next(), statements.next()) {
                (Some(Stmt::Return(statement)), None) => match statement.value() {
                    Some(value) => value,
                    None => return false,
                },
                _ => return false,
            }
        }
        FunctionBody::Native(_) => return false,
    };
    let (args, is_creation) = match expr.unparenthesized() {
        Expr::New(new) => (new.args(), true),
        Expr::Call(call) => (
            call.args(),
            // A bare type name can only be invoked as its unnamed
            // constructor; `Type.name` may be a static method, so it must
            // name a known constructor.
            resolve_call_constructor(call.callee(), model, false).is_some_and(
                |(identity, constructor)| {
                    matches!(call.callee().unparenthesized(), Expr::Ident(_))
                        || signatures.constructor(&identity, constructor).is_some()
                },
            ),
        ),
        _ => return false,
    };
    let mut positional = args.positional();
    is_creation
        && args.named().is_empty()
        && matches!(positional.next(), Some(argument) if matches!(argument.unparenthesized(), Expr::This(_)))
        && positional.next().is_none()
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
            UseToAndAsIfApplicable::NAME,
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
