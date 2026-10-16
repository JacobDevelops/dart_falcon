//! Flags classes that emulate an enum and could be declared as one.
//!
//! Before enhanced enums (Dart 2.17), a fixed set of values with fields and
//! methods was modeled as a class with private const constructors and
//! `static const` instances. An `enum` now expresses the same thing with
//! exhaustiveness checking, `values`, `index`, and `name` for free.
//!
//! ```dart
//! class LogPriority {           // flagged
//!   static const error = LogPriority._(1, 'Error');
//!   static const warning = LogPriority._(2, 'Warning');
//!   final int priority;
//!   final String prefix;
//!   const LogPriority._(this.priority, this.prefix);
//! }
//! ```
//!
//! A class qualifies when it is concrete, extends only `Object`, declares no
//! `==`, `hashCode`, `index`, or `values` member, has only const or factory
//! constructors (all private if the class is public), and has two or more
//! `static const` fields initialized by its own generative constructor. It is
//! disqualified when the file subclasses, implements, or mixes it in, or
//! invokes a generative constructor anywhere other than those initializers.
//!
//! Adopted from the official `use_enums` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UseEnums;

impl Rule for UseEnums {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Diagnostics(
            program
                .declarations
                .iter()
                .enumerate()
                .filter_map(|(index, decl)| match decl {
                    TopLevelDecl::Class(class) if behaves_like_enum(class, index, program) => Some(
                        ctx.diagnostic("use-enums", "Class should be an enum.", class.name.span),
                    ),
                    _ => None,
                })
                .collect(),
        )
    }
}

fn behaves_like_enum(class: &ClassDecl, index: usize, program: &Program) -> bool {
    if class.modifiers.is_abstract
        || class.modifiers.restriction == Some(ClassRestriction::Sealed)
        || class.extends.as_ref().is_some_and(|ty| !is_object(ty))
    {
        return false;
    }
    let public = !class.name.name.starts_with('_');
    let mut generative: Vec<(&str, bool)> = Vec::new();
    if let Some(primary) = &class.primary_constructor {
        let name = primary
            .constructor_name
            .as_ref()
            .map_or("new", |name| name.name.as_str());
        if !primary.is_const
            || (public && !name.starts_with('_'))
            || primary
                .params
                .params
                .iter()
                .any(|param| is_reserved(&param.name.name))
        {
            return false;
        }
        generative.push((name, true));
    }
    for member in &class.members {
        match member {
            ClassMember::Operator(op) if op.op == "==" => return false,
            ClassMember::Field(field)
                if field
                    .declarators
                    .iter()
                    .any(|declarator| is_reserved(&declarator.name.name)) =>
            {
                return false;
            }
            ClassMember::Method(MethodDecl { name, .. })
            | ClassMember::Getter(GetterDecl { name, .. })
            | ClassMember::Setter(SetterDecl { name, .. })
                if is_reserved(&name.name) =>
            {
                return false;
            }
            ClassMember::Constructor(constructor) => {
                if !constructor.is_factory && !constructor.is_const {
                    return false;
                }
                let private = constructor
                    .constructor_name
                    .as_ref()
                    .is_some_and(|name| name.name.starts_with('_'));
                if public && !private {
                    return false;
                }
                if !constructor.is_factory {
                    generative.push((constructor_name(constructor), constructor.is_const));
                }
            }
            _ => {}
        }
    }

    let name = class.name.name.as_str();
    let mut candidates = Vec::new();
    for member in &class.members {
        let ClassMember::Field(field) = member else {
            continue;
        };
        if !field.is_static || field.keyword != VarKeyword::Const {
            continue;
        }
        for initializer in field
            .declarators
            .iter()
            .filter_map(|d| d.initializer.as_ref())
        {
            let is_candidate = creation(initializer, name).is_some_and(|invoked| {
                generative
                    .iter()
                    .any(|&(constructor, is_const)| is_const && constructor == invoked)
            });
            if is_candidate {
                candidates.push(initializer.span().start);
            }
        }
    }
    if candidates.len() < 2 {
        return false;
    }

    let generative: Vec<&str> = generative.iter().map(|&(name, _)| name).collect();
    let mut finder = StrayCreation {
        name,
        generative: &generative,
        allowed: &candidates,
        found: false,
    };
    finder.visit_class_decl(class);
    for (other_index, decl) in program.declarations.iter().enumerate() {
        if other_index == index {
            continue;
        }
        if inherits_from(decl, name) {
            return false;
        }
        finder.visit_top_level_decl(decl);
    }
    !finder.found
}

/// Members whose presence conflicts with an enum's implicit API.
fn is_reserved(name: &str) -> bool {
    matches!(name, "hashCode" | "index" | "values")
}

fn is_object(ty: &DartType) -> bool {
    matches!(ty, DartType::Named(named)
        if named.type_args.is_empty()
            && matches!(named.segments.as_slice(), [segment] if segment.name == "Object"))
}

fn constructor_name(constructor: &ConstructorDecl) -> &str {
    constructor
        .constructor_name
        .as_ref()
        .map_or("new", |name| name.name.as_str())
}

fn names_class(ty: &DartType, name: &str) -> bool {
    matches!(ty, DartType::Named(named)
        if matches!(named.segments.as_slice(), [segment] if segment.name == name))
}

/// Another class in the file that extends, implements, or mixes in `name`.
fn inherits_from(decl: &TopLevelDecl, name: &str) -> bool {
    let (extends, with_clause, implements) = match decl {
        TopLevelDecl::Class(node) => (&node.extends, &node.with_clause, &node.implements),
        TopLevelDecl::MixinClass(node) => (&node.extends, &node.with_clause, &node.implements),
        _ => return false,
    };
    extends
        .iter()
        .chain(with_clause)
        .chain(implements)
        .any(|ty| names_class(ty, name))
}

/// The constructor a creation expression of class `name` invokes.
fn creation<'a>(expr: &'a Expr, name: &str) -> Option<&'a str> {
    match expr.unparenthesized() {
        Expr::New {
            dart_type: DartType::Named(named),
            constructor_name,
            ..
        } => match (named.segments.as_slice(), constructor_name) {
            ([class], ctor) if class.name == name => {
                Some(ctor.as_ref().map_or("new", |ctor| ctor.name.as_str()))
            }
            // `const C.named()` parses `C.named` as a qualified type name.
            ([class, ctor], None) if class.name == name => Some(ctor.name.as_str()),
            _ => None,
        },
        Expr::Call { callee, .. } => {
            // `C<int>(...)` and `C<int>.named(...)` name the class through a
            // generic instantiation.
            let names_class = |expr: &Expr| match expr.unparenthesized() {
                Expr::GenericInstantiation { target, .. } => {
                    matches!(target.unparenthesized(), Expr::Ident(identifier) if identifier.name == name)
                }
                Expr::Ident(identifier) => identifier.name == name,
                _ => false,
            };
            match callee.unparenthesized() {
                Expr::Field { object, field, .. } if names_class(object) => {
                    Some(field.name.as_str())
                }
                callee if names_class(callee) => Some("new"),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Finds an invocation of one of the class's generative constructors outside
/// the candidate constant initializers. A dot-shorthand call naming such a
/// constructor counts too, since its target type is not tracked.
struct StrayCreation<'a> {
    name: &'a str,
    generative: &'a [&'a str],
    allowed: &'a [usize],
    found: bool,
}

impl Visitor for StrayCreation<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::Paren { .. }) {
            falcon_syntax::visitor::walk_expr(self, node);
            return;
        }
        if self.found {
            return;
        }
        let invoked = creation(node, self.name).or(match node {
            Expr::Call { callee, .. } => match callee.unparenthesized() {
                Expr::DotShorthand { name, .. } => Some(name.name.as_str()),
                _ => None,
            },
            _ => None,
        });
        if invoked.is_some_and(|invoked| self.generative.contains(&invoked))
            && !self.allowed.contains(&node.span().start)
        {
            self.found = true;
            return;
        }
        walk_expr(self, node);
    }
}
