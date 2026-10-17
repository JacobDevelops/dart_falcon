//! Flags a nullable `this.x` or `super.x` initializing formal whose constructor
//! asserts `x != null`.
//!
//! When a constructor's initializer list asserts that an initializing formal is
//! not null, the parameter should declare a non-nullable type instead, so the
//! check happens at compile time. The assert must be `assert(x != null)` or
//! `assert(null != x)` with `x` naming a `this.x` or `super.x` formal of the same
//! constructor whose type is nullable: the explicit parameter type, the field's
//! declared type for an untyped `this.x` (an untyped field without initializer
//! is `dynamic`, which counts as nullable), or the super constructor's parameter
//! type for an untyped `super.x`. A trailing `?` or `dynamic` proves
//! nullability even when the named type itself is unresolved; otherwise
//! unresolved types are never reported. The analyzer treats an undefined type
//! (a compile error) as nullable and reports it, but Falcon cannot tell an
//! undefined type from one declared in a library it does not load.
//! Parenthesized conditions or operands are not the analyzer's direct binary
//! identifier/null test. Repeated asserts on one formal produce one diagnostic.
//!
//! ```dart
//! class A {
//!   String? p;
//!   A(this.p) : assert(p != null); // flagged
//!   A.typed(String this.p);        // ok
//! }
//! ```
//!
//! Adopted from the official `tighten_type_of_initializing_formals` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_class_decl, walk_enum_decl, walk_top_level_decl};
use falcon_syntax::cst::*;

use crate::lint::expressions::is_parenthesized;
use crate::lint::semantic_type_operations::nullability;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct TightenTypeOfInitializingFormals;

impl Rule for TightenTypeOfInitializingFormals {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if ctx.identities.is_none() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mut asserts = NonNullAsserts {
            formals: HashMap::new(),
            fields: Vec::new(),
        };
        asserts.visit_program(program);
        if asserts.formals.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            formals: asserts.formals,
            reported: HashSet::new(),
            diagnostics: Vec::new(),
        }))
    }
}

/// Assert-initializer conditions (keyed by span) that null-check an
/// initializing formal, mapped to that formal's span and whether the formal's
/// type is syntactically proven nullable.
struct NonNullAsserts<'a> {
    formals: HashMap<(usize, usize), (Span, bool)>,
    /// Field declarations of the enclosing class or enum.
    fields: Vec<HashMap<&'a str, FieldShape>>,
}

/// What the declaration of a field proves about its type's nullability.
#[derive(Clone, Copy)]
enum FieldShape {
    /// `T?`, `dynamic`, or an untyped field without an initializer (`dynamic`).
    Nullable,
    Other,
}

impl<'a> NonNullAsserts<'a> {
    fn push_fields(&mut self, members: AstChildren<'a, ClassMember<'a>>) {
        let mut fields = HashMap::new();
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            for declarator in field.declarators() {
                let shape = match field.field_type() {
                    Some(ty) if ty.is_nullable() || matches!(ty, DartType::Dynamic(_)) => {
                        FieldShape::Nullable
                    }
                    None if declarator.initializer().is_none() => FieldShape::Nullable,
                    _ => FieldShape::Other,
                };
                fields.insert(declarator.name().name(), shape);
            }
        }
        self.fields.push(fields);
    }

    /// Whether the formal's type is proven nullable from its own annotation or,
    /// for an untyped `this.x`, from the field declaration.
    fn syntactically_nullable(&self, param: FormalParam<'a>) -> bool {
        match param.param_type() {
            Some(ty) => ty.is_nullable() || matches!(ty, DartType::Dynamic(_)),
            None => {
                matches!(param.initializing(), Some(InitializingFormal::This))
                    && param.function_params().is_none()
                    && self
                        .fields
                        .last()
                        .and_then(|fields| fields.get(param.name().name()))
                        .is_some_and(|shape| matches!(shape, FieldShape::Nullable))
            }
        }
    }
}

impl<'a> Visitor<'a> for NonNullAsserts<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::ExtensionType(extension) = node {
            self.push_fields(extension.members());
            if let Some(representation) = extension.representation() {
                let shape = if representation
                    .param_type()
                    .is_some_and(|ty| ty.is_nullable() || matches!(ty, DartType::Dynamic(_)))
                {
                    FieldShape::Nullable
                } else {
                    FieldShape::Other
                };
                if let Some(fields) = self.fields.last_mut() {
                    fields.insert(representation.name().name(), shape);
                }
            }
            walk_top_level_decl(self, node);
            self.fields.pop();
        } else {
            walk_top_level_decl(self, node);
        }
    }
    fn visit_class_decl(&mut self, node: ClassDecl<'a>) {
        self.push_fields(node.members());
        walk_class_decl(self, node);
        self.fields.pop();
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        self.push_fields(node.members());
        walk_enum_decl(self, node);
        self.fields.pop();
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) {
        for initializer in node.initializers {
            let ConstructorInitializer::Assert(assertion) = initializer else {
                continue;
            };
            let condition = assertion.condition();
            if is_parenthesized(condition) {
                continue;
            }
            let Some(name) = null_checked_identifier(condition) else {
                continue;
            };
            if let Some(param) = node
                .params
                .params()
                .find(|param| param.initializing().is_some() && param.name().name() == name.name())
            {
                let span = condition.span();
                self.formals.insert(
                    (span.start, span.end),
                    (param.span(), self.syntactically_nullable(param)),
                );
            }
        }
    }
}

/// Only bare identifier/null operands match the official rule.
fn null_checked_identifier(condition: Expr<'_>) -> Option<Identifier<'_>> {
    let Expr::Binary(binary) = condition else {
        return None;
    };
    if binary.op() != BinaryOp::NotEq {
        return None;
    }
    let left = binary.left();
    let right = binary.right();
    if is_parenthesized(left) || is_parenthesized(right) {
        return None;
    }
    match (left, right) {
        (Expr::Ident(identifier), Expr::NullLit(_))
        | (Expr::NullLit(_), Expr::Ident(identifier)) => Some(identifier),
        _ => None,
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    formals: HashMap<(usize, usize), (Span, bool)>,
    /// Each initializing formal is reported once even with duplicate asserts.
    reported: HashSet<(usize, usize)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let span = node.span();
        let Some(&(param, syntactically_nullable)) = self.formals.get(&(span.start, span.end))
        else {
            return;
        };
        let Some(identifier) = null_checked_identifier(node) else {
            return;
        };
        let ty = state.environment.lookup(identifier.name());
        let nullable = syntactically_nullable
            || matches!(ty, ResolvedType::Dynamic)
            || nullability(&ty, &state.signatures, self.ctx.types) == Truth::Yes;
        if nullable && self.reported.insert((param.start, param.end)) {
            self.diagnostics.push(self.ctx.diagnostic(
                "tighten-type-of-initializing-formals",
                "Use a type annotation rather than 'assert' to enforce non-nullability.",
                param,
            ));
        }
    }
}
