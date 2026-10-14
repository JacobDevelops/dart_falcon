//! Shared leak detection for `cancel-subscriptions` and `close-sinks`, after
//! the official linter's `LeakDetectorProcessors`.
//!
//! A local variable or field whose type implements one of a rule's resource
//! types is reported unless its container — the enclosing function body for a
//! local, the whole file for a field — holds a "valid use": calling or tearing
//! off the release method on it (for a field, invoking any method on it,
//! including through `?.` or a cascade), passing it as a positional argument
//! to a call, returning it (locals), handing it to a cascade method call,
//! assigning it to or from another variable, or initializing a field from a
//! constructor.
//!
//! falcon has no element identities, so references are matched by name inside
//! the container. A same-named variable elsewhere in the container can only add
//! valid uses, which keeps the rules silent rather than wrong.

use falcon_analyze::{AnalyzeContext, ResolvedType, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_constructor_decl, walk_expr, walk_formal_param, walk_program, walk_stmt,
};

use crate::lint::function_bodies::{FunctionBodies, contains};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

/// A resource type and the method that releases it.
pub(crate) struct Release {
    pub(crate) library: &'static str,
    pub(crate) type_name: &'static str,
    pub(crate) method: &'static str,
}

pub(crate) struct LeakRule {
    pub(crate) name: &'static str,
    pub(crate) message: &'static str,
    pub(crate) releases: &'static [Release],
}

impl LeakRule {
    pub(crate) fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut locals = LocalCandidates {
            rule: self,
            bodies: FunctionBodies::new(program),
            candidates: Vec::new(),
        };
        let state = visit_program(&mut locals, program, state);
        let mut fields = FieldCandidates {
            rule: self,
            state: &state,
            file: Span::new(0, ctx.source.len()),
            candidates: locals.candidates,
        };
        walk_program(&mut fields, program);
        let mut uses = ValidUses {
            candidates: fields.candidates,
        };
        walk_program(&mut uses, program);
        uses.candidates
            .iter()
            .filter(|candidate| !candidate.valid)
            .map(|candidate| ctx.diagnostic(self.name, self.message, candidate.declarator))
            .collect()
    }

    /// The release methods that apply to a variable of type `ty`.
    fn methods(&self, ty: &ResolvedType, state: &SemanticState<'_>) -> Vec<&'static str> {
        self.releases
            .iter()
            .filter(|release| {
                state
                    .signatures
                    .implements_sdk(ty, release.library, release.type_name)
                    == Truth::Yes
            })
            .map(|release| release.method)
            .collect()
    }

    fn candidate(
        &self,
        declarator: &VarDeclarator,
        declared: Option<&DartType>,
        kind: VariableKind,
        container: Span,
        state: &SemanticState<'_>,
    ) -> Option<Candidate> {
        // `var a = b;` only aliases a resource tracked through `b`.
        if matches!(declarator.initializer, Some(Expr::Ident(_))) {
            return None;
        }
        let ty = match (declared, &declarator.initializer) {
            (Some(declared), _) => state
                .model
                .resolve_type_in(declared, &state.type_parameters),
            (None, Some(initializer)) => state.infer(initializer),
            (None, None) => return None,
        };
        let methods = self.methods(&ty, state);
        (!methods.is_empty()).then(|| Candidate {
            name: declarator.name.name.clone(),
            declarator: declarator.span,
            container,
            kind,
            methods,
            valid: false,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariableKind {
    Local,
    Field,
}

struct Candidate {
    name: String,
    declarator: Span,
    container: Span,
    kind: VariableKind,
    methods: Vec<&'static str>,
    valid: bool,
}

impl Candidate {
    /// `name`, `name!`, or `this.name`.
    fn is_reference(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Ident(identifier) => identifier.name == self.name,
            Expr::NullAssert { operand, .. } => {
                matches!(operand.as_ref(), Expr::Ident(identifier) if identifier.name == self.name)
            }
            Expr::Field { object, field, .. } => {
                field.name == self.name && matches!(object.as_ref(), Expr::This { .. })
            }
            _ => false,
        }
    }

    fn is_name(&self, expr: &Expr) -> bool {
        matches!(expr, Expr::Ident(identifier) if identifier.name == self.name)
    }

    fn passed_in(&self, args: &ArgList) -> bool {
        args.positional
            .iter()
            .any(|argument| self.is_name(argument))
    }

    fn is_valid_use(&self, node: &Expr) -> bool {
        match node {
            Expr::Assign { target, value, .. } => {
                self.is_name(value)
                    || (matches!(value.as_ref(), Expr::Ident(_))
                        && match target.as_ref() {
                            Expr::Ident(identifier) => identifier.name == self.name,
                            Expr::Field { field, .. } => field.name == self.name,
                            _ => false,
                        })
            }
            Expr::Call {
                callee, args, span, ..
            } => {
                let invoked = matches!(callee.as_ref(), Expr::Field { object, field, .. }
                    if (self.is_reference(object) && self.invocation_counts(&field.name))
                        || (self.methods.contains(&field.name.as_str())
                            && contains(&self.declarator, span)));
                invoked || self.passed_in(args)
            }
            // A tear-off of the release method.
            Expr::Field { object, field, .. } => {
                self.is_name(object) && self.methods.contains(&field.name.as_str())
            }
            Expr::Cascade {
                object, sections, ..
            } => sections.iter().any(|section| match section.ops.first() {
                Some(CascadeOp::Call(method, _, args)) => {
                    self.is_name(object)
                        || (self.is_reference(object) && self.invocation_counts(&method.name))
                        || self.passed_in(args)
                }
                _ => false,
            }),
            _ => false,
        }
    }

    /// Whether invoking `method` on the variable is a valid use: any method
    /// for a field, only a release method for a local.
    fn invocation_counts(&self, method: &str) -> bool {
        self.kind == VariableKind::Field || self.methods.contains(&method)
    }
}

struct LocalCandidates<'r> {
    rule: &'r LeakRule,
    bodies: FunctionBodies,
    candidates: Vec<Candidate>,
}

impl SemanticRuleVisitor for LocalCandidates<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::LocalVar(declaration) = node else {
            return;
        };
        let Some(container) = self
            .bodies
            .enclosing(&declaration.span)
            .map(|body| body.span)
        else {
            return;
        };
        for declarator in &declaration.declarators {
            self.candidates.extend(self.rule.candidate(
                declarator,
                declaration.var_type.as_ref(),
                VariableKind::Local,
                container,
                state,
            ));
        }
    }
}

struct FieldCandidates<'r, 's, 'a> {
    rule: &'r LeakRule,
    state: &'s SemanticState<'a>,
    file: Span,
    candidates: Vec<Candidate>,
}

impl Visitor for FieldCandidates<'_, '_, '_> {
    fn visit_field_decl(&mut self, node: &FieldDecl) {
        for declarator in &node.declarators {
            self.candidates.extend(self.rule.candidate(
                declarator,
                node.field_type.as_ref(),
                VariableKind::Field,
                self.file,
                self.state,
            ));
        }
    }
}

struct ValidUses {
    candidates: Vec<Candidate>,
}

impl ValidUses {
    fn mark(&mut self, span: &Span, valid: impl Fn(&Candidate) -> bool) {
        for candidate in &mut self.candidates {
            if !candidate.valid && contains(&candidate.container, span) && valid(candidate) {
                candidate.valid = true;
            }
        }
    }
}

impl Visitor for ValidUses {
    fn visit_expr(&mut self, node: &Expr) {
        self.mark(node.span(), |candidate| candidate.is_valid_use(node));
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Return(statement) = node
            && let Some(value) = &statement.value
        {
            self.mark(&statement.span, |candidate| {
                candidate.kind == VariableKind::Local && candidate.is_name(value)
            });
        }
        walk_stmt(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        for initializer in &node.initializers {
            if let ConstructorInitializer::FieldInit { field, span, .. } = initializer {
                self.mark(span, |candidate| {
                    candidate.kind == VariableKind::Field && candidate.name == field.name
                });
            }
        }
        walk_constructor_decl(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if matches!(node.initializing, Some(InitializingFormal::This)) {
            self.mark(&node.span, |candidate| {
                candidate.kind == VariableKind::Field && candidate.name == node.name.name
            });
        }
        walk_formal_param(self, node);
    }
}
