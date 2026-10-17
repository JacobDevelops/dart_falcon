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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseEnums;

impl Rule for UseEnums {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let candidates: Vec<_> = program
            .declarations
            .iter()
            .filter_map(|decl| {
                if let TopLevelDecl::Class(class) = decl {
                    enum_candidate(class)
                } else {
                    None
                }
            })
            .collect();
        if candidates.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector { ctx, candidates }))
    }
}

struct Candidate<'a> {
    name: &'a str,
    span: Span,
    generative: Vec<&'a str>,
    allowed: Vec<usize>,
    found: bool,
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    candidates: Vec<Candidate<'a>>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates.into_iter().filter(|candidate| !candidate.found).map(|candidate| {
            self.ctx.diagnostic("use-enums", "Class should be an enum.", candidate.span)
        }).collect()
    }

    fn visit_library_directive(&mut self, _node: &LibraryDirective) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_part_of_directive(&mut self, _node: &PartOfDirective) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_part_directive(&mut self, _node: &PartDirective) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_import(&mut self, _node: &ImportDirective) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_export(&mut self, _node: &ExportDirective) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) -> StructuralTraversal {
        for candidate in &mut self.candidates {
            if !candidate.found
                && !matches!(decl, TopLevelDecl::Class(class) if class.name.span == candidate.span)
                && inherits_from(decl, candidate.name)
            {
                candidate.found = true;
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if !matches!(node, Expr::Paren { .. }) {
            for candidate in &mut self.candidates {
                if candidate.found {
                    continue;
                }
                let invoked = creation(node, candidate.name).or(match node {
                    Expr::Call { callee, .. } => match callee.unparenthesized() {
                        Expr::DotShorthand { name, .. } => Some(name.name.as_str()),
                        _ => None,
                    },
                    _ => None,
                });
                if invoked.is_some_and(|invoked| candidate.generative.contains(&invoked))
                    && !candidate.allowed.contains(&node.span().start)
                {
                    candidate.found = true;
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}

fn enum_candidate(class: &ClassDecl) -> Option<Candidate<'_>> {
    if class.modifiers.is_abstract
        || class.modifiers.restriction == Some(ClassRestriction::Sealed)
        || class.extends.as_ref().is_some_and(|ty| !is_object(ty))
    {
        return None;
    }
    let public = !class.name.name.starts_with('_');
    let mut generative: Vec<&str> = Vec::new();
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
            return None;
        }
        generative.push(name);
    }
    for member in &class.members {
        match member {
            ClassMember::Operator(op) if op.op == "==" => return None,
            ClassMember::Field(field)
                if field
                    .declarators
                    .iter()
                    .any(|declarator| is_reserved(&declarator.name.name)) =>
            {
                return None;
            }
            ClassMember::Method(MethodDecl { name, .. })
            | ClassMember::Getter(GetterDecl { name, .. })
            | ClassMember::Setter(SetterDecl { name, .. })
                if is_reserved(&name.name) =>
            {
                return None;
            }
            ClassMember::Constructor(constructor) => {
                if !constructor.is_factory && !constructor.is_const {
                    return None;
                }
                let private = constructor
                    .constructor_name
                    .as_ref()
                    .is_some_and(|name| name.name.starts_with('_'));
                if public && !private {
                    return None;
                }
                if !constructor.is_factory {
                    generative.push(constructor_name(constructor));
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
            let is_candidate =
                creation(initializer, name).is_some_and(|invoked| generative.contains(&invoked));
            if is_candidate {
                candidates.push(initializer.span().start);
            }
        }
    }
    if candidates.len() < 2 {
        return None;
    }

    Some(Candidate {
        name,
        span: class.name.span,
        generative,
        allowed: candidates,
        found: false,
    })
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
