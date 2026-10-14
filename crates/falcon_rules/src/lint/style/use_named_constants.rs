//! Prefers a class's predefined static constant to constructing an equal constant.
//!
//! When a class declares `static const zero = A(0);`, writing `const A(0)`
//! elsewhere creates the very same canonicalized object; referring to `A.zero`
//! names the intent. The rule fires on a constant constructor invocation (an
//! explicit `const` creation or a constructor call in a constant context) whose
//! value equals an accessible `static const` field of the constructed class. An
//! invocation inside a field initializer of that same class is exempt, so
//! `static const a = A(); static const b = A();` is fine.
//!
//! falcon compares constant values with its conservative evaluator, so classes
//! declared outside the analyzed project (SDK and package classes) and values it
//! cannot fold are not checked.
//!
//! Adopted from the official `use_named_constants` lint.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, ResolvedType, Rule, evaluate_constant,
    resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_annotation, walk_constructor_decl, walk_enum_decl, walk_expr, walk_field_decl,
    walk_formal_param, walk_local_var, walk_pattern, walk_program,
};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

/// Source ranges that Dart evaluates in a constant context.
#[derive(Default)]
pub(crate) struct ConstContexts {
    spans: Vec<Span>,
    const_constructor_asserts: Vec<Span>,
}

impl ConstContexts {
    pub(crate) fn new(program: &Program) -> Self {
        let mut contexts = Self::default();
        contexts.visit_program(program);
        contexts
    }

    /// Whether `span` lies inside a constant context.
    pub(crate) fn contains(&self, span: &Span) -> bool {
        self.spans
            .iter()
            .any(|context| context.start <= span.start && span.end <= context.end)
    }

    /// Whether `span` lies inside an assert initializer of a `const` constructor.
    pub(crate) fn in_const_constructor_assert(&self, span: &Span) -> bool {
        self.const_constructor_asserts
            .iter()
            .any(|context| context.start <= span.start && span.end <= context.end)
    }
}

impl Visitor for ConstContexts {
    fn visit_program(&mut self, node: &Program) {
        for declaration in &node.declarations {
            if let TopLevelDecl::Variable(variable) = declaration
                && variable.keyword == VarKeyword::Const
            {
                self.spans.extend(
                    variable
                        .declarators
                        .iter()
                        .filter_map(|declarator| declarator.initializer.as_ref())
                        .map(|initializer| *initializer.span()),
                );
            }
        }
        walk_program(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.keyword == VarKeyword::Const {
            self.spans.extend(
                node.declarators
                    .iter()
                    .filter_map(|declarator| declarator.initializer.as_ref())
                    .map(|initializer| *initializer.span()),
            );
        }
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        if node.keyword == VarKeyword::Const {
            self.spans.extend(
                node.declarators
                    .iter()
                    .filter_map(|declarator| declarator.initializer.as_ref())
                    .map(|initializer| *initializer.span()),
            );
        }
        walk_local_var(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        if node.is_const {
            for initializer in &node.initializers {
                if let ConstructorInitializer::Assert { span, .. } = initializer {
                    self.const_constructor_asserts.push(*span);
                }
            }
        }
        walk_constructor_decl(self, node);
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        self.spans.extend(
            node.variants
                .iter()
                .filter_map(|variant| variant.args.as_ref())
                .map(|args| args.span),
        );
        walk_enum_decl(self, node);
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        if let Some(args) = &node.args {
            self.spans.push(args.span);
        }
        walk_annotation(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if let Some(default) = &node.default_value {
            self.spans.push(*default.span());
        }
        walk_formal_param(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        self.spans.push(*node.span());
        walk_pattern(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::List {
            is_const: true,
            span,
            ..
        }
        | Expr::Map {
            is_const: true,
            span,
            ..
        }
        | Expr::Set {
            is_const: true,
            span,
            ..
        }
        | Expr::Record {
            is_const: true,
            span,
            ..
        }
        | Expr::New {
            is_const: true,
            span,
            ..
        }
        | Expr::DotShorthand {
            is_const: true,
            span,
            ..
        } = node
        {
            self.spans.push(*span);
        }
        walk_expr(self, node);
    }
}

#[derive(Default)]
pub struct UseNamedConstants;

impl Rule for UseNamedConstants {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            contexts: ConstContexts::new(program),
            own_field_initializers: own_field_initializers(program),
            local_classes: local_classes(program),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

/// Field-initializer ranges of each class declared in this file.
fn own_field_initializers(program: &Program) -> Vec<(String, Span)> {
    let mut ranges = Vec::new();
    for declaration in &program.declarations {
        let (name, members) = match declaration {
            TopLevelDecl::Class(class) => (&class.name.name, &class.members),
            TopLevelDecl::ExtensionType(extension) => (&extension.name.name, &extension.members),
            _ => continue,
        };
        for member in members {
            if let ClassMember::Field(field) = member {
                ranges.extend(
                    field
                        .declarators
                        .iter()
                        .filter_map(|declarator| declarator.initializer.as_ref())
                        .map(|initializer| (name.clone(), *initializer.span())),
                );
            }
        }
    }
    ranges
}

fn local_classes(program: &Program) -> Vec<String> {
    program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            TopLevelDecl::Class(class) => Some(class.name.name.clone()),
            TopLevelDecl::ExtensionType(extension) => Some(extension.name.name.clone()),
            _ => None,
        })
        .collect()
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    contexts: ConstContexts,
    own_field_initializers: Vec<(String, Span)>,
    local_classes: Vec<String>,
}

impl Collector<'_> {
    fn constructed_class(
        &self,
        node: &Expr,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        match node {
            Expr::New {
                is_const: true,
                dart_type,
                ..
            } => match state.model.resolve_type(dart_type) {
                ResolvedType::Interface { identity, .. } => Some(identity),
                _ => None,
            },
            Expr::Call { callee, .. } if self.contexts.contains(node.span()) => {
                resolve_call_constructor(callee, &state.model, true).map(|(identity, _)| identity)
            }
            _ => None,
        }
    }

    fn matching_constant(
        &self,
        node: &Expr,
        identity: &DeclarationIdentity,
        state: &SemanticState<'_>,
    ) -> Option<String> {
        let facts = state.signatures.declaration(identity)?;
        if facts.static_consts.is_empty() {
            return None;
        }
        let empty = HashMap::new();
        let value = evaluate_constant(node, None, &empty, &state.model, &state.signatures)?;
        if !matches!(value, ConstantValue::Instance(..)) {
            return None;
        }
        let fields: HashMap<&str, &Expr> = facts
            .static_consts
            .iter()
            .map(|field| (field.name.as_str(), &field.initializer))
            .collect();
        let declared_here = self
            .local_classes
            .iter()
            .any(|name| name == identity.name());
        facts
            .static_consts
            .iter()
            .filter(|field| declared_here || !field.name.starts_with('_'))
            .find(|field| {
                evaluate_constant(
                    &field.initializer,
                    Some(identity),
                    &fields,
                    &state.model,
                    &state.signatures,
                )
                .as_ref()
                    == Some(&value)
            })
            .map(|field| format!("{}.{}", identity.name(), field.name))
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Some(identity) = self.constructed_class(node, state) else {
            return;
        };
        let span = node.span();
        let in_own_field = self.own_field_initializers.iter().any(|(class, range)| {
            class == identity.name() && range.start <= span.start && span.end <= range.end
        });
        if in_own_field {
            return;
        }
        if let Some(constant) = self.matching_constant(node, &identity, state) {
            self.diags.push(self.ctx.diagnostic(
                "use-named-constants",
                format!(
                    "Use the constant '{constant}' rather than a constructor returning the same object."
                ),
                *span,
            ));
        }
    }
}
