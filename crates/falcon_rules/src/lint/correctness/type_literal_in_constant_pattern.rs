//! Don't use constant patterns with type literals.

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{AnalyzeContext, NameIdentity, QualifiedName, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct TypeLiteralInConstantPattern;
impl Rule for TypeLiteralInConstantPattern {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let enclosing_members = program
            .declarations
            .iter()
            .filter_map(|declaration| {
                let members = crate::lint::declarations::members_of(declaration)?;
                Some((*declaration.span(), member_names(members)))
            })
            .collect();
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            enclosing_members,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    enclosing_members: Vec<(Span, HashSet<String>)>,
}
impl SemanticRuleVisitor for Collector<'_> {
    fn visit_pattern(
        &mut self,
        pattern: &Pattern,
        _matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        let Pattern::Const(constant) = pattern else {
            return;
        };
        let ConstPatternValue::Name(name) = &constant.value else {
            return;
        };
        let Some(first) = name.first() else {
            return;
        };
        if state.environment.is_bound(&first.name)
            || self.enclosing_members.iter().any(|(span, members)| {
                span.start <= first.span.start
                    && first.span.end <= span.end
                    && members.contains(&first.name)
            })
        {
            return;
        }
        if QualifiedName::from_identifiers(name)
            .is_some_and(|name| state.model.name_identity(name) == NameIdentity::Type)
        {
            self.diags.push(self.ctx.diagnostic(
                "type-literal-in-constant-pattern",
                "Use a type pattern or wrap the type literal in 'const (...)'.",
                name.last().expect("non-empty constant name").span,
            ));
        }
    }
}

fn member_names(members: &[ClassMember]) -> HashSet<String> {
    let mut names = HashSet::new();
    for member in members {
        match member {
            ClassMember::Field(field) => {
                names.extend(
                    field
                        .declarators
                        .iter()
                        .map(|declarator| declarator.name.name.clone()),
                );
            }
            ClassMember::Method(method) => {
                names.insert(method.name.name.clone());
            }
            ClassMember::Getter(getter) => {
                names.insert(getter.name.name.clone());
            }
            ClassMember::Setter(setter) => {
                names.insert(setter.name.name.clone());
            }
            ClassMember::Constructor(_) | ClassMember::Operator(_) | ClassMember::Error(_) => {}
        }
    }
    names
}
