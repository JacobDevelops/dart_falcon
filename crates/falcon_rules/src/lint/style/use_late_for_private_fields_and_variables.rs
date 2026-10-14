//! Flags a private nullable field or top-level variable that is only ever
//! assigned non-null values and read through `!`, so it could be `late` with a
//! non-nullable type.
//!
//! When every read of a private variable is a null assertion and every write
//! stores a non-nullable value, the nullable type only exists to defer
//! initialization; `late` expresses that and moves the null check into the
//! runtime. Candidates are private (underscore) top-level variables and private
//! fields (instance or static), plus every field of a private extension; instance
//! fields of extension types, fields of classes with a `const` constructor, and
//! all enum members are skipped, as are `late`, `const`, `external`, and
//! `abstract` declarations. Any other use — a plain read, a `this._x`
//! initializing formal, a `: _x = ...` initializer, a compound or `??=`
//! assignment, or an assignment whose value is not proven non-nullable —
//! disqualifies every declaration with that name.
//!
//! Library awareness (suppress-only): any mention of the name in a sibling part
//! file disqualifies it, and a file whose library cannot be fully scanned (an
//! unresolved part, or `part`/`part of` directives without library context)
//! reports nothing.
//!
//! ```dart
//! int? _i; // flagged
//! void f() {
//!   _i = 1;
//!   print(_i!);
//! }
//! ```
//!
//! Adopted from the official `use_late_for_private_fields_and_variables` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth, TypeParameterScope};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::semantic_type_operations::nullability;

#[derive(Default)]
pub struct UseLateForPrivateFieldsAndVariables;

impl Rule for UseLateForPrivateFieldsAndVariables {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let has_part_directives =
            !program.part_directives.is_empty() || program.part_of_directive.is_some();
        match ctx.library {
            Some(library) if library.has_unresolved_parts() => return Vec::new(),
            None if has_part_directives => return Vec::new(),
            _ => {}
        }
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let candidates = collect_candidates(program, &state, ctx);
        if candidates.is_empty() {
            return Vec::new();
        }
        let names: HashSet<&str> = candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect();

        let mut safe = SafeUses {
            ctx,
            names: &names,
            spans: HashSet::new(),
        };
        visit_program(&mut safe, program, state);

        let mut mentions = Mentions {
            names: &names,
            found: Vec::new(),
        };
        mentions.visit_program(program);
        let mut disqualified: HashSet<&str> = HashSet::new();
        for (name, start) in &mentions.found {
            let declaration = candidates
                .iter()
                .any(|candidate| candidate.name_start == *start);
            if !declaration && !safe.spans.contains(start) {
                disqualified.insert(*name);
            }
        }
        if let Some(library) = ctx.library {
            mentions.found.clear();
            for sibling in library.siblings() {
                mentions.visit_program(sibling);
            }
            disqualified.extend(mentions.found.iter().map(|(name, _)| *name));
        }

        candidates
            .iter()
            .filter(|candidate| !disqualified.contains(candidate.name.as_str()))
            .map(|candidate| {
                ctx.diagnostic(
                    "use-late-for-private-fields-and-variables",
                    "Use 'late' for private members with a non-nullable type.",
                    candidate.span,
                )
            })
            .collect()
    }
}

struct Candidate {
    name: String,
    name_start: usize,
    span: Span,
}

fn collect_candidates(
    program: &Program,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for declaration in &program.declarations {
        let (type_params, members, all_fields, static_only) = match declaration {
            TopLevelDecl::Variable(variable) => {
                if !variable.is_late
                    && !variable.is_external
                    && variable.keyword != VarKeyword::Const
                {
                    let scope = TypeParameterScope::default();
                    for declarator in &variable.declarators {
                        if is_private(&declarator.name.name)
                            && potentially_nullable(
                                variable.var_type.as_ref(),
                                declarator,
                                &scope,
                                state,
                                ctx,
                            )
                        {
                            candidates.push(candidate(declarator));
                        }
                    }
                }
                continue;
            }
            TopLevelDecl::Class(class) => {
                if has_const_constructor(&class.members) {
                    continue;
                }
                (&class.type_params, &class.members, false, false)
            }
            TopLevelDecl::MixinClass(class) => {
                if has_const_constructor(&class.members) {
                    continue;
                }
                (&class.type_params, &class.members, false, false)
            }
            TopLevelDecl::Mixin(mixin) => (&mixin.type_params, &mixin.members, false, false),
            TopLevelDecl::Extension(extension) => {
                let private = extension
                    .name
                    .as_ref()
                    .is_none_or(|name| is_private(&name.name));
                (&extension.type_params, &extension.members, private, false)
            }
            TopLevelDecl::ExtensionType(extension) => {
                (&extension.type_params, &extension.members, false, true)
            }
            _ => continue,
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_params, &state.model);
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.is_late
                || field.is_external
                || field.is_abstract
                || field.keyword == VarKeyword::Const
                || (static_only && !field.is_static)
            {
                continue;
            }
            for declarator in &field.declarators {
                if (all_fields || is_private(&declarator.name.name))
                    && potentially_nullable(
                        field.field_type.as_ref(),
                        declarator,
                        &scope,
                        state,
                        ctx,
                    )
                {
                    candidates.push(candidate(declarator));
                }
            }
        }
    }
    candidates
}

fn candidate(declarator: &VarDeclarator) -> Candidate {
    Candidate {
        name: declarator.name.name.clone(),
        name_start: declarator.name.span.start,
        span: declarator.span,
    }
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

fn has_const_constructor(members: &[ClassMember]) -> bool {
    members.iter().any(
        |member| matches!(member, ClassMember::Constructor(constructor) if constructor.is_const),
    )
}

/// Whether the declared type is proven not to be non-nullable (the analyzer's
/// `!isNonNullable`), including `dynamic` and potentially nullable type
/// parameters. An unresolved type is never a candidate.
fn potentially_nullable(
    declared: Option<&DartType>,
    declarator: &VarDeclarator,
    scope: &TypeParameterScope,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext,
) -> bool {
    let ty = match (declared, &declarator.initializer) {
        (Some(ty), _) => state.model.resolve_type_in(ty, scope),
        // Without a type or initializer the variable is `dynamic`; a `null`
        // initializer also infers `dynamic`.
        (None, None | Some(Expr::NullLit { .. })) => ResolvedType::Dynamic,
        (None, Some(initializer)) => state.infer(initializer),
    };
    match &ty {
        ResolvedType::Dynamic => true,
        ResolvedType::TypeParameter {
            bound,
            nullable: false,
            ..
        } => {
            matches!(bound.as_ref(), ResolvedType::Dynamic)
                || nullability(bound, &state.signatures, ctx.types) == Truth::Yes
        }
        _ => nullability(&ty, &state.signatures, ctx.types) == Truth::Yes,
    }
}

/// Records the identifier spans of uses that keep a candidate lateable: a `!`
/// operand, or the target of a plain `=` of a proven non-nullable value.
struct SafeUses<'a> {
    ctx: &'a AnalyzeContext<'a>,
    names: &'a HashSet<&'a str>,
    spans: HashSet<usize>,
}

impl SafeUses<'_> {
    fn record(&mut self, expression: &Expr) {
        let identifier = match expression {
            Expr::Ident(identifier) => identifier,
            Expr::Field { field, .. } => field,
            _ => return,
        };
        if self.names.contains(identifier.name.as_str()) {
            self.spans.insert(identifier.span.start);
        }
    }

    fn record_assignment(
        &mut self,
        target: &Expr,
        op: AssignOp,
        value: &Expr,
        state: &SemanticState<'_>,
    ) {
        if op == AssignOp::Eq
            && nullability(&state.infer(value), &state.signatures, self.ctx.types) == Truth::No
        {
            self.record(target);
        }
    }
}

impl SemanticRuleVisitor for SafeUses<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::NullAssert { operand, .. } => self.record(operand),
            Expr::Assign {
                target, op, value, ..
            } => self.record_assignment(target, *op, value, state),
            Expr::Cascade { sections, .. } => {
                for section in sections {
                    if let [CascadeOp::Assign(target, op, value)] = section.ops.as_slice() {
                        self.record_assignment(target, *op, value, state);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Every occurrence of a candidate name, declarations included.
struct Mentions<'a> {
    names: &'a HashSet<&'a str>,
    found: Vec<(&'a str, usize)>,
}

impl<'a> Mentions<'a> {
    fn record(&mut self, identifier: &Identifier) {
        if let Some(name) = self.names.get(identifier.name.as_str()) {
            self.found.push((name, identifier.span.start));
        }
    }
}

impl Visitor for Mentions<'_> {
    fn visit_identifier(&mut self, node: &Identifier) {
        self.record(node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Field { field, .. } => self.record(field),
            Expr::DotShorthand { name, .. } => self.record(name),
            _ => {}
        }
        walk_expr(self, node);
    }
}
