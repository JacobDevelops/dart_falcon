//! Flags field initializers in classes with a `const` constructor.
//!
//! In a class that can be instantiated as a constant, a `final` instance field
//! initialized to a fixed value — either at its declaration or in the
//! initializer list of the class's only `const` constructor without using any
//! constructor parameter — stores the same value in every instance. A getter
//! expresses that without spending a field per object.
//!
//! ```dart
//! class A {
//!   final a = const [];         // flagged
//!   const A();
//! }
//!
//! class B {
//!   final b;
//!   const B() : b = const [];   // flagged
//! }
//!
//! class C {
//!   List get c => const [];     // preferred
//!   const C();
//! }
//! ```
//!
//! An initializer-list entry that reads a constructor parameter is allowed, and
//! initializer lists are not checked when the class declares several
//! constructors.
//!
//! Adopted from the official `avoid_field_initializers_in_const_classes` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

const MESSAGE: &str = "Fields in 'const' classes should not have initializers.";

#[derive(Default)]
pub struct AvoidFieldInitializersInConstClasses;

impl Rule for AvoidFieldInitializersInConstClasses {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let members = match decl {
                TopLevelDecl::Class(class) => &class.members,
                TopLevelDecl::MixinClass(class) => &class.members,
                _ => continue,
            };
            check_members(members, ctx, &mut diags);
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn check_members(members: &[ClassMember], ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let constructors: Vec<&ConstructorDecl> = members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        })
        .collect();
    if !constructors.iter().any(|constructor| constructor.is_const) {
        return;
    }
    for member in members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        if field.is_static || field.keyword != VarKeyword::Final {
            continue;
        }
        for declarator in &field.declarators {
            if declarator.initializer.is_some() {
                diags.push(ctx.diagnostic(
                    "avoid-field-initializers-in-const-classes",
                    MESSAGE,
                    declarator.span,
                ));
            }
        }
    }
    let [constructor] = constructors.as_slice() else {
        return;
    };
    if !constructor.is_const {
        return;
    }
    let parameters: Vec<&str> = constructor
        .params
        .params
        .iter()
        .map(|param| param.name.name.as_str())
        .collect();
    for initializer in &constructor.initializers {
        let ConstructorInitializer::FieldInit { value, span, .. } = initializer else {
            continue;
        };
        let mut finder = ParameterReference {
            parameters: &parameters,
            found: false,
        };
        finder.visit_expr(value);
        if !finder.found {
            diags.push(ctx.diagnostic("avoid-field-initializers-in-const-classes", MESSAGE, *span));
        }
    }
}

/// Finds any identifier naming one of the constructor's parameters. A const
/// initializer cannot contain closures, so no inner scope can shadow them.
struct ParameterReference<'a> {
    parameters: &'a [&'a str],
    found: bool,
}

impl Visitor for ParameterReference<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if self.found {
            return;
        }
        if let Expr::Ident(identifier) = node
            && self.parameters.contains(&identifier.name.as_str())
        {
            self.found = true;
            return;
        }
        walk_expr(self, node);
    }
}
