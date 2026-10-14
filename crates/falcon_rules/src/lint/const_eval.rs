use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    DeclarationIdentity, SemanticModel, SignatureIndex, evaluate_constant, expression_segments,
};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub(crate) fn arguments_potentially_constant(
    args: &ArgList,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    args.positional
        .iter()
        .chain(args.named.iter().map(|argument| &argument.value))
        .all(|argument| {
            potentially_constant_with_shadowed(
                argument, allowed, shadowed, owner, fields, model, signatures,
            )
        })
}

pub(crate) fn potentially_constant(
    expr: &Expr,
    allowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    potentially_constant_with_shadowed(expr, allowed, allowed, owner, fields, model, signatures)
}

pub(crate) fn potentially_constant_with_shadowed(
    expr: &Expr,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let mut checker = PotentiallyConstant {
        allowed,
        shadowed,
        owner,
        fields,
        model,
        signatures,
        valid: true,
    };
    checker.visit_expr(expr);
    checker.valid
}

struct PotentiallyConstant<'a, 'model> {
    allowed: &'a HashSet<String>,
    shadowed: &'a HashSet<String>,
    owner: &'a DeclarationIdentity,
    fields: &'a HashMap<&'a str, &'a Expr>,
    model: &'a SemanticModel<'model>,
    signatures: &'a SignatureIndex,
    valid: bool,
}

impl Visitor for PotentiallyConstant<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if !self.valid {
            return;
        }
        if !matches!(node, Expr::Call { .. } | Expr::New { .. })
            && evaluate_constant(
                node,
                Some(self.owner),
                self.fields,
                self.model,
                self.signatures,
            )
            .is_some()
        {
            return;
        }
        match node {
            Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
            | Expr::SymbolLit { .. } => {}
            Expr::Ident(identifier) => {
                self.valid = self.allowed.contains(&identifier.name)
                    || evaluate_constant(
                        node,
                        Some(self.owner),
                        self.fields,
                        self.model,
                        self.signatures,
                    )
                    .is_some();
            }
            Expr::Call { callee, args, .. } => {
                let shadowed = expression_segments(callee, false)
                    .and_then(|segments| segments.into_iter().next())
                    .is_some_and(|name| self.shadowed.contains(name));
                self.valid = !shadowed && const_constructor(callee, self.model, self.signatures);
                if self.valid {
                    self.visit_arguments(args);
                }
            }
            Expr::New {
                is_const: true,
                args,
                ..
            } => self.visit_arguments(args),
            Expr::StringLit(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Conditional { .. }
            | Expr::As { .. }
            | Expr::Field { .. }
            | Expr::NullAssert { .. }
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. } => walk_expr(self, node),
            _ => self.valid = false,
        }
    }
}

impl PotentiallyConstant<'_, '_> {
    fn visit_arguments(&mut self, args: &ArgList) {
        for argument in args
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value))
        {
            self.visit_expr(argument);
        }
    }
}

fn const_constructor(
    callee: &Expr,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some((identity, constructor)) =
        falcon_analyze::resolve_call_constructor(callee, model, false)
    else {
        return false;
    };
    signatures.is_const_constructor(&identity, constructor)
}
