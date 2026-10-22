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
//! and an argument whose parameter resolves to `double`, including `super`
//! and `this` constructor initializers with instantiated superclass parameters.
//! Parenthesized literals and values outside the 64-bit integer range are left alone.
//!
//! Adopted from the official `prefer_int_literals` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_field_decl,
    walk_function_decl, walk_getter_decl, walk_local_var, walk_method_decl, walk_stmt,
    walk_top_level_decl,
};
use falcon_syntax::cst::*;
use std::collections::HashMap;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};


#[derive(Default)]
pub struct PreferIntLiterals;

impl Rule for PreferIntLiterals {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let diagnostics = {
            let mut declared = DeclaredContexts {
                diagnostics: Vec::new(),
                ctx,
                model: &model,
                returns_double: Vec::new(),
            };
            declared.visit_program(program);
            declared.diagnostics
        };
        RulePreparation::Semantic(Box::new(ArgumentContexts { diagnostics, ctx }))
    }
}

/// The span of the double literal in `expr` (`1.0` or `-1.0`) when it holds a
/// whole number an int literal can spell.
fn replaceable_literal(expr: Expr<'_>) -> Option<Span> {
    let (value, span) = match expr {
        Expr::DoubleLit(literal) => (literal.value(), literal.span()),
        Expr::Unary(unary) if unary.op() == UnaryOp::Minus => match unary.operand() {
            Expr::DoubleLit(literal) => (literal.value(), literal.span()),
            _ => return None,
        },
        _ => return None,
    };
    let value: f64 = value.replace('_', "").parse().ok()?;
    // Mirrors `value == value.truncate()` on a 64-bit int.
    let whole = value.is_finite() && value.trunc() == value && value.abs() <= i64::MAX as f64;
    whole.then_some(span)
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

impl<'a> DeclaredContexts<'a, '_> {
    fn declares_double(&self, ty: Option<DartType<'a>>) -> bool {
        ty.is_some_and(|ty| is_double(&self.model.resolve_type(ty)))
    }

    fn check(&mut self, expr: Expr<'a>) {
        if let Some(span) = replaceable_literal(expr) {
            self.diagnostics
                .push(self.ctx.diagnostic("Use an integer literal instead of this unnecessary `double` literal.", span));
        }
    }

    fn check_declarators(
        &mut self,
        ty: Option<DartType<'a>>,
        declarators: AstChildren<'a, VarDeclarator<'a>>,
    ) {
        if !self.declares_double(ty) {
            return;
        }
        for initializer in declarators.filter_map(|d| d.initializer()) {
            self.check(initializer);
        }
    }

    /// Walks a function with `return_type`, checking an arrow body directly.
    fn in_function(
        &mut self,
        return_type: Option<DartType<'a>>,
        body: Option<FunctionBody<'a>>,
        walk: impl FnOnce(&mut Self),
    ) {
        let returns_double = self.declares_double(return_type);
        if returns_double && let Some(FunctionBody::Arrow(body)) = body {
            self.check(body.expr());
        }
        self.returns_double.push(returns_double);
        walk(self);
        self.returns_double.pop();
    }
}

impl<'a> Visitor<'a> for DeclaredContexts<'a, '_> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::Variable(variable) = node {
            self.check_declarators(variable.var_type(), variable.declarators());
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        self.check_declarators(node.field_type(), node.declarators());
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        self.check_declarators(node.var_type(), node.declarators());
        walk_local_var(self, node);
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        self.in_function(node.return_type(), node.body(), |this| {
            walk_function_decl(this, node)
        });
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        self.in_function(node.return_type(), node.body(), |this| {
            walk_method_decl(this, node)
        });
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) {
        self.in_function(node.return_type(), node.body(), |this| {
            walk_getter_decl(this, node)
        });
    }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) {
        self.in_function(None, None, |this| walk_constructor_decl(this, node));
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        if let ClassMember::Operator(operator) = node {
            self.in_function(operator.return_type(), operator.body(), |this| {
                walk_class_member(this, node)
            });
        } else {
            walk_class_member(self, node);
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::LocalFunc(function) => {
                self.in_function(function.return_type(), Some(function.body()), |this| {
                    walk_stmt(this, node)
                });
                return;
            }
            Stmt::Return(statement) if self.returns_double.last() == Some(&true) => {
                if let Some(value) = statement.value() {
                    self.check(value);
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        match node {
            Expr::FuncExpr(_) => {
                self.in_function(None, None, |this| walk_expr(this, node));
                return;
            }
            Expr::List(list) if self.declares_double(list.type_arg()) => {
                for element in list.elements() {
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

impl<'a> ArgumentContexts<'a> {
    fn check(&mut self, parameter: &ResolvedType, argument: Expr<'a>) {
        if !is_double(parameter) {
            return;
        }
        if let Some(span) = replaceable_literal(argument) {
            self.diagnostics
                .push(self.ctx.diagnostic("Use an integer literal instead of this unnecessary `double` literal.", span));
        }
    }

    fn check_arguments(
        &mut self,
        args: ArgList<'a>,
        positional: &[ResolvedType],
        named: &HashMap<String, ResolvedType>,
    ) {
        for (argument, parameter) in args.positional().zip(positional) {
            self.check(parameter, argument);
        }
        for argument in args.named() {
            if let Some(parameter) = named.get(argument.name().name()) {
                self.check(parameter, argument.value());
            }
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for ArgumentContexts<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
    fn visit_call_arguments(
        &mut self,
        args: ArgList<'a>,
        positional: &[ResolvedType],
        named: &HashMap<String, ResolvedType>,
        _state: &SemanticState<'a>,
    ) {
        self.check_arguments(args, positional, named);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_dart_parser::parse;

    #[test]
    fn binary_operands_use_operator_parameters_not_the_result_context() {
        let source = r#"
T pick<T extends num>(T value) => value;
class Numeric {
  double operator +(num value) => 0;
  double operator -(double value) => 0;
}
void run(Numeric number) {
  double broad = number + pick(1.0);
  double narrow = number - pick(2.0);
  double direct = pick(3.0);
}
"#;
        let path = std::path::Path::new("/project/lib/main.dart");
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let indexes = crate::lint::test_support::TestIndexes::new(&[(path, &parsed)]);
        let config = falcon_config::FalconConfig::default();
        let context = indexes.context(path, &config, &parsed);
        let diagnostics = PreferIntLiterals.analyze(PreferIntLiterals::NAME, parsed.program(), &context);
        let reported: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(reported, ["2.0", "3.0"]);
    }
}
