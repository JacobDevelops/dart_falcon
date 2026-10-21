//! Warns when credential-like variables, fields, or parameter defaults use
//! nonempty string or numeric literals.
//!
//! Names such as `password`, `apiKey`, `clientSecret`, and `accessToken` are
//! checked case-insensitively, including snake_case and private spellings.
//! String values matching their variable name are ignored as storage-key names.
//! Read credentials from secure runtime configuration instead. This is a naming
//! heuristic, not a general-purpose secrets scanner.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidHardcodedCredentials;

impl Rule for AvoidHardcodedCredentials {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>, value: Expr<'_>) {
        let name_text = name.name().trim_start_matches('_');
        if ![
            "password",
            "passwd",
            "secret",
            "clientSecret",
            "client_secret",
            "apiKey",
            "api_key",
            "accessToken",
            "access_token",
            "authToken",
            "auth_token",
            "privateKey",
            "private_key",
        ]
        .iter()
        .any(|candidate| name_text.eq_ignore_ascii_case(candidate))
        {
            return;
        }
        let literal = match value.unparenthesized() {
            Expr::StringLit(string) => {
                string.interpolations().next().is_none()
                    && !string.value().is_empty()
                    && !string
                        .value()
                        .bytes()
                        .filter(|byte| !matches!(byte, b'_' | b'-'))
                        .map(|byte| byte.to_ascii_lowercase())
                        .eq(name_text
                            .bytes()
                            .filter(|byte| !matches!(byte, b'_' | b'-'))
                            .map(|byte| byte.to_ascii_lowercase()))
            }
            Expr::IntLit(_) | Expr::DoubleLit(_) => true,
            _ => false,
        };
        if literal {
            self.diagnostics.push(self.ctx.diagnostic("Credential-like name is assigned a hardcoded value; read the credential from secure runtime configuration.", name.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_var_declarator(&mut self, node: VarDeclarator<'a>) -> StructuralTraversal {
        if let Some(value) = node.initializer() {
            self.check(node.name(), value);
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        if let Some(value) = node.default_value() {
            self.check(node.name(), value);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Assign(assignment) = node {
            match assignment.target().unparenthesized() {
                Expr::Ident(identifier) => self.check(identifier, assignment.value()),
                Expr::Field(property) => self.check(property.field(), assignment.value()),
                _ => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
