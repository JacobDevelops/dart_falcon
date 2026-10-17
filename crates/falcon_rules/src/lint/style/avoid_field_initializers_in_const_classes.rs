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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

const MESSAGE: &str = "Fields in 'const' classes should not have initializers.";

#[derive(Default)]
pub struct AvoidFieldInitializersInConstClasses;

impl Rule for AvoidFieldInitializersInConstClasses {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            constructor: None,
            initializer: None,
            found: false,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    constructor: Option<ConstructorDecl<'a>>,
    initializer: Option<Span>,
    found: bool,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        self.constructor = None;
        let members = match declaration {
            TopLevelDecl::Class(class) => class.members(),
            TopLevelDecl::MixinClass(class) => class.members(),
            _ => return StructuralTraversal::SkipChildren,
        };
        let mut constructors = members.clone().filter_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        });
        let first = constructors.next();
        let second = constructors.next();
        let has_const = first.is_some_and(|constructor| constructor.is_const())
            || second.is_some_and(|constructor| constructor.is_const())
            || constructors.any(|constructor| constructor.is_const());
        if !has_const {
            return StructuralTraversal::SkipChildren;
        }
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.is_static() || field.keyword() != VarKeyword::Final {
                continue;
            }
            for declarator in field.declarators() {
                if declarator.initializer().is_some() {
                    self.diags.push(self.ctx.diagnostic(
                        "avoid-field-initializers-in-const-classes",
                        MESSAGE,
                        declarator.span(),
                    ));
                }
            }
        }
        if second.is_none() {
            self.constructor = first.filter(|constructor| constructor.is_const());
        }
        if self.constructor.is_some() {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if matches!(node, ClassMember::Constructor(_)) {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_constructor_initializer(&mut self, node: ConstructorInitializer<'a>) -> StructuralTraversal {
        if let ConstructorInitializer::FieldInit(initializer) = node {
            self.initializer = Some(initializer.span());
            self.found = false;
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn exit_constructor_initializer(&mut self, _node: ConstructorInitializer<'a>) {
        if let Some(span) = self.initializer.take()
            && !self.found
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-field-initializers-in-const-classes", MESSAGE, span,
            ));
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if self.initializer.is_none() || self.found {
            return StructuralTraversal::SkipChildren;
        }
        if let Expr::Ident(identifier) = node
            && self.constructor.is_some_and(|constructor| constructor.params().params()
                .any(|parameter| parameter.name().name() == identifier.name()))
        {
            self.found = true;
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn visit_primary_constructor(&mut self, _node: PrimaryConstructor<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn declaration_initializers_precede_constructor_initializers() {
        let source = "class A { const A() : x = 1; final int x; final int y = 2; }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics =
            AvoidFieldInitializersInConstClasses.analyze(parsed.tree.program(source), &ctx);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].span.start, source.find("y = 2").unwrap());
        assert_eq!(diagnostics[1].span.start, source.find("x = 1").unwrap());
    }

    #[test]
    fn multiple_constructors_exclude_initializer_lists() {
        let source =
            "class A { const A() : x = 1; const A.named() : x = 2; final int x; final int y = 3; }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics =
            AvoidFieldInitializersInConstClasses.analyze(parsed.tree.program(source), &ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span.start, source.find("y = 3").unwrap());
    }

    #[test]
    fn parameter_reads_do_not_leak_between_initializers() {
        let source = "class Base { const Base(int v); } class A extends Base { final int x, y, z; const A(int v) : assert(v > 0), x = 1, y = v, z = 2, super(v); }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics =
            AvoidFieldInitializersInConstClasses.analyze(parsed.tree.program(source), &ctx);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].span.start, source.find("x = 1").unwrap());
        assert_eq!(diagnostics[1].span.start, source.find("z = 2").unwrap());
    }

    #[test]
    fn mixin_class_initializers_reset_parameter_reference_state() {
        let source = "mixin class A { final int x, y; const A(int v) : x = v, y = 1; }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics =
            AvoidFieldInitializersInConstClasses.analyze(parsed.tree.program(source), &ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span.start, source.find("y = 1").unwrap());
    }
}
