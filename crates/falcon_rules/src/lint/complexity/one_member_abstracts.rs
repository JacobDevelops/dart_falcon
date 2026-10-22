//! Flags abstract classes whose only member is a single abstract method.
//!
//! Such a class is just a function type with extra ceremony; a top-level
//! function or a function-typed parameter does the same job. Classes that
//! extend, mix in, or implement another type, or that declare any field,
//! getter, or setter, are left alone because they carry more than one member.
//!
//! ```dart
//! abstract class Predicate { bool test(Object o); } // flagged
//! typedef Predicate = bool Function(Object o);      // ok
//! ```
//!
//! Adopted from the official `one_member_abstracts` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct OneMemberAbstracts;

impl Rule for OneMemberAbstracts {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let candidate = match declaration {
            TopLevelDecl::Class(class)
                if class.modifiers().is_abstract
                    && class.extends().is_none()
                    && class.with_clause().is_empty()
                    && class.implements().is_empty() =>
            {
                Some((class.name(), class.members()))
            }
            TopLevelDecl::MixinClass(class)
                if class.is_abstract()
                    && class.extends().is_none()
                    && class.with_clause().is_empty()
                    && class.implements().is_empty() =>
            {
                Some((class.name(), class.members()))
            }
            _ => None,
        };
        if let Some((name, members)) = candidate
            && let Some(method) = single_abstract_method(members)
        {
            self.diags.push(self.ctx.diagnostic(
                format!(
                    "Make `{method}` a top-level function and remove this unnecessary abstract class."
                ),
                name.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}

/// The name of the class's only method when it is abstract and the class has
/// no fields or accessors; operators count as methods, constructors do not.
fn single_abstract_method<'a>(members: AstChildren<'a, ClassMember<'a>>) -> Option<&'a str> {
    let mut method = None;
    for member in members {
        let (name, abstract_body) = match member {
            ClassMember::Constructor(_) | ClassMember::PrimaryConstructorBody(_) => continue,
            ClassMember::Method(m) => (m.name().name(), m.body().is_none() && !m.is_external()),
            ClassMember::Operator(o) => (o.op(), o.body().is_none() && !o.is_external()),
            ClassMember::Field(_)
            | ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Error(_) => return None,
        };
        if method.is_some() {
            return None;
        }
        method = Some((name, abstract_body));
    }
    let (name, abstract_body) = method?;
    abstract_body.then_some(name)
}
