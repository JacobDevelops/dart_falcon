//! Flags a whole-number double literal, such as `1.0`, where the context type is
//! `double` so the int literal `1` means the same value.
//!
//! Since Dart 2.1 an int literal in a `double` context is a double constant,
//! so the `.0` is noise:
//!
//! ```dart
//! const double myDouble = 8.0;  // flagged
//! final anotherDouble = myDouble + 7.0e2; // fine: no `double` context
//! main() { someMethod(6.0); }   // flagged when the parameter is `double`
//! ```
//!
//! The literal (optionally negated) is checked only where the replacement
//! keeps its meaning: the initializer of a variable, field, or top-level
//! declared `double`; an element of a `<double>[...]` list; a value returned
//! from a function, method, getter, or operator declared to return `double`;
//! and an argument whose parameter resolves to `double`. Parenthesized
//! literals and values outside the 64-bit integer range are left alone.
//!
//! Adopted from the official `prefer_int_literals` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_field_decl,
    walk_function_decl, walk_getter_decl, walk_local_var, walk_method_decl, walk_stmt,
    walk_top_level_decl,
};
use std::collections::HashMap;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

const RULE: &str = "prefer-int-literals";
const MESSAGE: &str = "Unnecessary use of a 'double' literal.";

#[derive(Default)]
pub struct PreferIntLiterals;

impl Rule for PreferIntLiterals {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let diagnostics = {
            let mut declared = DeclaredContexts {
                diagnostics: Vec::new(),
                ctx,
                model: &state.model,
                returns_double: Vec::new(),
            };
            declared.visit_program(program);
            declared.diagnostics
        };
        let mut arguments = ArgumentContexts { diagnostics, ctx };
        visit_program(&mut arguments, program, state);
        arguments.diagnostics
    }
}

/// The span of the double literal in `expr` (`1.0` or `-1.0`) when it holds a
/// whole number an int literal can spell.
fn replaceable_literal(source: &str, expr: &Expr) -> Option<Span> {
    let (value, span) = match expr {
        Expr::DoubleLit { value, span } => (value, *span),
        Expr::Unary {
            op: UnaryOp::Minus,
            operand,
            ..
        } => match operand.as_ref() {
            Expr::DoubleLit { value, span } if !is_parenthesized(source, *span) => (value, *span),
            _ => return None,
        },
        _ => return None,
    };
    let value: f64 = value.replace('_', "").parse().ok()?;
    // Mirrors `value == value.truncate()` on a 64-bit int.
    let whole = value.is_finite() && value.trunc() == value && value.abs() <= i64::MAX as f64;
    whole.then_some(span)
}

fn previous_char(source: &str, offset: usize) -> Option<char> {
    source.get(..offset)?.trim_end().chars().next_back()
}

fn next_char(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.trim_start().chars().next()
}

/// Whether `span` is wrapped in parentheses; the parser drops them from the AST.
fn is_parenthesized(source: &str, span: Span) -> bool {
    previous_char(source, span.start) == Some('(') && next_char(source, span.end) == Some(')')
}

/// Like [`is_parenthesized`] for a call argument, whose own `(` may be the
/// argument list's: only an extra `(` after `(`, `,`, or `name:` counts.
fn is_parenthesized_argument(source: &str, span: Span) -> bool {
    if !is_parenthesized(source, span) {
        return false;
    }
    let open = source[..span.start].trim_end().len() - 1;
    matches!(previous_char(source, open), Some('(' | ',' | ':'))
}

fn is_double(ty: &ResolvedType) -> bool {
    ty.interface("dart:core", "double")
}

/// Declared-type contexts: variable initializers, `<double>[...]` elements,
/// and returns from functions declared to return `double`.
struct DeclaredContexts<'a, 'm> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: &'m SemanticModel<'a>,
    /// Whether each enclosing function body is declared to return `double`.
    returns_double: Vec<bool>,
}

impl DeclaredContexts<'_, '_> {
    fn declares_double(&self, ty: Option<&DartType>) -> bool {
        ty.is_some_and(|ty| is_double(&self.model.resolve_type(ty)))
    }

    fn check(&mut self, expr: &Expr) {
        if is_parenthesized(self.ctx.source, *expr.span()) {
            return;
        }
        if let Some(span) = replaceable_literal(self.ctx.source, expr) {
            self.diagnostics
                .push(self.ctx.diagnostic(RULE, MESSAGE, span));
        }
    }

    fn check_declarators(&mut self, ty: Option<&DartType>, declarators: &[VarDeclarator]) {
        if !self.declares_double(ty) {
            return;
        }
        for initializer in declarators.iter().filter_map(|d| d.initializer.as_ref()) {
            self.check(initializer);
        }
    }

    /// Walks a function with `return_type`, checking an arrow body directly.
    fn in_function(
        &mut self,
        return_type: Option<&DartType>,
        body: Option<&FunctionBody>,
        walk: impl FnOnce(&mut Self),
    ) {
        let returns_double = self.declares_double(return_type);
        if returns_double && let Some(FunctionBody::Arrow(expr, _)) = body {
            self.check(expr);
        }
        self.returns_double.push(returns_double);
        walk(self);
        self.returns_double.pop();
    }
}

impl Visitor for DeclaredContexts<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.check_declarators(variable.var_type.as_ref(), &variable.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check_declarators(node.field_type.as_ref(), &node.declarators);
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.check_declarators(node.var_type.as_ref(), &node.declarators);
        walk_local_var(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.in_function(node.return_type.as_ref(), node.body.as_ref(), |this| {
            walk_function_decl(this, node)
        });
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.in_function(node.return_type.as_ref(), node.body.as_ref(), |this| {
            walk_method_decl(this, node)
        });
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.in_function(node.return_type.as_ref(), node.body.as_ref(), |this| {
            walk_getter_decl(this, node)
        });
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.in_function(None, None, |this| walk_constructor_decl(this, node));
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.in_function(
                operator.return_type.as_ref(),
                operator.body.as_ref(),
                |this| walk_class_member(this, node),
            );
        } else {
            walk_class_member(self, node);
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalFunc(function) => {
                self.in_function(
                    function.return_type.as_ref(),
                    Some(&function.body),
                    |this| walk_stmt(this, node),
                );
                return;
            }
            Stmt::Return(ReturnStmt {
                value: Some(value), ..
            }) if self.returns_double.last() == Some(&true) => self.check(value),
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::FuncExpr { .. } => {
                self.in_function(None, None, |this| walk_expr(this, node));
                return;
            }
            Expr::List {
                type_arg: Some(element_type),
                elements,
                ..
            } if self.declares_double(Some(element_type)) => {
                for element in elements {
                    if let CollectionElement::Expr(expr) = element {
                        self.check(expr);
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

/// Argument contexts: positional and named arguments whose resolved parameter
/// type is `double`.
struct ArgumentContexts<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl ArgumentContexts<'_> {
    fn check(&mut self, parameter: &ResolvedType, argument: &Expr) {
        if !is_double(parameter) || is_parenthesized_argument(self.ctx.source, *argument.span()) {
            return;
        }
        if let Some(span) = replaceable_literal(self.ctx.source, argument) {
            self.diagnostics
                .push(self.ctx.diagnostic(RULE, MESSAGE, span));
        }
    }

    fn check_arguments(
        &mut self,
        args: &ArgList,
        positional: &[ResolvedType],
        named: &HashMap<String, ResolvedType>,
    ) {
        for (argument, parameter) in args.positional.iter().zip(positional) {
            self.check(parameter, argument);
        }
        for argument in &args.named {
            if let Some(parameter) = named.get(&argument.name.name) {
                self.check(parameter, &argument.value);
            }
        }
    }
}

impl SemanticRuleVisitor for ArgumentContexts<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                if let Some(signature) = state.call_signature(callee, type_args) {
                    self.check_arguments(args, &signature.positional, &signature.named);
                }
            }
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                let ResolvedType::Interface {
                    identity,
                    arguments,
                    ..
                } = state
                    .model
                    .resolve_type_in(dart_type, &state.type_parameters)
                else {
                    return;
                };
                let Some(signature) = state.signatures.constructor(
                    &identity,
                    constructor_name
                        .as_ref()
                        .map_or("new", |name| name.name.as_str()),
                ) else {
                    return;
                };
                let substitutions: HashMap<_, _> = signature
                    .owner_parameters
                    .iter()
                    .cloned()
                    .zip(arguments)
                    .collect();
                let positional: Vec<ResolvedType> = signature
                    .positional
                    .iter()
                    .map(|ty| state.model.substitute(ty, &substitutions))
                    .collect();
                let named: HashMap<String, ResolvedType> = signature
                    .named
                    .iter()
                    .map(|(name, ty)| (name.clone(), state.model.substitute(ty, &substitutions)))
                    .collect();
                self.check_arguments(args, &positional, &named);
            }
            _ => {}
        }
    }
}
