//! Avoid method calls or property accesses on a `dynamic` target.
//!
//! Members reached through `dynamic` are looked up at runtime: typos and
//! signature mismatches surface as `NoSuchMethodError` instead of compile
//! errors, and the dispatch is slower:
//!
//! ```dart
//! void log(dynamic event) {
//!   print(event.name); // BAD
//! }
//!
//! void log(Event event) {
//!   print(event.name); // GOOD
//! }
//! ```
//!
//! Reported targets: member accesses, method calls, index operations,
//! user-definable binary operators (not `==`/`!=`), prefix and postfix
//! operators, and compound assignments on a `dynamic` receiver, plus calls of a
//! `dynamic` or `Function`-typed variable. `hashCode`, `runtimeType`,
//! `toString`, and `noSuchMethod` — members every object has — are allowed,
//! as are explicit `as dynamic` casts and `x!`.
//!
//! falcon does not infer types for untyped closure parameters or untyped
//! collection literals, so only expressions whose `dynamic` type is proven are
//! reported: parameters and variables explicitly declared `dynamic` (or
//! `Function`) — every same-named declaration in the file must agree — values
//! read from such variables or from explicitly typed `Map<K, dynamic>` /
//! `List<dynamic>` variables, and results of dynamic member accesses on them.
//! Members inherited from declarations in other files stay silent. falcon has
//! no flow analysis, so a name that could be type-promoted anywhere in its
//! outermost enclosing declaration — tested with `is`/`is!`, cast with `as`,
//! used as a pattern-match subject, or assigned directly or by a pattern — is
//! left alone at every use there.
//!
//! Adopted from the official `avoid_dynamic_calls` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_expr, walk_formal_param, walk_function_decl, walk_local_var,
    walk_pattern, walk_program, walk_stmt, walk_top_level_decl,
};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

/// Members that exist on every object, even one typed `Object?`.
const ALLOWED_MEMBERS: [&str; 4] = ["hashCode", "noSuchMethod", "runtimeType", "toString"];

#[derive(Default)]
pub struct AvoidDynamicCalls;

impl Rule for AvoidDynamicCalls {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut declarations = Declarations::default();
        walk_program(&mut declarations, program);
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            declarations: declarations.names,
            top_level: declarations.top_level,
            promotable: PromotableNames::of(program),
            callees: HashSet::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

/// What a declaration's written type proves about the values it holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Declared {
    Dynamic,
    /// `Function` or `Function?`.
    Function,
    /// `Map<K, dynamic>`.
    MapOfDynamic,
    /// `List<dynamic>`.
    ListOfDynamic,
    /// Any other type, no annotation, or not a variable at all.
    Other,
}

impl Declared {
    fn of(ty: Option<&DartType>) -> Self {
        match ty {
            Some(DartType::Dynamic { .. }) => Self::Dynamic,
            Some(ty) if named(ty, "Function", 0) => Self::Function,
            Some(ty) if named_with_dynamic_last(ty, "Map", 2) => Self::MapOfDynamic,
            Some(ty) if named_with_dynamic_last(ty, "List", 1) => Self::ListOfDynamic,
            _ => Self::Other,
        }
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Every declaration of each name in the file.
    declarations: HashMap<String, Vec<Declared>>,
    top_level: HashSet<String>,
    promotable: PromotableNames,
    /// Member accesses already handled as the callee of a call.
    callees: HashSet<(usize, usize)>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-dynamic-calls",
            "Method invocation or property access on a 'dynamic' target.",
            span,
        ));
    }

    /// Reports `target` when it is provably `dynamic`; explicit casts are allowed.
    fn report_if_dynamic(&mut self, target: &Expr, state: &SemanticState<'_>) -> bool {
        if matches!(target, Expr::As { .. }) || !self.is_dynamic(target, state) {
            return false;
        }
        self.report(*target.span());
        true
    }

    /// Whether every declaration `identifier` may refer to here is `accepted`:
    /// it must be bound in scope (a parameter, local, or field of the enclosing
    /// declaration in this file), or a top-level variable read outside any
    /// type body, and must not be possibly promoted at this use.
    fn all_declared(
        &self,
        identifier: &Identifier,
        state: &SemanticState<'_>,
        accepted: &[Declared],
    ) -> bool {
        let name = identifier.name.as_str();
        let visible = state.environment.is_bound(name)
            || (state.enclosing_type.is_none() && self.top_level.contains(name));
        visible
            && self.declarations.get(name).is_some_and(|declared| {
                !declared.is_empty() && declared.iter().all(|kind| accepted.contains(kind))
            })
            && !self.promotable.may_promote(identifier)
    }

    /// Whether `expr` is provably `dynamic`: `dynamic` originates at a declared
    /// variable, an `as dynamic` cast, or an element read, and flows outward
    /// through dynamic member accesses, calls, index reads, and operators.
    fn is_dynamic(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        let mut current = expr;
        loop {
            if self.originates_dynamic(current, state) {
                return true;
            }
            match dynamic_receiver(current) {
                Some(receiver) => current = receiver,
                None => return false,
            }
        }
    }

    /// An expression whose `dynamic` type does not depend on a receiver.
    fn originates_dynamic(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        match expr {
            Expr::Ident(identifier) => self.all_declared(identifier, state, &[Declared::Dynamic]),
            Expr::As { dart_type, .. } => matches!(dart_type, DartType::Dynamic { .. }),
            Expr::Index { object, .. } => self.has_dynamic_elements(object, state),
            Expr::Call { callee, .. } => self.is_function_typed(callee, state),
            _ => false,
        }
    }

    fn is_function_typed(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        let Expr::Ident(identifier) = expr else {
            return false;
        };
        if !self.all_declared(identifier, state, &[Declared::Dynamic, Declared::Function]) {
            return false;
        }
        let inferred = state.infer(expr);
        matches!(inferred, ResolvedType::Dynamic) || inferred.interface("dart:core", "Function")
    }

    /// A variable explicitly typed `Map<K, dynamic>` or `List<dynamic>`.
    fn has_dynamic_elements(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        let Expr::Ident(identifier) = expr else {
            return false;
        };
        let inferred = state.infer(expr);
        (inferred.interface("dart:core", "Map")
            && self.all_declared(identifier, state, &[Declared::MapOfDynamic]))
            || (inferred.interface("dart:core", "List")
                && self.all_declared(identifier, state, &[Declared::ListOfDynamic]))
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Assign {
                target, op, span, ..
            } => {
                if !matches!(op, AssignOp::Eq | AssignOp::NullCoalesceEq)
                    && self.is_dynamic(target, state)
                {
                    self.report(*span);
                }
            }
            Expr::Binary { op, left, .. } => {
                if is_user_definable(*op) && !matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) {
                    self.report_if_dynamic(left, state);
                }
            }
            Expr::Unary { operand, .. } | Expr::PostfixIncDec { operand, .. } => {
                self.report_if_dynamic(operand, state);
            }
            Expr::Index { object, .. } => {
                self.report_if_dynamic(object, state);
            }
            Expr::Field {
                object,
                field,
                span,
                ..
            } => {
                if !self.callees.contains(&(span.start, span.end))
                    && !ALLOWED_MEMBERS.contains(&field.name.as_str())
                {
                    self.report_if_dynamic(object, state);
                }
            }
            Expr::Call { callee, args, .. } => match callee.as_ref() {
                Expr::Field {
                    object,
                    field,
                    span,
                    ..
                } => {
                    self.callees.insert((span.start, span.end));
                    if !is_universal_call(&field.name, args) {
                        self.report_if_dynamic(object, state);
                    }
                }
                other => {
                    if !matches!(other, Expr::As { .. })
                        && (self.is_dynamic(other, state) || self.is_function_typed(other, state))
                    {
                        self.report(*other.span());
                    }
                }
            },
            Expr::Cascade {
                object, sections, ..
            } => {
                let dynamic_section = sections.iter().any(|section| match section.ops.first() {
                    Some(CascadeOp::Call(name, _, args)) => !is_universal_call(&name.name, args),
                    Some(CascadeOp::Field { name, .. }) => {
                        !ALLOWED_MEMBERS.contains(&name.name.as_str())
                    }
                    Some(CascadeOp::Index { .. }) => true,
                    Some(CascadeOp::Assign(target, ..)) => !matches!(
                        target.as_ref(),
                        Expr::Ident(name) if ALLOWED_MEMBERS.contains(&name.name.as_str())
                    ),
                    None => false,
                });
                if dynamic_section {
                    self.report_if_dynamic(object, state);
                }
            }
            _ => {}
        }
    }
}

/// `toString()` and `noSuchMethod(invocation)` exist on every object.
fn is_universal_call(name: &str, args: &ArgList) -> bool {
    (name == "toString" && args.positional.is_empty() && args.named.is_empty())
        || (name == "noSuchMethod" && args.positional.len() == 1 && args.named.is_empty())
}

/// The operand whose `dynamic` type a dynamic operation on it passes on, for
/// operations whose result is `dynamic` when that operand is.
fn dynamic_receiver(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::NullAssert { operand, .. } => Some(operand),
        Expr::Field { object, field, .. } if !ALLOWED_MEMBERS.contains(&field.name.as_str()) => {
            Some(object)
        }
        Expr::Index { object, .. } => Some(object),
        Expr::Call { callee, args, .. } => match callee.as_ref() {
            Expr::Field { object, field, .. } => {
                (!is_universal_call(&field.name, args)).then_some(object.as_ref())
            }
            other => Some(other),
        },
        Expr::Binary { op, left, .. }
            if is_user_definable(*op) && !matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) =>
        {
            Some(left)
        }
        _ => None,
    }
}

fn is_user_definable(op: BinaryOp) -> bool {
    !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce)
}

/// An unprefixed `name<...>` with `arity` type arguments.
fn named(ty: &DartType, name: &str, arity: usize) -> bool {
    matches!(ty, DartType::Named(named)
        if named.type_args.len() == arity
            && matches!(named.segments.as_slice(), [segment] if segment.name == name))
}

fn named_with_dynamic_last(ty: &DartType, name: &str, arity: usize) -> bool {
    named(ty, name, arity)
        && matches!(ty, DartType::Named(named)
            if matches!(named.type_args.last(), Some(DartType::Dynamic { .. })))
}

/// Every name a file declares, classified by its written type.
#[derive(Default)]
struct Declarations {
    names: HashMap<String, Vec<Declared>>,
    top_level: HashSet<String>,
}

impl Declarations {
    fn add(&mut self, name: &Identifier, ty: Option<&DartType>) {
        self.names
            .entry(name.name.clone())
            .or_default()
            .push(Declared::of(ty));
    }

    fn add_declarators(&mut self, declarators: &[VarDeclarator], ty: Option<&DartType>) {
        for declarator in declarators {
            self.add(&declarator.name, ty);
        }
    }

    fn add_for_header(&mut self, header: &ForHeader) {
        if let Some(ForInit::ForIn { var_type, name, .. }) = &header.init {
            self.add(name, var_type.as_ref());
        }
    }

    /// Declares the loop variables of collection `for` elements.
    fn add_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                self.add_element(then_elem);
                if let Some(else_elem) = else_elem {
                    self.add_element(else_elem);
                }
            }
            CollectionElement::For {
                header, element, ..
            } => {
                self.add_for_header(header);
                self.add_element(element);
            }
            _ => {}
        }
    }

    fn add_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::If {
                then_entry,
                else_entry,
                ..
            } => {
                self.add_map_element(then_entry);
                if let Some(else_entry) = else_entry {
                    self.add_map_element(else_entry);
                }
            }
            MapElement::For { header, entry, .. } => {
                self.add_for_header(header);
                self.add_map_element(entry);
            }
            _ => {}
        }
    }
}

impl Visitor for Declarations {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            for declarator in &variable.declarators {
                self.top_level.insert(declarator.name.name.clone());
            }
            self.add_declarators(&variable.declarators, variable.var_type.as_ref());
        }
        walk_top_level_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.add(&node.name, None);
        walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        match node {
            ClassMember::Field(field) => {
                self.add_declarators(&field.declarators, field.field_type.as_ref());
            }
            ClassMember::Method(method) => self.add(&method.name, None),
            ClassMember::Getter(getter) => self.add(&getter.name, None),
            ClassMember::Setter(setter) => self.add(&setter.name, None),
            _ => {}
        }
        walk_class_member(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        let ty = if node.function_params.is_some() {
            None
        } else {
            node.param_type.as_ref()
        };
        self.add(&node.name, ty);
        walk_formal_param(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.add_declarators(&node.declarators, node.var_type.as_ref());
        walk_local_var(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalFunc(function) => self.add(&function.name, None),
            Stmt::TryCatch(statement) => {
                for catch in &statement.catches {
                    for name in catch.exception_var.iter().chain(&catch.stack_trace_var) {
                        self.add(name, None);
                    }
                }
            }
            Stmt::For(statement) => self.add_for_header(&statement.header),
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.add_element(element);
                }
            }
            Expr::Map { elements, .. } => {
                for element in elements {
                    self.add_map_element(element);
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, .. } = node {
            self.add(name, None);
        }
        walk_pattern(self, node);
    }
}

/// Variables flow analysis may promote, each recorded with the outermost
/// declaration (function, method, constructor, accessor, or variable) that
/// contains a promoting position: an `is`/`is!` test, a non-`dynamic` `as`
/// cast, a pattern-match subject, or a direct or pattern assignment. falcon has
/// no flow analysis, so such a name may hold a promoted type at any of its uses
/// in that declaration.
#[derive(Default)]
pub(super) struct PromotableNames {
    scope: Option<Span>,
    in_pattern_assignment: bool,
    names: HashMap<String, Vec<Span>>,
}

impl PromotableNames {
    pub(super) fn of(program: &Program) -> Self {
        let mut names = Self::default();
        walk_program(&mut names, program);
        names
    }

    /// Whether `identifier` may refer to a promoted variable at its position.
    pub(super) fn may_promote(&self, identifier: &Identifier) -> bool {
        self.names.get(&identifier.name).is_some_and(|scopes| {
            scopes.iter().any(|scope| {
                scope.start <= identifier.span.start && identifier.span.end <= scope.end
            })
        })
    }

    fn record(&mut self, expr: &Expr) {
        if let Expr::Ident(identifier) = expr {
            self.record_name(identifier);
        }
    }

    /// Positions outside any declaration (enum constant arguments, for one)
    /// conservatively cover the whole file.
    fn record_name(&mut self, identifier: &Identifier) {
        let scope = self.scope.unwrap_or(Span {
            start: 0,
            end: usize::MAX,
        });
        self.names
            .entry(identifier.name.clone())
            .or_default()
            .push(scope);
    }

    fn scoped(&mut self, span: Span, walk: impl FnOnce(&mut Self)) {
        if self.scope.is_some() {
            walk(self);
            return;
        }
        self.scope = Some(span);
        walk(self);
        self.scope = None;
    }

    /// Records the subjects of collection `if (x case ...)` elements.
    fn record_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                if let IfCondition::Case(subject, _, _) = condition {
                    self.record(subject);
                }
                self.record_element(then_elem);
                if let Some(else_elem) = else_elem {
                    self.record_element(else_elem);
                }
            }
            CollectionElement::For { element, .. } => self.record_element(element),
            _ => {}
        }
    }

    fn record_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                if let IfCondition::Case(subject, _, _) = condition {
                    self.record(subject);
                }
                self.record_map_element(then_entry);
                if let Some(else_entry) = else_entry {
                    self.record_map_element(else_entry);
                }
            }
            MapElement::For { entry, .. } => self.record_map_element(entry),
            _ => {}
        }
    }
}

impl Visitor for PromotableNames {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if matches!(node, TopLevelDecl::Function(_) | TopLevelDecl::Variable(_)) {
            self.scoped(*node.span(), |names| walk_top_level_decl(names, node));
        } else {
            walk_top_level_decl(self, node);
        }
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        self.scoped(*node.span(), |names| walk_class_member(names, node));
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Is { expr, .. } => self.record(expr),
            Expr::As {
                expr, dart_type, ..
            } if !matches!(dart_type, DartType::Dynamic { .. }) => self.record(expr),
            Expr::Assign {
                target,
                op: AssignOp::Eq | AssignOp::NullCoalesceEq,
                ..
            } => self.record(target),
            Expr::Switch { subject, .. } => self.record(subject),
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.record_element(element);
                }
            }
            Expr::Map { elements, .. } => {
                for element in elements {
                    self.record_map_element(element);
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::If(statement) => {
                if let IfCondition::Case(subject, _, _) = &statement.condition {
                    self.record(subject);
                }
            }
            Stmt::Switch(statement) => self.record(&statement.subject),
            Stmt::PatternAssign(statement) => {
                let outer = std::mem::replace(&mut self.in_pattern_assignment, true);
                self.visit_pattern(&statement.pattern);
                self.in_pattern_assignment = outer;
                self.visit_expr(&statement.value);
                return;
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if self.in_pattern_assignment
            && let Pattern::Variable { name, .. } = node
        {
            self.record_name(name);
        }
        walk_pattern(self, node);
    }
}
