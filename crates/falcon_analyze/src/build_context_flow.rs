use std::collections::{HashMap, HashSet};

use crate::{
    DeclarationIdentity, ResolvedType, SemanticModel, SignatureIndex, Truth, TypeEnvironment,
    TypeParameterScope,
};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, bound_names, walk_expr};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BindingId(usize);

#[derive(Clone, PartialEq, Eq, Hash)]
enum Token {
    State,
    This,
    Binding(BindingId),
    Unbound(Box<str>),
    Field(Box<Token>, Box<str>),
}

impl Token {
    fn rooted_at(&self, root: &Self) -> bool {
        self == root || matches!(self, Self::Field(parent, _) if parent.rooted_at(root))
    }

    fn binding(&self) -> Option<BindingId> {
        match self {
            Self::Binding(binding) => Some(*binding),
            Self::Field(parent, _) => parent.binding(),
            Self::State | Self::This | Self::Unbound(_) => None,
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    mounted: Option<HashSet<Token>>,
    inferred_types: HashMap<BindingId, Vec<ResolvedType>>,
}

impl State {
    fn join(left: &Self, right: &Self) -> Self {
        let mounted = match (&left.mounted, &right.mounted) {
            (Some(left), Some(right)) => Some(left.intersection(right).cloned().collect()),
            (Some(mounted), None) | (None, Some(mounted)) => Some(mounted.clone()),
            (None, None) => None,
        };
        let mut inferred_types = left.inferred_types.clone();
        for (binding, types) in &right.inferred_types {
            let joined = inferred_types.entry(*binding).or_default();
            for ty in types {
                if !joined.contains(ty) {
                    joined.push(ty.clone());
                }
            }
        }
        Self {
            mounted,
            inferred_types,
        }
    }

    fn assume(&mut self, tokens: HashSet<Token>) {
        if let Some(mounted) = &mut self.mounted {
            mounted.extend(tokens);
        }
    }

    fn gap(&mut self) {
        self.mounted = Some(HashSet::new());
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Exit {
    Return,
    Throw,
    Break(Option<String>),
    Continue(Option<String>),
}

struct Outcome {
    normal: Option<State>,
    abrupt: Vec<(Exit, State)>,
}

impl Outcome {
    fn normal(state: State) -> Self {
        Self {
            normal: Some(state),
            abrupt: Vec::new(),
        }
    }

    fn abrupt(exit: Exit, state: State) -> Self {
        Self {
            normal: None,
            abrupt: vec![(exit, state)],
        }
    }

    fn from_parts(normal: Option<State>, abrupt: Vec<(Exit, State)>) -> Self {
        let mut outcome = Self {
            normal,
            abrupt: Vec::new(),
        };
        for (exit, state) in abrupt {
            outcome.push_abrupt(exit, state);
        }
        outcome
    }

    fn push_abrupt(&mut self, exit: Exit, state: State) {
        push_abrupt_path(&mut self.abrupt, exit, state);
    }

    fn states(&self) -> impl Iterator<Item = &State> {
        self.normal
            .iter()
            .chain(self.abrupt.iter().map(|(_, state)| state))
    }
}

#[derive(Default)]
struct Guard {
    when_true: HashSet<Token>,
    when_false: HashSet<Token>,
}

pub struct BuildContextFlowAnalyzer<'a> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    environment: TypeEnvironment,
    binding_scopes: Vec<HashMap<String, BindingId>>,
    inferred_bindings: HashSet<BindingId>,
    next_binding_id: usize,
    type_parameters: TypeParameterScope,
    state_class: bool,
    diagnostics: Vec<Span>,
    reported: HashSet<usize>,
    loop_depth: usize,
    remaining_switch_jumps: usize,
    switch_flow_bailed: bool,
}

/// Each loop analyzes its body twice (entry pass + backedge pass), so nesting
/// costs 2^depth body walks. Past this depth the backedge pass is replaced by a
/// conservative gap state instead.
const MAX_LOOP_REANALYSIS_DEPTH: usize = 4;

// Bound the whole function, including nested switches.
const MAX_SWITCH_JUMPS: usize = 4096;

impl<'a> BuildContextFlowAnalyzer<'a> {
    pub fn new(
        model: &'a SemanticModel<'a>,
        signatures: &'a SignatureIndex,
        state_class: bool,
    ) -> Self {
        Self {
            model,
            signatures,
            environment: TypeEnvironment::new(),
            binding_scopes: vec![HashMap::new()],
            inferred_bindings: HashSet::new(),
            next_binding_id: 0,
            type_parameters: TypeParameterScope::default(),
            state_class,
            diagnostics: Vec::new(),
            reported: HashSet::new(),
            loop_depth: 0,
            remaining_switch_jumps: MAX_SWITCH_JUMPS,
            switch_flow_bailed: false,
        }
    }

    /// Enter a loop body, reporting whether the backedge pass is still affordable.
    fn enter_loop(&mut self) -> bool {
        self.loop_depth += 1;
        self.loop_depth <= MAX_LOOP_REANALYSIS_DEPTH
    }

    fn leave_loop(&mut self) {
        self.loop_depth -= 1;
    }

    pub fn analyze(
        mut self,
        params: Option<&FormalParamList>,
        type_params: &[TypeParam],
        body: &FunctionBody,
    ) -> Vec<Span> {
        self.type_parameters.push(type_params, self.model);
        if let Some(params) = params {
            self.environment
                .bind_params(params, self.model, &self.type_parameters);
            self.bind_param_names(params);
        }
        self.function_body(body, State::default());
        if self.switch_flow_bailed {
            tracing::warn!(
                file = ?self.model.source_path(),
                function_start = body.span().start,
                function_end = body.span().end,
                "BuildContext flow analysis exhausted the switch jump budget"
            );
            Vec::new()
        } else {
            self.diagnostics
        }
    }

    fn push_scope(&mut self) {
        self.environment.push_scope();
        self.binding_scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.environment.pop_scope();
        self.binding_scopes.pop();
    }

    fn bind_name(&mut self, name: &str) -> BindingId {
        let token = BindingId(self.next_binding_id);
        self.next_binding_id += 1;
        if let Some(scope) = self.binding_scopes.last_mut() {
            scope.insert(name.to_string(), token);
        }
        token
    }

    fn declare(&mut self, name: String, ty: ResolvedType) {
        self.bind_name(&name);
        self.environment.declare(name, ty);
    }

    fn declare_inferred(&mut self, name: String, ty: ResolvedType, state: &mut State) {
        let token = self.bind_name(&name);
        self.inferred_bindings.insert(token);
        state.inferred_types.insert(token, vec![ty.clone()]);
        self.environment.declare(name, ty);
    }

    fn mark_inferred(&mut self, name: &str, ty: ResolvedType, state: &mut State) {
        if let Some(token) = self
            .binding_scopes
            .last()
            .and_then(|scope| scope.get(name))
            .copied()
        {
            self.inferred_bindings.insert(token);
            state.inferred_types.insert(token, vec![ty]);
        }
    }

    fn is_inferred(&self, name: &str) -> bool {
        self.binding_id(name)
            .is_some_and(|binding| self.inferred_bindings.contains(&binding))
    }

    fn is_bound_in_current_scope(&self, name: &str) -> bool {
        self.binding_scopes
            .last()
            .is_some_and(|scope| scope.contains_key(name))
    }

    fn bind_param_names(&mut self, params: &FormalParamList) {
        for param in params.params.iter() {
            self.bind_name(&param.name.name);
        }
    }

    fn binding_id(&self, name: &str) -> Option<BindingId> {
        self.binding_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
    }

    fn binding_token(&self, name: &str) -> Token {
        self.binding_id(name)
            .map_or_else(|| Token::Unbound(name.into()), Token::Binding)
    }

    fn function_body(&mut self, body: &FunctionBody, mut state: State) -> Outcome {
        match body {
            FunctionBody::Block(block) => self.block(&block.stmts, state),
            FunctionBody::Arrow(expression, _) => {
                self.expression(expression, &mut state);
                Outcome::normal(state)
            }
            FunctionBody::Native(_, _) => Outcome::normal(state),
        }
    }

    fn nested_function(
        &mut self,
        params: &FormalParamList,
        type_params: &[TypeParam],
        body: &FunctionBody,
    ) {
        let first_nested_binding = self.next_binding_id;
        let saved_parameters = self.type_parameters.clone();
        self.type_parameters.push(type_params, self.model);
        self.push_scope();
        self.environment
            .bind_params(params, self.model, &self.type_parameters);
        self.bind_param_names(params);
        self.function_body(body, State::default());
        self.pop_scope();
        self.inferred_bindings
            .retain(|binding| binding.0 < first_nested_binding);
        self.type_parameters = saved_parameters;
    }

    fn infer_type(&self, expression: &Expr, state: &State) -> ResolvedType {
        self.environment.infer_with_signatures_using_lookup(
            expression,
            self.model,
            self.signatures,
            &self.type_parameters,
            &|name| {
                self.binding_id(name)
                    .and_then(|binding| state.inferred_types.get(&binding))
                    .map(|types| {
                        types
                            .iter()
                            .find(|ty| self.is_build_context(ty))
                            .or_else(|| types.first())
                            .cloned()
                            .unwrap_or(ResolvedType::Unknown)
                    })
            },
        )
    }

    fn iterable_element_type(&self, iterable: &Expr, state: &State) -> ResolvedType {
        let iterable = self.infer_type(iterable, state);
        if iterable.interface("dart:core", "Iterable")
            || iterable.interface("dart:core", "List")
            || iterable.interface("dart:core", "Set")
        {
            return iterable
                .arguments()
                .first()
                .cloned()
                .unwrap_or(ResolvedType::Unknown);
        }
        self.signatures
            .instantiated_supertype(&iterable, "dart:core", "Iterable", self.model)
            .and_then(|ty| ty.arguments().first().cloned())
            .unwrap_or(ResolvedType::Unknown)
    }

    fn infer_initializer_type(&self, expression: &Expr, state: &State) -> ResolvedType {
        if let Expr::Record { fields, .. } = expression {
            let mut positional = Vec::new();
            let mut named = Vec::new();
            for field in fields {
                let ty = self.infer_initializer_type(&field.value, state);
                if let Some(name) = &field.name {
                    named.push((name.name.clone(), ty));
                } else {
                    positional.push(ty);
                }
            }
            return ResolvedType::Record {
                positional,
                named,
                nullable: false,
            };
        }
        self.infer_type(expression, state)
    }

    fn bind_pattern(&mut self, pattern: &Pattern, matched: ResolvedType, state: &mut State) {
        for name in bound_names(pattern) {
            self.declare(name.name.clone(), ResolvedType::Unknown);
        }
        self.bind_pattern_types(pattern, matched, state);
    }

    fn bind_pattern_types(&mut self, pattern: &Pattern, matched: ResolvedType, state: &mut State) {
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let ty = type_
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or(matched);
                self.environment.declare(name.name.clone(), ty.clone());
                if type_.is_none() {
                    self.mark_inferred(&name.name, ty, state);
                }
            }
            Pattern::List(list) => {
                let element = list
                    .type_arg
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .or_else(|| {
                        self.signatures
                            .instantiated_supertype(&matched, "dart:core", "List", self.model)
                            .and_then(|ty| ty.arguments().first().cloned())
                    })
                    .unwrap_or(ResolvedType::Unknown);
                for child in &list.elements {
                    match child {
                        ListPatternElement::Pattern(child) => {
                            self.bind_pattern_types(child, element.clone(), state);
                        }
                        ListPatternElement::Rest(Some(child), _) => {
                            self.bind_pattern_types(child, matched.clone(), state);
                        }
                        ListPatternElement::Rest(None, _) => {}
                    }
                }
            }
            Pattern::Record(record) => {
                let (positional, named) = match &matched {
                    ResolvedType::Record {
                        positional, named, ..
                    } => (positional.as_slice(), named.as_slice()),
                    _ => (&[][..], &[][..]),
                };
                let mut positional_index = 0;
                for field in &record.fields {
                    let field_type = field
                        .name
                        .as_ref()
                        .and_then(|name| {
                            named
                                .iter()
                                .find(|(field, _)| field == &name.name)
                                .map(|(_, ty)| ty.clone())
                        })
                        .unwrap_or_else(|| {
                            let ty = positional
                                .get(positional_index)
                                .cloned()
                                .unwrap_or(ResolvedType::Unknown);
                            positional_index += 1;
                            ty
                        });
                    self.bind_pattern_types(&field.pattern, field_type, state);
                }
            }
            Pattern::Map(map) => {
                let value_type = self
                    .signatures
                    .instantiated_supertype(&matched, "dart:core", "Map", self.model)
                    .and_then(|ty| ty.arguments().get(1).cloned())
                    .unwrap_or(ResolvedType::Unknown);
                for entry in &map.entries {
                    self.bind_pattern_types(&entry.pattern, value_type.clone(), state);
                }
            }
            Pattern::Object(object) => {
                let object_type = self
                    .model
                    .resolve_type_in(&object.type_, &self.type_parameters);
                for field in &object.fields {
                    let field_type = self
                        .signatures
                        .resolved_field(&object_type, &field.name.name, self.model)
                        .map(|(ty, substitutions)| self.model.substitute(&ty, &substitutions))
                        .unwrap_or(ResolvedType::Unknown);
                    if let Some(child) = &field.pattern {
                        self.bind_pattern_types(child, field_type, state);
                    } else {
                        self.environment
                            .declare(field.name.name.clone(), field_type.clone());
                        self.mark_inferred(&field.name.name, field_type, state);
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.bind_pattern_types(left, matched.clone(), state);
                self.bind_pattern_types(right, matched, state);
            }
            Pattern::Cast {
                inner, cast_type, ..
            } => {
                let cast = self.model.resolve_type_in(cast_type, &self.type_parameters);
                self.bind_pattern_types(inner, cast, state);
            }
            Pattern::NullCheck { inner, .. } | Pattern::NullAssert { inner, .. } => {
                self.bind_pattern_types(inner, matched.with_nullable(false), state);
            }
            Pattern::ParenPattern { inner, .. } => self.bind_pattern_types(inner, matched, state),
            Pattern::Wildcard { .. }
            | Pattern::Literal(_)
            | Pattern::Const(_)
            | Pattern::Relational { .. }
            | Pattern::Error { .. } => {}
        }
    }

    fn block(&mut self, statements: &[Stmt], state: State) -> Outcome {
        for statement in statements {
            if let Stmt::LocalFunc(function) = statement {
                self.declare(function.name.name.clone(), ResolvedType::Unknown);
            }
        }
        let mut outcome = Outcome::normal(state);
        for statement in statements {
            let Some(state) = outcome.normal.take() else {
                break;
            };
            let next = self.statement(statement, state);
            outcome.normal = next.normal;
            for (exit, state) in next.abrupt {
                outcome.push_abrupt(exit, state);
            }
        }
        outcome
    }

    fn scoped_block(&mut self, statements: &[Stmt], state: State) -> Outcome {
        self.push_scope();
        let outcome = self.block(statements, state);
        self.pop_scope();
        outcome
    }

    fn scoped_statement(&mut self, statement: &Stmt, state: State) -> Outcome {
        self.push_scope();
        let outcome = self.statement(statement, state);
        self.pop_scope();
        outcome
    }

    fn local_declaration(&mut self, declaration: &LocalVarDecl, state: &mut State) {
        let declared = declaration
            .var_type
            .as_ref()
            .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
            .unwrap_or(ResolvedType::Unknown);
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.expression(initializer, state);
            }
            let ty = if matches!(declared, ResolvedType::Unknown) {
                declarator
                    .initializer
                    .as_ref()
                    .map(|expression| self.infer_type(expression, state))
                    .unwrap_or(ResolvedType::Unknown)
            } else {
                declared.clone()
            };
            if declaration.var_type.is_none() {
                self.declare_inferred(declarator.name.name.clone(), ty, state);
            } else {
                self.declare(declarator.name.name.clone(), ty);
            }
        }
    }

    fn statement(&mut self, statement: &Stmt, mut state: State) -> Outcome {
        match statement {
            Stmt::Block(block) => {
                self.push_scope();
                let outcome = self.block(&block.stmts, state);
                self.pop_scope();
                outcome
            }
            Stmt::Expr(expression) => {
                self.expression(&expression.expr, &mut state);
                Outcome::normal(state)
            }
            Stmt::LocalVar(declaration) => {
                self.local_declaration(declaration, &mut state);
                Outcome::normal(state)
            }
            Stmt::If(statement) => self.if_statement(statement, state),
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.value {
                    self.expression(expression, &mut state);
                }
                Outcome::abrupt(Exit::Return, state)
            }
            Stmt::Throw(statement) => {
                self.expression(&statement.value, &mut state);
                Outcome::abrupt(Exit::Throw, state)
            }
            Stmt::Rethrow(_) => Outcome::abrupt(Exit::Throw, state),
            Stmt::Empty(_) => Outcome::normal(state),
            Stmt::Break(statement) => Outcome::abrupt(
                Exit::Break(statement.label.as_ref().map(|label| label.name.clone())),
                state,
            ),
            Stmt::Continue(statement) => Outcome::abrupt(
                Exit::Continue(statement.label.as_ref().map(|label| label.name.clone())),
                state,
            ),
            Stmt::While(statement) => self.while_loop(statement, state, None),
            Stmt::DoWhile(statement) => self.do_while_loop(statement, state, None),
            Stmt::For(statement) => self.for_loop(statement, state, None),
            Stmt::TryCatch(statement) => self.try_statement(statement, state),
            Stmt::Switch(statement) => self.switch_statement(statement, state, None),
            Stmt::Assert(statement) => {
                self.expression(&statement.condition, &mut state);
                if let Some(message) = &statement.message {
                    self.expression(message, &mut state);
                }
                Outcome::normal(state)
            }
            Stmt::Yield(statement) => {
                self.expression(&statement.value, &mut state);
                state.gap();
                Outcome::normal(state)
            }
            Stmt::Labeled(statement) => match statement.stmt.as_ref() {
                Stmt::While(loop_statement) => {
                    self.while_loop(loop_statement, state, Some(&statement.label.name))
                }
                Stmt::DoWhile(loop_statement) => {
                    self.do_while_loop(loop_statement, state, Some(&statement.label.name))
                }
                Stmt::For(loop_statement) => {
                    self.for_loop(loop_statement, state, Some(&statement.label.name))
                }
                Stmt::Switch(switch) => {
                    self.switch_statement(switch, state, Some(&statement.label.name))
                }
                other => {
                    let mut outcome = self.scoped_statement(other, state);
                    let mut normal = outcome.normal.take();
                    outcome.abrupt.retain(|(exit, exit_state)| {
                        if exit == &Exit::Break(Some(statement.label.name.clone())) {
                            normal = Some(match normal.take() {
                                Some(current) => State::join(&current, exit_state),
                                None => exit_state.clone(),
                            });
                            false
                        } else {
                            true
                        }
                    });
                    outcome.normal = normal;
                    outcome
                }
            },
            Stmt::PatternDecl(declaration) => {
                self.expression(&declaration.init, &mut state);
                let matched = self.infer_initializer_type(&declaration.init, &state);
                self.bind_pattern(&declaration.pattern, matched, &mut state);
                Outcome::normal(state)
            }
            Stmt::PatternAssign(statement) => {
                self.expression(&statement.value, &mut state);
                for name in bound_names(&statement.pattern) {
                    self.invalidate_binding(&name.name, &mut state);
                }
                Outcome::normal(state)
            }
            Stmt::LocalFunc(function) => {
                if !self.is_bound_in_current_scope(&function.name.name) {
                    self.declare(function.name.name.clone(), ResolvedType::Unknown);
                }
                self.nested_function(&function.params, &function.type_params, &function.body);
                Outcome::normal(state)
            }
            Stmt::Error(_) => Outcome::normal(state),
        }
    }

    fn if_statement(&mut self, statement: &IfStmt, mut state: State) -> Outcome {
        match &statement.condition {
            IfCondition::Case(expression, pattern, guard) => {
                self.expression(expression, &mut state);
                let matched = self.infer_type(expression, &state);
                self.push_scope();
                let mut then_state = state.clone();
                self.bind_pattern(pattern, matched, &mut then_state);
                let mut else_state = state.clone();
                if let Some(guard) = guard {
                    let guards = self.guard(guard, &then_state);
                    self.expression(guard, &mut then_state);
                    let guard_false = {
                        let mut state = then_state.clone();
                        state.assume(guards.when_false);
                        state
                    };
                    then_state.assume(guards.when_true);
                    else_state = State::join(&else_state, &guard_false);
                }
                let then_out = self.statement(&statement.then_branch, then_state);
                self.pop_scope();
                let else_out = match &statement.else_branch {
                    Some(branch) => self.scoped_statement(branch, else_state),
                    None => Outcome::normal(else_state),
                };
                join_outcomes(then_out, else_out)
            }
            IfCondition::Expr(condition) => {
                let guards = self.guard(condition, &state);
                self.expression(condition, &mut state);
                let mut then_state = state.clone();
                then_state.assume(guards.when_true);
                let then_out = self.scoped_statement(&statement.then_branch, then_state);
                let mut else_state = state;
                else_state.assume(guards.when_false);
                let else_out = match &statement.else_branch {
                    Some(branch) => self.scoped_statement(branch, else_state),
                    None => Outcome::normal(else_state),
                };
                join_outcomes(then_out, else_out)
            }
        }
    }

    fn while_loop(&mut self, statement: &WhileStmt, state: State, label: Option<&str>) -> Outcome {
        self.push_scope();
        let reanalyze = self.enter_loop();
        let guards = self.guard(&statement.condition, &state);
        let mut condition_state = state.clone();
        self.expression(&statement.condition, &mut condition_state);
        let mut condition_exit = condition_state.clone();
        condition_exit.assume(guards.when_false.clone());
        let mut body_input = condition_state;
        body_input.assume(guards.when_true.clone());
        let first = self.scoped_statement(&statement.body, body_input);
        let (backedge, break_state, mut abrupt) = loop_paths(first, label);
        let mut exits = vec![condition_exit];
        if let Some(break_state) = break_state {
            exits.push(break_state);
        }
        if let Some(backedge) = backedge {
            if !reanalyze {
                exits.push(gap_state(backedge));
            } else {
                let mut repeated_condition = backedge;
                self.expression(&statement.condition, &mut repeated_condition);
                let mut repeated_exit = repeated_condition.clone();
                repeated_exit.assume(guards.when_false);
                exits.push(repeated_exit);
                repeated_condition.assume(guards.when_true);
                let repeated = self.scoped_statement(&statement.body, repeated_condition);
                let (_, repeated_break, repeated_abrupt) = loop_paths(repeated, label);
                for (exit, state) in repeated_abrupt {
                    push_abrupt_path(&mut abrupt, exit, state);
                }
                if let Some(repeated_break) = repeated_break {
                    exits.push(repeated_break);
                }
            }
        }
        let normal = join_states(exits).or(Some(state));
        self.leave_loop();
        self.pop_scope();
        Outcome::from_parts(normal, abrupt)
    }

    fn do_while_loop(
        &mut self,
        statement: &DoWhileStmt,
        state: State,
        label: Option<&str>,
    ) -> Outcome {
        self.push_scope();
        let reanalyze = self.enter_loop();
        let guards = self.guard(&statement.condition, &state);
        let first = self.scoped_statement(&statement.body, state.clone());
        let (backedge, break_state, mut abrupt) = loop_paths(first, label);
        let mut exits = Vec::new();
        if let Some(break_state) = break_state {
            exits.push(break_state);
        }
        if let Some(mut condition_state) = backedge {
            self.expression(&statement.condition, &mut condition_state);
            let mut condition_exit = condition_state.clone();
            condition_exit.assume(guards.when_false.clone());
            exits.push(condition_exit);
            if !reanalyze {
                exits.push(gap_state(condition_state));
            } else {
                let mut repeated_input = condition_state;
                repeated_input.assume(guards.when_true.clone());
                let repeated = self.scoped_statement(&statement.body, repeated_input);
                let (repeated_backedge, repeated_break, repeated_abrupt) =
                    loop_paths(repeated, label);
                for (exit, state) in repeated_abrupt {
                    push_abrupt_path(&mut abrupt, exit, state);
                }
                if let Some(repeated_break) = repeated_break {
                    exits.push(repeated_break);
                }
                if let Some(mut repeated_condition) = repeated_backedge {
                    self.expression(&statement.condition, &mut repeated_condition);
                    repeated_condition.assume(guards.when_false);
                    exits.push(repeated_condition);
                }
            }
        }
        let outcome = Outcome::from_parts(join_states(exits), abrupt);
        self.leave_loop();
        self.pop_scope();
        outcome
    }

    fn for_init(&mut self, init: &ForInit, mut state: State) -> State {
        match init {
            ForInit::VarDecl(declaration) => self.local_declaration(declaration, &mut state),
            ForInit::ForIn {
                var_type,
                name,
                iterable,
                ..
            } => {
                self.expression(iterable, &mut state);
                let ty = var_type
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or_else(|| self.iterable_element_type(iterable, &state));
                if var_type.is_none() {
                    self.declare_inferred(name.name.clone(), ty, &mut state);
                } else {
                    self.declare(name.name.clone(), ty);
                }
            }
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
                self.expression(iterable, &mut state);
                let matched = self.iterable_element_type(iterable, &state);
                self.bind_pattern(pattern, matched, &mut state);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.expression(expression, &mut state);
                }
            }
        }
        state
    }

    fn for_loop(&mut self, statement: &ForStmt, mut state: State, label: Option<&str>) -> Outcome {
        self.push_scope();
        let reanalyze = self.enter_loop();
        if statement.is_await {
            state.gap();
        }
        if let Some(init) = &statement.header.init {
            state = self.for_init(init, state);
        }
        let may_skip_body = statement.header.condition.is_some()
            || matches!(
                statement.header.init.as_ref(),
                Some(ForInit::ForIn { .. } | ForInit::PatternForIn { .. })
            );
        let guards = statement
            .header
            .condition
            .as_ref()
            .map(|condition| self.guard(condition, &state))
            .unwrap_or_default();
        if let Some(condition) = &statement.header.condition {
            self.expression(condition, &mut state);
        }
        let mut condition_exit = state.clone();
        condition_exit.assume(guards.when_false.clone());
        let mut body_input = state;
        body_input.assume(guards.when_true.clone());
        let first = self.scoped_statement(&statement.body, body_input);
        let (backedge, break_state, mut abrupt) = loop_paths(first, label);
        let mut exits = Vec::new();
        if may_skip_body {
            exits.push(condition_exit);
        }
        if let Some(break_state) = break_state {
            exits.push(break_state);
        }
        if let Some(mut repeated_condition) = backedge {
            if !reanalyze {
                exits.push(gap_state(repeated_condition));
            } else {
                for update in &statement.header.updates {
                    self.expression(update, &mut repeated_condition);
                }
                if let Some(condition) = &statement.header.condition {
                    self.expression(condition, &mut repeated_condition);
                }
                let mut repeated_exit = repeated_condition.clone();
                repeated_exit.assume(guards.when_false);
                if may_skip_body {
                    exits.push(repeated_exit);
                }
                let mut repeated_input = repeated_condition;
                repeated_input.assume(guards.when_true);
                let repeated = self.scoped_statement(&statement.body, repeated_input);
                let (_, repeated_break, repeated_abrupt) = loop_paths(repeated, label);
                for (exit, state) in repeated_abrupt {
                    push_abrupt_path(&mut abrupt, exit, state);
                }
                if let Some(repeated_break) = repeated_break {
                    exits.push(repeated_break);
                }
            }
        }
        let outcome = Outcome::from_parts(join_states(exits), abrupt);
        self.leave_loop();
        self.pop_scope();
        outcome
    }

    fn try_statement(&mut self, statement: &TryCatchStmt, state: State) -> Outcome {
        let body = self.scoped_block(&statement.body.stmts, state.clone());
        let catch_input = join_states(std::iter::once(state.clone()).chain(body.states().cloned()))
            .unwrap_or_else(|| state.clone());
        let mut branches = vec![body];
        for catch in &statement.catches {
            self.push_scope();
            if let Some(exception) = &catch.exception_var {
                let ty = catch
                    .exception_type
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or(ResolvedType::Unknown);
                self.declare(exception.name.clone(), ty);
            }
            if let Some(stack_trace) = &catch.stack_trace_var {
                self.declare(stack_trace.name.clone(), ResolvedType::Unknown);
            }
            let outcome = self.block(&catch.body.stmts, catch_input.clone());
            self.pop_scope();
            branches.push(outcome);
        }
        let outcome = join_many_outcomes(branches).unwrap_or_else(|| Outcome::normal(state));
        let Some(finally) = &statement.finally else {
            return outcome;
        };

        let mut final_normal = None;
        let mut final_abrupt = Vec::new();
        if let Some(normal) = outcome.normal {
            let final_out = self.scoped_block(&finally.stmts, normal);
            final_normal = final_out.normal;
            for (exit, state) in final_out.abrupt {
                push_abrupt_path(&mut final_abrupt, exit, state);
            }
        }
        for (exit, path_state) in outcome.abrupt {
            let final_out = self.scoped_block(&finally.stmts, path_state);
            if let Some(state) = final_out.normal {
                push_abrupt_path(&mut final_abrupt, exit, state);
            }
            for (exit, state) in final_out.abrupt {
                push_abrupt_path(&mut final_abrupt, exit, state);
            }
        }
        Outcome::from_parts(final_normal, final_abrupt)
    }

    fn switch_statement(
        &mut self,
        statement: &SwitchStmt,
        mut state: State,
        label: Option<&str>,
    ) -> Outcome {
        self.expression(&statement.subject, &mut state);
        let subject_type = self.infer_type(&statement.subject, &state);
        let mut exits = Vec::new();
        let mut abrupt = Vec::new();
        let mut pending_jumps = Vec::new();
        let mut unmatched = Some(state.clone());
        let mut deferred_default = None;
        for case in &statement.cases {
            let Some(case_input) = unmatched.clone() else {
                break;
            };
            self.push_scope();
            let mut body_inputs = Vec::new();
            let mut label_input = case_input;
            let mut is_default = false;
            for kind in &case.cases {
                match kind {
                    SwitchCaseKind::Pattern(pattern, guard) => {
                        self.bind_pattern(pattern, subject_type.clone(), &mut label_input);
                        let irrefutable = pattern_is_irrefutable(pattern);
                        if let Some(guard) = guard.as_ref() {
                            let guards = self.guard(guard, &label_input);
                            let mut guard_state = label_input.clone();
                            self.expression(guard, &mut guard_state);
                            let mut body_state = guard_state.clone();
                            body_state.assume(guards.when_true);
                            body_inputs.push(body_state);
                            guard_state.assume(guards.when_false);
                            label_input = if irrefutable {
                                guard_state
                            } else {
                                State::join(&label_input, &guard_state)
                            };
                            unmatched = Some(label_input.clone());
                        } else {
                            body_inputs.push(label_input.clone());
                            unmatched = (!irrefutable).then(|| label_input.clone());
                            if irrefutable {
                                break;
                            }
                        }
                    }
                    SwitchCaseKind::Default => {
                        is_default = true;
                    }
                }
            }
            if is_default {
                deferred_default = Some((&case.body, body_inputs));
            } else if let Some(body_input) = join_states(body_inputs) {
                let outcome = self.block(&case.body, body_input);
                if let Some(normal) = outcome.normal {
                    exits.push(normal);
                }
                for (exit, exit_state) in outcome.abrupt {
                    match exit {
                        Exit::Break(None) => exits.push(exit_state),
                        Exit::Break(Some(ref target)) if Some(target.as_str()) == label => {
                            exits.push(exit_state);
                        }
                        Exit::Continue(Some(target)) => {
                            pending_jumps.push((target, exit_state));
                        }
                        _ => push_abrupt_path(&mut abrupt, exit, exit_state),
                    }
                }
            }
            self.pop_scope();
        }
        if let Some((body, mut body_inputs)) = deferred_default {
            if let Some(default_input) = unmatched.clone() {
                body_inputs.push(default_input);
            }
            if let Some(body_input) = join_states(body_inputs) {
                let outcome = self.scoped_block(body, body_input);
                if let Some(normal) = outcome.normal {
                    exits.push(normal);
                }
                for (exit, exit_state) in outcome.abrupt {
                    match exit {
                        Exit::Break(None) => exits.push(exit_state),
                        Exit::Break(Some(ref target)) if Some(target.as_str()) == label => {
                            exits.push(exit_state);
                        }
                        Exit::Continue(Some(target)) => {
                            pending_jumps.push((target, exit_state));
                        }
                        _ => push_abrupt_path(&mut abrupt, exit, exit_state),
                    }
                }
            }
        } else if let Some(unmatched) = unmatched {
            exits.push(unmatched);
        }
        let mut processed_jumps = HashMap::<String, State>::new();
        while let Some((target, mut jump_state)) = pending_jumps.pop() {
            if self.remaining_switch_jumps == 0 {
                self.switch_flow_bailed = true;
                return Outcome::normal(State::default());
            }
            self.remaining_switch_jumps -= 1;
            let Some(case) = statement.cases.iter().find(|case| {
                case.labels
                    .iter()
                    .any(|case_label| case_label.name == target)
            }) else {
                push_abrupt_path(&mut abrupt, Exit::Continue(Some(target)), jump_state);
                continue;
            };
            let is_live = |binding: BindingId| {
                self.binding_scopes
                    .iter()
                    .any(|scope| scope.values().any(|id| *id == binding))
            };
            jump_state
                .inferred_types
                .retain(|binding, _| is_live(*binding));
            if let Some(mounted) = &mut jump_state.mounted {
                mounted.retain(|token| token.binding().is_none_or(is_live));
            }
            let jump_state = match processed_jumps.get(&target) {
                Some(current) => {
                    let joined = State::join(current, &jump_state);
                    if &joined == current {
                        continue;
                    }
                    processed_jumps.insert(target.clone(), joined.clone());
                    joined
                }
                None => {
                    processed_jumps.insert(target.clone(), jump_state.clone());
                    jump_state
                }
            };
            let outcome = self.scoped_block(&case.body, jump_state);
            if let Some(normal) = outcome.normal {
                exits.push(normal);
            }
            for (exit, exit_state) in outcome.abrupt {
                match exit {
                    Exit::Break(None) => exits.push(exit_state),
                    Exit::Break(Some(ref target)) if Some(target.as_str()) == label => {
                        exits.push(exit_state);
                    }
                    Exit::Continue(Some(target)) => pending_jumps.push((target, exit_state)),
                    _ => push_abrupt_path(&mut abrupt, exit, exit_state),
                }
            }
        }
        Outcome::from_parts(join_states(exits), abrupt)
    }

    fn expression(&mut self, expression: &Expr, state: &mut State) {
        let mut scanner = ExprScanner {
            analyzer: self,
            state,
            suppress_exact: None,
        };
        scanner.visit_expr(expression);
    }

    fn guard(&self, expression: &Expr, state: &State) -> Guard {
        match expression {
            Expr::Unary {
                op: UnaryOp::Bang,
                operand,
                ..
            } => {
                let guard = self.guard(operand, state);
                Guard {
                    when_true: guard.when_false,
                    when_false: guard.when_true,
                }
            }
            Expr::Binary {
                op: op @ (BinaryOp::And | BinaryOp::Or),
                left,
                right,
                ..
            } => {
                let mut left = self.guard(left, state);
                let mut right = self.guard(right, state);
                let is_or = matches!(op, BinaryOp::Or);
                if is_or {
                    std::mem::swap(&mut left.when_true, &mut left.when_false);
                    std::mem::swap(&mut right.when_true, &mut right.when_false);
                }
                let mut guard = Guard {
                    when_true: left.when_true.union(&right.when_true).cloned().collect(),
                    when_false: left
                        .when_false
                        .intersection(&right.when_false)
                        .cloned()
                        .collect(),
                };
                if is_or {
                    std::mem::swap(&mut guard.when_true, &mut guard.when_false);
                }
                guard
            }
            Expr::Binary {
                op, left, right, ..
            } if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) => {
                if let Expr::BoolLit { value, .. } = right.as_ref() {
                    let mut guard = self.guard(left, state);
                    if (*value && matches!(op, BinaryOp::NotEq))
                        || (!*value && matches!(op, BinaryOp::EqEq))
                    {
                        std::mem::swap(&mut guard.when_true, &mut guard.when_false);
                    }
                    guard
                } else {
                    Guard::default()
                }
            }
            other => self
                .mounted_token(other, state)
                .map_or_else(Guard::default, |token| Guard {
                    when_true: HashSet::from([token]),
                    when_false: HashSet::new(),
                }),
        }
    }

    fn mounted_token(&self, expression: &Expr, state: &State) -> Option<Token> {
        match expression {
            Expr::Ident(identifier)
                if identifier.name == "mounted"
                    && self.state_class
                    && !self.environment.is_bound("mounted") =>
            {
                Some(Token::State)
            }
            Expr::Field { object, field, .. } if field.name == "mounted" => {
                self.context_token(object, state)
            }
            Expr::NullAssert { operand: expr, .. } => self.mounted_token(expr, state),
            _ => None,
        }
    }

    fn context_token(&self, expression: &Expr, state: &State) -> Option<Token> {
        match expression {
            Expr::Ident(identifier) if self.is_context_name(&identifier.name, state) => Some(
                if self.state_class
                    && identifier.name == "context"
                    && !self.environment.is_bound("context")
                {
                    Token::State
                } else {
                    self.binding_token(&identifier.name)
                },
            ),
            Expr::Field { object, field, .. }
                if self.state_class
                    && field.name == "context"
                    && matches!(object.as_ref(), Expr::This { .. }) =>
            {
                Some(Token::State)
            }
            Expr::Field { object, field, .. } => {
                let ty = self.infer_type(expression, state);
                if self.is_build_context(&ty) {
                    Some(Token::Field(
                        Box::new(self.expression_token(object)?),
                        field.name.as_str().into(),
                    ))
                } else {
                    None
                }
            }
            Expr::NullAssert { operand: expr, .. } => self.context_token(expr, state),
            _ => None,
        }
    }

    fn expression_token(&self, expression: &Expr) -> Option<Token> {
        match expression {
            Expr::Ident(identifier) => Some(self.binding_token(&identifier.name)),
            Expr::This { .. } => Some(Token::This),
            Expr::Field { object, field, .. } => Some(Token::Field(
                Box::new(self.expression_token(object)?),
                field.name.as_str().into(),
            )),
            Expr::NullAssert { operand: expr, .. } => self.expression_token(expr),
            _ => None,
        }
    }

    fn invalidate_binding(&self, name: &str, state: &mut State) {
        self.invalidate_token(&self.binding_token(name), state);
    }

    fn invalidate_assignment(&self, target: &Expr, state: &mut State) {
        let Some(token) = self.expression_token(target) else {
            return;
        };
        self.invalidate_token(&token, state);
    }

    fn invalidate_token(&self, token: &Token, state: &mut State) {
        if let Some(mounted) = &mut state.mounted {
            mounted.retain(|mounted| !mounted.rooted_at(token));
        }
    }

    fn is_context_name(&self, name: &str, state: &State) -> bool {
        (self.state_class && name == "context" && !self.environment.is_bound(name))
            || self
                .binding_id(name)
                .and_then(|binding| state.inferred_types.get(&binding))
                .map_or_else(
                    || self.is_build_context(&self.environment.lookup(name)),
                    |types| types.iter().any(|ty| self.is_build_context(ty)),
                )
    }

    fn is_build_context(&self, ty: &ResolvedType) -> bool {
        let target = DeclarationIdentity::flutter("BuildContext");
        matches!(ty, ResolvedType::Interface { identity, .. } if identity == &target)
            || self.signatures.is_subtype_of(ty, &target, self.model) == Truth::Yes
    }

    fn use_context(&mut self, expression: &Expr, state: &State) {
        let Some(mounted) = &state.mounted else {
            return;
        };
        let Some(token) = self.context_token(expression, state) else {
            return;
        };
        let span = expression.span();
        if !mounted.contains(&token) && self.reported.insert(span.start) {
            self.diagnostics.push(*span);
        }
    }
}

struct ExprScanner<'a, 'b> {
    analyzer: &'a mut BuildContextFlowAnalyzer<'b>,
    state: &'a mut State,
    suppress_exact: Option<usize>,
}

impl Visitor for ExprScanner<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Await { expr, .. } => {
                self.visit_expr(expr);
                self.state.gap();
            }
            Expr::Field { object, field, .. } if field.name == "mounted" => {
                let saved = self.suppress_exact.replace(object.span().start);
                self.visit_expr(object);
                self.suppress_exact = saved;
            }
            Expr::Binary {
                op: op @ (BinaryOp::And | BinaryOp::Or),
                left,
                right,
                ..
            } => {
                let mut guard = self.analyzer.guard(left, self.state);
                if matches!(op, BinaryOp::Or) {
                    std::mem::swap(&mut guard.when_true, &mut guard.when_false);
                }
                self.visit_expr(left);
                let mut short_circuit = self.state.clone();
                short_circuit.assume(guard.when_false);
                let mut right_state = self.state.clone();
                right_state.assume(guard.when_true);
                self.analyzer.expression(right, &mut right_state);
                *self.state = State::join(&short_circuit, &right_state);
            }
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } => {
                match target.as_ref() {
                    Expr::Ident(_) => {}
                    Expr::Field { object, .. } => self.visit_expr(object),
                    Expr::Index { object, index, .. } => {
                        self.visit_expr(object);
                        self.visit_expr(index);
                    }
                    other => self.visit_expr(other),
                }
                self.visit_expr(value);
                // Indexed targets invalidate nothing: `expression_token` has no
                // token for `a[i]`, so `a[i] = x` leaves mounted facts standing.
                self.analyzer.invalidate_assignment(target, self.state);
                if let Expr::Ident(identifier) = target.as_ref()
                    && self.analyzer.is_inferred(&identifier.name)
                    && let Some(token) = self.analyzer.binding_id(&identifier.name)
                {
                    let ty = self.analyzer.infer_initializer_type(value, self.state);
                    self.state.inferred_types.insert(token, vec![ty]);
                }
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let guard = self.analyzer.guard(condition, self.state);
                self.visit_expr(condition);
                let base = self.state.clone();
                let mut then_state = base.clone();
                then_state.assume(guard.when_true);
                self.analyzer.expression(then_expr, &mut then_state);
                let mut else_state = base;
                else_state.assume(guard.when_false);
                self.analyzer.expression(else_expr, &mut else_state);
                *self.state = State::join(&then_state, &else_state);
            }
            Expr::FuncExpr {
                type_params,
                params,
                body,
                ..
            } => self.analyzer.nested_function(params, type_params, body),
            _ => {
                if self.suppress_exact != Some(node.span().start) {
                    self.analyzer.use_context(node, self.state);
                }
                walk_expr(self, node);
            }
        }
    }
}

fn pattern_is_irrefutable(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard { type_: None, .. } | Pattern::Variable { type_: None, .. } => true,
        Pattern::ParenPattern { inner, .. } => pattern_is_irrefutable(inner),
        Pattern::LogicalAnd { left, right, .. } => {
            pattern_is_irrefutable(left) && pattern_is_irrefutable(right)
        }
        Pattern::LogicalOr { left, right, .. } => {
            pattern_is_irrefutable(left) || pattern_is_irrefutable(right)
        }
        _ => false,
    }
}

fn push_abrupt_path(paths: &mut Vec<(Exit, State)>, exit: Exit, state: State) {
    if let Some((_, current)) = paths
        .iter_mut()
        .find(|(current_exit, _)| current_exit == &exit)
    {
        *current = State::join(current, &state);
    } else {
        paths.push((exit, state));
    }
}

fn loop_paths(
    outcome: Outcome,
    label: Option<&str>,
) -> (Option<State>, Option<State>, Vec<(Exit, State)>) {
    let mut backedges = outcome.normal.into_iter().collect::<Vec<_>>();
    let mut breaks = Vec::new();
    let mut abrupt = Vec::new();
    for (exit, state) in outcome.abrupt {
        match &exit {
            Exit::Continue(None) => backedges.push(state),
            Exit::Continue(Some(target)) if Some(target.as_str()) == label => {
                backedges.push(state);
            }
            Exit::Break(None) => breaks.push(state),
            Exit::Break(Some(target)) if Some(target.as_str()) == label => breaks.push(state),
            _ => push_abrupt_path(&mut abrupt, exit, state),
        }
    }
    (join_states(backedges), join_states(breaks), abrupt)
}

/// The state to fall back on when a path is not analyzed: nothing is known to
/// be mounted, and an async gap may have happened.
fn gap_state(mut state: State) -> State {
    state.gap();
    state
}

fn join_states(states: impl IntoIterator<Item = State>) -> Option<State> {
    states
        .into_iter()
        .reduce(|left, right| State::join(&left, &right))
}

fn join_many_outcomes(outcomes: Vec<Outcome>) -> Option<Outcome> {
    outcomes.into_iter().reduce(join_outcomes)
}

fn join_outcomes(mut left: Outcome, right: Outcome) -> Outcome {
    left.normal = match (left.normal, right.normal) {
        (Some(left), Some(right)) => Some(State::join(&left, &right)),
        (Some(state), None) | (None, Some(state)) => Some(state),
        (None, None) => None,
    };
    for (exit, state) in right.abrupt {
        left.push_abrupt(exit, state);
    }
    left
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_dart_parser::parse;

    use super::*;
    use crate::resolve::{IdentityIndex, IdentitySource, TypeIndex};

    #[test]
    fn flow_token_roots_distinguish_state_and_field_paths() {
        let unbound_state = Token::Unbound("$state".into());
        assert!(!unbound_state.rooted_at(&Token::State));
        assert!(!Token::State.rooted_at(&unbound_state));

        let root = Token::Unbound("a".into());
        let field = Token::Field(Box::new(root.clone()), "b".into());
        let descendant = Token::Field(Box::new(field.clone()), "context".into());
        let sibling = Token::Field(Box::new(root.clone()), "bc".into());
        assert!(descendant.rooted_at(&field));
        assert!(descendant.rooted_at(&root));
        assert!(!sibling.rooted_at(&field));
        assert!(!field.rooted_at(&descendant));
    }

    #[test]
    fn labeled_switch_continue_converges_with_inferred_locals() {
        let source = r#"
void f(int value) async {
  switch (value) {
    again:
    case 0:
      var temporary = 0;
      await tick();
      context.toString();
      continue again;
    default:
      return;
  }
}
"#;
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let path = Path::new("/project/lib/main.dart");
        let sources = [IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };
        let diagnostics = BuildContextFlowAnalyzer::new(&model, &signatures, true).analyze(
            function.params(),
            &function.type_params,
            function.body.as_ref().unwrap(),
        );
        assert_eq!(diagnostics.len(), 1);
        let mut exhausted = BuildContextFlowAnalyzer::new(&model, &signatures, true);
        exhausted.remaining_switch_jumps = 0;
        exhausted.function_body(function.body.as_ref().unwrap(), State::default());
        assert!(exhausted.switch_flow_bailed);
        assert_eq!(exhausted.model.source_path(), Some(path));
        assert_eq!(
            diagnostics[0].start,
            source.find("context.toString").unwrap()
        );
        for (budget, expected_count) in [(1, 0), (2, 1)] {
            let mut bounded = BuildContextFlowAnalyzer::new(&model, &signatures, true);
            bounded.remaining_switch_jumps = budget;
            let diagnostics = bounded.analyze(
                function.params(),
                &function.type_params,
                function.body.as_ref().unwrap(),
            );
            assert_eq!(diagnostics.len(), expected_count, "budget {budget}");
        }
    }

    #[test]
    fn nested_malformed_object_pattern_does_not_mark_outer_binding_inferred() {
        let (mut program, errors) =
            parse("void nested() { var (Object(outer: var local),) = value; }");
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let TopLevelDecl::Function(function) = &mut program.declarations[0] else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(block)) = &mut function.body else {
            panic!("expected block");
        };
        let Stmt::PatternDecl(declaration) = &mut block.stmts[0] else {
            panic!("expected pattern declaration");
        };
        let Pattern::Record(record) = &mut declaration.pattern else {
            panic!("expected record pattern");
        };
        let Pattern::Object(object) = &mut record.fields[0].pattern else {
            panic!("expected object pattern");
        };
        object.fields[0].pattern = None;

        let path = Path::new("/project/lib/main.dart");
        let sources = [IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };
        let mut analyzer = BuildContextFlowAnalyzer::new(&model, &signatures, false);
        analyzer.declare("outer".into(), ResolvedType::Unknown);
        let outer_binding = analyzer.binding_id("outer");
        analyzer.nested_function(
            function.params().unwrap(),
            &function.type_params,
            function.body.as_ref().unwrap(),
        );
        assert!(analyzer.binding_id("outer") == outer_binding);
        assert!(!analyzer.is_inferred("outer"));
        assert_eq!(analyzer.environment.lookup("outer"), ResolvedType::Unknown);
    }
}
