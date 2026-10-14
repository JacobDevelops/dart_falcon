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
//! unresolved types are never reported.
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

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_class_decl, walk_constructor_decl, walk_enum_decl};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::semantic_type_operations::nullability;

#[derive(Default)]
pub struct TightenTypeOfInitializingFormals;

impl Rule for TightenTypeOfInitializingFormals {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut asserts = NonNullAsserts::default();
        asserts.visit_program(program);
        if asserts.formals.is_empty() {
            return Vec::new();
        }
        let mut collector = Collector {
            ctx,
            formals: asserts.formals,
            diagnostics: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

/// Assert-initializer conditions (keyed by span) that null-check an
/// initializing formal, mapped to that formal's span and whether the formal's
/// type is syntactically proven nullable.
#[derive(Default)]
struct NonNullAsserts {
    formals: HashMap<(usize, usize), (Span, bool)>,
    /// Field declarations of the enclosing class or enum.
    fields: Vec<HashMap<String, FieldShape>>,
}

/// What the declaration of a field proves about its type's nullability.
#[derive(Clone, Copy)]
enum FieldShape {
    /// `T?`, `dynamic`, or an untyped field without an initializer (`dynamic`).
    Nullable,
    Other,
}

impl NonNullAsserts {
    fn push_fields(&mut self, members: &[ClassMember]) {
        let mut fields = HashMap::new();
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            for declarator in &field.declarators {
                let shape = match &field.field_type {
                    Some(ty) if ty.is_nullable() || matches!(ty, DartType::Dynamic { .. }) => {
                        FieldShape::Nullable
                    }
                    None if declarator.initializer.is_none() => FieldShape::Nullable,
                    _ => FieldShape::Other,
                };
                fields.insert(declarator.name.name.clone(), shape);
            }
        }
        self.fields.push(fields);
    }

    /// Whether the formal's type is proven nullable from its own annotation or,
    /// for an untyped `this.x`, from the field declaration.
    fn syntactically_nullable(&self, param: &FormalParam) -> bool {
        match &param.param_type {
            Some(ty) => ty.is_nullable() || matches!(ty, DartType::Dynamic { .. }),
            None => {
                matches!(param.initializing, Some(InitializingFormal::This))
                    && param.function_params.is_none()
                    && self
                        .fields
                        .last()
                        .and_then(|fields| fields.get(&param.name.name))
                        .is_some_and(|shape| matches!(shape, FieldShape::Nullable))
            }
        }
    }
}

impl Visitor for NonNullAsserts {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.push_fields(&node.members);
        walk_class_decl(self, node);
        self.fields.pop();
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        self.push_fields(&node.members);
        walk_enum_decl(self, node);
        self.fields.pop();
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        for initializer in &node.initializers {
            let ConstructorInitializer::Assert { condition, .. } = initializer else {
                continue;
            };
            let Some(name) = null_checked_identifier(condition) else {
                continue;
            };
            if let Some(param) = node
                .params
                .params
                .iter()
                .find(|param| param.initializing.is_some() && param.name.name == name.name)
            {
                let span = condition.span();
                self.formals.insert(
                    (span.start, span.end),
                    (param.span, self.syntactically_nullable(param)),
                );
            }
        }
        walk_constructor_decl(self, node);
    }
}

/// The identifier `x` of an `x != null` or `null != x` condition.
fn null_checked_identifier(condition: &Expr) -> Option<&Identifier> {
    let Expr::Binary {
        op: BinaryOp::NotEq,
        left,
        right,
        ..
    } = condition
    else {
        return None;
    };
    match (left.as_ref(), right.as_ref()) {
        (Expr::Ident(identifier), Expr::NullLit { .. })
        | (Expr::NullLit { .. }, Expr::Ident(identifier)) => Some(identifier),
        _ => None,
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    formals: HashMap<(usize, usize), (Span, bool)>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let span = node.span();
        let Some(&(param, syntactically_nullable)) = self.formals.get(&(span.start, span.end))
        else {
            return;
        };
        let Some(identifier) = null_checked_identifier(node) else {
            return;
        };
        let ty = state.environment.lookup(&identifier.name);
        let nullable = syntactically_nullable
            || matches!(ty, ResolvedType::Dynamic)
            || nullability(&ty, &state.signatures, self.ctx.types) == Truth::Yes;
        if nullable {
            self.diagnostics.push(self.ctx.diagnostic(
                "tighten-type-of-initializing-formals",
                "Use a type annotation rather than 'assert' to enforce non-nullability.",
                param,
            ));
        }
    }
}
