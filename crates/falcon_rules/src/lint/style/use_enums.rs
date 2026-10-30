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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseEnums;

impl Rule for UseEnums {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let candidates: Vec<_> = program
            .declarations()
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates.into_iter().filter(|candidate| !candidate.found).map(|candidate| {
            self.ctx.diagnostic("Class should be an enum.", candidate.span)
        }).collect()
    }

    fn visit_library_directive(&mut self, _node: LibraryDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_part_of_directive(&mut self, _node: PartOfDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_part_directive(&mut self, _node: PartDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_import(&mut self, _node: ImportDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_export(&mut self, _node: ExportDirective<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        for candidate in &mut self.candidates {
            if !candidate.found
                && !matches!(decl, TopLevelDecl::Class(class) if class.name().span() == candidate.span)
                && inherits_from(decl, candidate.name)
            {
                candidate.found = true;
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if !matches!(node, Expr::Paren(_)) {
            for candidate in &mut self.candidates {
                if candidate.found {
                    continue;
                }
                let invoked = creation(node, candidate.name).or(match node {
                    Expr::Call(call) => match call.callee().unparenthesized() {
                        Expr::DotShorthand(dot) => Some(dot.name().name()),
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

fn enum_candidate(class: ClassDecl<'_>) -> Option<Candidate<'_>> {
    if class.modifiers().is_abstract
        || class.modifiers().restriction == Some(ClassRestriction::Sealed)
        || class.extends().is_some_and(|ty| !is_object(ty))
    {
        return None;
    }
    let public = !class.name().name().starts_with('_');
    let mut generative: Vec<&str> = Vec::new();
    if let Some(primary) = class.primary_constructor() {
        let name = primary.constructor_name().map_or("new", |name| name.name());
        if !primary.is_const()
            || (public && !name.starts_with('_'))
            || primary
                .params()
                .params()
                .any(|param| is_reserved(param.name().name()))
        {
            return None;
        }
        generative.push(name);
    }
    for member in class.members() {
        match member {
            ClassMember::Operator(op) if op.op() == "==" => return None,
            ClassMember::Field(field)
                if field
                    .declarators()
                    .any(|declarator| is_reserved(declarator.name().name())) =>
            {
                return None;
            }
            ClassMember::Method(method) if is_reserved(method.name().name()) => return None,
            ClassMember::Getter(getter) if is_reserved(getter.name().name()) => return None,
            ClassMember::Setter(setter) if is_reserved(setter.name().name()) => return None,
            ClassMember::Constructor(constructor) => {
                if !constructor.is_factory() && !constructor.is_const() {
                    return None;
                }
                let private = constructor
                    .constructor_name()
                    .is_some_and(|name| name.name().starts_with('_'));
                if public && !private {
                    return None;
                }
                if !constructor.is_factory() {
                    generative.push(constructor_name(constructor));
                }
            }
            _ => {}
        }
    }

    let name = class.name().name();
    let mut candidates = Vec::new();
    for member in class.members() {
        let ClassMember::Field(field) = member else {
            continue;
        };
        if !field.is_static() || field.keyword() != VarKeyword::Const {
            continue;
        }
        for initializer in field.declarators().filter_map(VarDeclarator::initializer) {
            if creation(initializer, name).is_some_and(|invoked| generative.contains(&invoked)) {
                candidates.push(initializer.span().start);
            }
        }
    }
    if candidates.len() < 2 {
        return None;
    }
    Some(Candidate {
        name,
        span: class.name().span(),
        generative,
        allowed: candidates,
        found: false,
    })
}

/// Members whose presence conflicts with an enum's implicit API.
fn is_reserved(name: &str) -> bool {
    matches!(name, "hashCode" | "index" | "values")
}

fn is_object(ty: DartType<'_>) -> bool {
    matches!(ty, DartType::Named(named)
    if named.type_args().is_empty() && {
        let mut segments = named.segments();
        matches!((segments.next(), segments.next()), (Some(segment), None) if segment.name() == "Object")
    })
}

fn constructor_name(constructor: ConstructorDecl<'_>) -> &str {
    constructor
        .constructor_name()
        .map_or("new", |name| name.name())
}

fn names_class(ty: DartType<'_>, name: &str) -> bool {
    matches!(ty, DartType::Named(named) if {
        let mut segments = named.segments();
        matches!((segments.next(), segments.next()), (Some(segment), None) if segment.name() == name)
    })
}

/// Another class in the file that extends, implements, or mixes in `name`.
fn inherits_from(decl: TopLevelDecl<'_>, name: &str) -> bool {
    let (extends, with_clause, implements) = match decl {
        TopLevelDecl::Class(node) => (node.extends(), node.with_clause(), node.implements()),
        TopLevelDecl::MixinClass(node) => (node.extends(), node.with_clause(), node.implements()),
        _ => return false,
    };
    extends
        .into_iter()
        .chain(with_clause)
        .chain(implements)
        .any(|ty| names_class(ty, name))
}

/// The constructor a creation expression of class `name` invokes.
fn creation<'a>(expr: Expr<'a>, name: &str) -> Option<&'a str> {
    match expr.unparenthesized() {
        Expr::New(new) => {
            let DartType::Named(named) = new.dart_type() else {
                return None;
            };
            let mut segments = named.segments();
            match (
                segments.next(),
                segments.next(),
                segments.next(),
                new.constructor_name(),
            ) {
                (Some(class), None, None, ctor) if class.name() == name => {
                    Some(ctor.map_or("new", |ctor| ctor.name()))
                }
                // `const C.named()` parses `C.named` as a qualified type name.
                (Some(class), Some(ctor), None, None) if class.name() == name => Some(ctor.name()),
                _ => None,
            }
        }
        Expr::Call(call) => {
            // `C<int>(...)` and `C<int>.named(...)` name the class through a
            // generic instantiation.
            let names_class = |expr: Expr<'a>| match expr.unparenthesized() {
                Expr::GenericInstantiation(generic) => {
                    matches!(generic.target().unparenthesized(), Expr::Ident(identifier) if identifier.name() == name)
                }
                Expr::Ident(identifier) => identifier.name() == name,
                _ => false,
            };
            match call.callee().unparenthesized() {
                Expr::Field(field) if names_class(field.object()) => Some(field.field().name()),
                callee if names_class(callee) => Some("new"),
                _ => None,
            }
        }
        _ => None,
    }
}
