use super::*;
use crate::resolve::semantic::ResolvedTypeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InferenceInput {
    Value(DeclarationIdentity),
    Member(DeclarationIdentity, String),
    Constructor(DeclarationIdentity, String),
    Constructors(DeclarationIdentity),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum InferenceInputRef<'a> {
    Value(&'a DeclarationIdentity),
    Member(&'a DeclarationIdentity, &'a str),
    Constructor(&'a DeclarationIdentity, &'a str),
    Constructors(&'a DeclarationIdentity),
}

trait InferenceInputQuery {
    fn as_input(&self) -> InferenceInputRef<'_>;
}

impl InferenceInputQuery for InferenceInput {
    fn as_input(&self) -> InferenceInputRef<'_> {
        match self {
            Self::Value(identity) => InferenceInputRef::Value(identity),
            Self::Member(identity, name) => InferenceInputRef::Member(identity, name),
            Self::Constructor(identity, name) => InferenceInputRef::Constructor(identity, name),
            Self::Constructors(identity) => InferenceInputRef::Constructors(identity),
        }
    }
}

impl InferenceInputQuery for InferenceInputRef<'_> {
    fn as_input(&self) -> InferenceInputRef<'_> {
        *self
    }
}

impl Hash for InferenceInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_input().hash(state);
    }
}

impl Hash for dyn InferenceInputQuery + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_input().hash(state);
    }
}

impl PartialEq for dyn InferenceInputQuery + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.as_input() == other.as_input()
    }
}

impl Eq for dyn InferenceInputQuery + '_ {}

impl<'a> Borrow<dyn InferenceInputQuery + 'a> for InferenceInput {
    fn borrow(&self) -> &(dyn InferenceInputQuery + 'a) {
        self
    }
}

impl InferenceInputRef<'_> {
    fn into_owned(self) -> InferenceInput {
        match self {
            Self::Value(identity) => InferenceInput::Value(identity.clone()),
            Self::Member(identity, name) => {
                InferenceInput::Member(identity.clone(), name.to_owned())
            }
            Self::Constructor(identity, name) => {
                InferenceInput::Constructor(identity.clone(), name.to_owned())
            }
            Self::Constructors(identity) => InferenceInput::Constructors(identity.clone()),
        }
    }
}

#[derive(Default)]
struct InferenceRecording {
    reads: FxHashSet<InferenceInput>,
    writes: FxHashSet<InferenceInput>,
    blocked: bool,
    settlement: Option<std::rc::Rc<std::cell::RefCell<InferenceSettlement>>>,
    output: Option<InferenceInput>,
}

thread_local! {
    static INFERENCE_RECORDINGS: std::cell::RefCell<Vec<InferenceRecording>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

pub(super) fn inference_deferred() -> bool {
    INFERENCE_RECORDINGS.with(|recordings| {
        recordings
            .borrow()
            .last()
            .is_some_and(|recording| recording.blocked)
    })
}

pub(super) fn defer_inherited_combination() -> bool {
    INFERENCE_RECORDINGS.with(|recordings| {
        let mut recordings = recordings.borrow_mut();
        let Some(recording) = recordings.last_mut() else {
            return false;
        };
        if recording.settlement.is_some() && !recording.blocked {
            return false;
        }
        recording.blocked = true;
        true
    })
}

struct InferenceSettlement {
    producers: Arc<FxHashMap<InferenceInput, Vec<usize>>>,
    pending: FxHashSet<usize>,
    order: std::collections::VecDeque<usize>,
}

impl InferenceSettlement {
    fn sources<'a>(
        producers: &'a FxHashMap<InferenceInput, Vec<usize>>,
        input: InferenceInputRef<'_>,
    ) -> Option<&'a Vec<usize>> {
        let query: &dyn InferenceInputQuery = &input;
        producers.get(query).or_else(|| match input {
            InferenceInputRef::Constructor(identity, _) => {
                let aggregate = InferenceInputRef::Constructors(identity);
                let query: &dyn InferenceInputQuery = &aggregate;
                producers.get(query)
            }
            _ => None,
        })
    }

    fn new(
        producers: Arc<FxHashMap<InferenceInput, Vec<usize>>>,
        outputs: &[InferenceInput],
        reads: &[FxHashSet<InferenceInput>],
        blocked: &std::collections::BTreeSet<usize>,
    ) -> Self {
        let mut dependencies = vec![FxHashSet::default(); reads.len()];
        let mut dependents = vec![Vec::new(); reads.len()];
        for (consumer, inputs) in reads.iter().enumerate() {
            for input in inputs {
                // Getter and setter work can share an output. The publication
                // guard is not a dependency on any producer of that same key.
                if *input == outputs[consumer] {
                    continue;
                }
                for &source in Self::sources(&producers, input.as_input())
                    .into_iter()
                    .flatten()
                {
                    if dependencies[consumer].insert(source) {
                        dependents[source].push(consumer);
                    }
                }
            }
        }
        let mut pending: FxHashSet<_> = blocked.iter().copied().collect();
        let mut discovery: Vec<_> = blocked.iter().copied().collect();
        while let Some(source) = discovery.pop() {
            for &consumer in &dependents[source] {
                if pending.insert(consumer) {
                    discovery.push(consumer);
                }
            }
        }
        let mut remaining = vec![0; reads.len()];
        let mut ready = std::collections::VecDeque::new();
        for (id, sources) in dependencies.iter().enumerate() {
            if pending.contains(&id) {
                remaining[id] = sources
                    .iter()
                    .filter(|source| pending.contains(source))
                    .count();
                if remaining[id] == 0 {
                    ready.push_back(id);
                }
            }
        }
        let mut order = std::collections::VecDeque::new();
        while let Some(source) = ready.pop_front() {
            order.push_back(source);
            for &consumer in &dependents[source] {
                if pending.contains(&consumer) {
                    remaining[consumer] -= 1;
                    if remaining[consumer] == 0 {
                        ready.push_back(consumer);
                    }
                }
            }
        }
        Self {
            producers,
            pending,
            order,
        }
    }

    fn input_stable(&self, input: InferenceInputRef<'_>) -> bool {
        Self::sources(&self.producers, input)
            .is_none_or(|sources| sources.iter().all(|source| !self.pending.contains(source)))
    }
}

pub(super) fn record_inference_read(input: InferenceInputRef<'_>) {
    INFERENCE_RECORDINGS.with(|recordings| {
        if let Some(recording) = recordings.borrow_mut().last_mut() {
            let query: &dyn InferenceInputQuery = &input;
            if let Some(settlement) = &recording.settlement
                && recording
                    .output
                    .as_ref()
                    .is_none_or(|output| output.as_input() != input)
                && !settlement.as_ref().borrow().input_stable(input)
            {
                recording.blocked = true;
            }
            if !recording.reads.contains(query) {
                recording.reads.insert(input.into_owned());
            }
        }
    });
}

#[cfg(test)]
thread_local! {
    static INFERENCE_VISITS: std::cell::RefCell<Vec<FxHashMap<InferenceInput, usize>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(super) fn with_inference_visits(run: impl FnOnce()) -> FxHashMap<InferenceInput, usize> {
    struct Visits;
    impl Drop for Visits {
        fn drop(&mut self) {
            INFERENCE_VISITS.with(|visits| {
                visits.borrow_mut().pop();
            });
        }
    }
    INFERENCE_VISITS.with(|visits| visits.borrow_mut().push(FxHashMap::default()));
    let guard = Visits;
    run();
    let counts =
        INFERENCE_VISITS.with(|visits| std::mem::take(visits.borrow_mut().last_mut().unwrap()));
    drop(guard);
    counts
}

pub(super) fn record_inference_write(input: impl FnOnce() -> InferenceInput) {
    INFERENCE_RECORDINGS.with(|recordings| {
        if let Some(recording) = recordings.borrow_mut().last_mut() {
            recording.writes.insert(input());
        }
    });
}

/// The stack restores an enclosing recorder even if inference unwinds.
struct InferenceRecorder;

impl InferenceRecorder {
    fn start(recording: InferenceRecording) -> Self {
        INFERENCE_RECORDINGS.with(|recordings| {
            recordings.borrow_mut().push(recording);
        });
        Self
    }

    fn finish(self) -> InferenceRecording {
        let recording = INFERENCE_RECORDINGS
            .with(|recordings| std::mem::take(recordings.borrow_mut().last_mut().unwrap()));
        drop(self);
        recording
    }
}

impl Drop for InferenceRecorder {
    fn drop(&mut self) {
        INFERENCE_RECORDINGS.with(|recordings| {
            recordings.borrow_mut().pop();
        });
    }
}

/// A value declaration is either unqualified or import-prefixed. Longer
/// instance chains are handled one receiver at a time, never rescanned.
pub(super) fn initializer_value_name(expression: Expr<'_>) -> Option<QualifiedName<'_>> {
    match expression.unparenthesized() {
        Expr::Ident(identifier) => Some(identifier.name().into()),
        Expr::Field(field) => {
            let Expr::Ident(prefix) = field.object() else {
                return None;
            };
            Some(QualifiedName::prefixed(prefix.name(), field.field().name()))
        }
        _ => None,
    }
}

/// The shared initializer traversal keeps lexical names out of both the root
/// cycle graph and readiness checks. Callbacks run after children, allowing
/// receiver chains to reuse their immediate receiver's result.
struct InitializerDependencies<F> {
    environment: TypeEnvironment,
    inspect: F,
}

impl<'a, F: FnMut(Expr<'a>, &TypeEnvironment)> InitializerDependencies<F> {
    fn bind(&mut self, name: &str) {
        self.environment.declare(name, ResolvedType::UNKNOWN);
    }

    fn bind_statements(&mut self, statements: AstChildren<'a, falcon_syntax::cst::Stmt<'a>>) {
        use falcon_syntax::cst::Stmt;
        for statement in statements {
            match statement {
                Stmt::LocalVar(variable) => {
                    for declarator in variable.declarators() {
                        self.bind(declarator.name().name());
                    }
                }
                Stmt::LocalFunc(function) => self.bind(function.name().name()),
                Stmt::PatternDecl(declaration) => self.bind_pattern(declaration.pattern()),
                _ => {}
            }
        }
    }

    fn bind_pattern(&mut self, pattern: falcon_syntax::cst::Pattern<'a>) {
        struct Bindings<'a>(&'a mut TypeEnvironment);
        impl<'a> Visitor<'a> for Bindings<'_> {
            fn visit_pattern(&mut self, pattern: falcon_syntax::cst::Pattern<'a>) {
                if let falcon_syntax::cst::Pattern::Variable(variable) = pattern {
                    self.0
                        .declare(variable.name().name(), ResolvedType::UNKNOWN);
                }
                if let falcon_syntax::cst::Pattern::Object(object) = pattern {
                    for field in object.fields() {
                        if field.pattern().is_none() {
                            self.0.declare(field.name().name(), ResolvedType::UNKNOWN);
                        }
                    }
                }
                falcon_syntax::cst::visit::walk_pattern(self, pattern);
            }
            fn visit_expr(&mut self, _: Expr<'a>) {}
        }
        Bindings(&mut self.environment).visit_pattern(pattern);
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.environment.push_scope();
        visit(self);
        self.environment.pop_scope();
    }

    fn condition(
        &mut self,
        condition: falcon_syntax::cst::IfCondition<'a>,
        then: impl FnOnce(&mut Self),
    ) {
        use falcon_syntax::cst::IfCondition;
        match condition {
            IfCondition::Expr(expression) => {
                self.visit_expr(expression);
                self.scoped(then);
            }
            IfCondition::Case(condition) => {
                self.visit_expr(condition.expr());
                self.scoped(|visitor| {
                    visitor.bind_pattern(condition.pattern());
                    visitor.visit_pattern(condition.pattern());
                    if let Some(guard) = condition.guard() {
                        visitor.visit_expr(guard);
                    }
                    then(visitor);
                });
            }
        }
    }
}

#[test]
fn initializer_dependencies_walk_long_chains_in_postorder() {
    let source = format!("final value = a{};", " + a".repeat(99_999));
    let parsed = falcon_dart_parser::parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Variable(declaration) =
        parsed.tree.program(&source).declarations().next().unwrap()
    else {
        panic!("expected a variable");
    };
    let expression = declaration
        .declarators()
        .next()
        .unwrap()
        .initializer()
        .unwrap();
    let mut identifiers = 0;
    let mut binaries = 0;
    InitializerDependencies {
        environment: TypeEnvironment::new(),
        inspect: |expression: Expr<'_>, _: &TypeEnvironment| match expression {
            Expr::Ident(_) => identifiers += 1,
            Expr::Binary(_) => {
                binaries += 1;
                assert_eq!(identifiers, binaries + 1);
            }
            _ => panic!("unexpected expression"),
        },
    }
    .visit_expr(expression);
    assert_eq!(identifiers, 100_000);
    assert_eq!(binaries, 99_999);
}

impl<'a, F: FnMut(Expr<'a>, &TypeEnvironment)> Visitor<'a> for InitializerDependencies<F> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        falcon_syntax::cst::visit::walk_expr_chain_with_exit(
            self,
            expression,
            |visitor, expression| {
                if let Expr::FuncExpr(function) = expression {
                    visitor.scoped(|visitor| {
                        for parameter in function.params().params() {
                            visitor.bind(parameter.name().name());
                        }
                        walk_expr(visitor, expression);
                        (visitor.inspect)(expression, &visitor.environment);
                    });
                    false
                } else {
                    true
                }
            },
            |visitor, expression| {
                if !matches!(expression, Expr::FuncExpr(_)) {
                    (visitor.inspect)(expression, &visitor.environment);
                }
            },
        );
    }

    fn visit_block(&mut self, block: falcon_syntax::cst::Block<'a>) {
        self.scoped(|visitor| {
            // Local declarations shadow throughout their block.
            visitor.bind_statements(block.stmts());
            falcon_syntax::cst::visit::walk_block(visitor, block);
        });
    }

    fn visit_stmt(&mut self, statement: falcon_syntax::cst::Stmt<'a>) {
        use falcon_syntax::cst::Stmt;
        match statement {
            Stmt::LocalFunc(function) => self.scoped(|visitor| {
                for parameter in function.params().params() {
                    visitor.bind(parameter.name().name());
                }
                walk_stmt(visitor, statement);
            }),
            Stmt::For(_) => self.scoped(|visitor| walk_stmt(visitor, statement)),
            Stmt::If(statement) => {
                self.condition(statement.condition(), |visitor| {
                    visitor.visit_stmt(statement.then_branch())
                });
                if let Some(branch) = statement.else_branch() {
                    self.visit_stmt(branch);
                }
            }
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_for_header(&mut self, header: falcon_syntax::cst::ForHeader<'a>) {
        use falcon_syntax::cst::ForInit;
        match header.init() {
            Some(ForInit::VarDecl(declaration)) => {
                for declarator in declaration.declarators() {
                    self.bind(declarator.name().name());
                }
            }
            Some(ForInit::ForIn(initializer)) if initializer.declares_variable() => {
                self.visit_expr(initializer.iterable());
                self.bind(initializer.name().name());
                return;
            }
            Some(ForInit::PatternForIn(initializer)) => {
                self.visit_expr(initializer.iterable());
                self.bind_pattern(initializer.pattern());
                self.visit_pattern(initializer.pattern());
                return;
            }
            _ => {}
        }
        falcon_syntax::cst::visit::walk_for_header(self, header);
    }

    fn visit_catch_clause(&mut self, catch: falcon_syntax::cst::CatchClause<'a>) {
        self.scoped(|visitor| {
            for name in [catch.exception_var(), catch.stack_trace_var()]
                .into_iter()
                .flatten()
            {
                visitor.bind(name.name());
            }
            falcon_syntax::cst::visit::walk_catch_clause(visitor, catch);
        });
    }

    fn visit_switch_case(&mut self, case: falcon_syntax::cst::SwitchCase<'a>) {
        self.scoped(|visitor| {
            for kind in case.cases() {
                if let falcon_syntax::cst::SwitchCaseKind::Pattern(label) = kind {
                    visitor.bind_pattern(label.pattern());
                }
            }
            visitor.bind_statements(case.body());
            falcon_syntax::cst::visit::walk_switch_case(visitor, case);
        });
    }

    fn visit_switch_expr_arm(&mut self, arm: falcon_syntax::cst::SwitchExprArm<'a>) {
        self.scoped(|visitor| {
            visitor.bind_pattern(arm.pattern());
            falcon_syntax::cst::visit::walk_switch_expr_arm(visitor, arm);
        });
    }

    fn visit_element<E: falcon_syntax::cst::Element<'a>>(&mut self, element: E) {
        use falcon_syntax::cst::ElementKind;
        match element.kind() {
            kind @ ElementKind::For { .. } => {
                self.scoped(|visitor| falcon_syntax::cst::visit::walk_element(visitor, kind))
            }
            ElementKind::If {
                condition,
                then_element,
                else_element,
            } => {
                self.condition(condition, |visitor| visitor.visit_element(then_element));
                if let Some(element) = else_element {
                    self.visit_element(element);
                }
            }
            kind => falcon_syntax::cst::visit::walk_element(self, kind),
        }
    }
}

impl SignatureIndex {
    /// Visit each initializer once, resolving import prefixes to declaration
    /// identities before finding cycles across all indexed libraries.
    fn collect_value_dependencies(
        program: Program<'_>,
        model: &SemanticModel<'_>,
        graph: &mut FxHashMap<DeclarationIdentity, Vec<DeclarationIdentity>>,
    ) {
        let mut edges = FxHashSet::default();
        for declaration in program.declarations() {
            let TopLevelDecl::Variable(variable) = declaration else {
                continue;
            };
            if variable.var_type().is_some() {
                continue;
            }
            for declarator in variable.declarators() {
                if !model.owns_declaration(declarator.name().name()) {
                    continue;
                }
                if let Some(initializer) = declarator.initializer()
                    && let Some(identity) = model.resolve_value(declarator.name().name())
                {
                    edges.clear();
                    InitializerDependencies {
                        environment: TypeEnvironment::new(),
                        inspect: |expression: Expr<'_>, environment: &TypeEnvironment| {
                            if let Some(name) = initializer_value_name(expression)
                                && !environment.is_bound(name.prefix.unwrap_or(name.name))
                                && let Some(identity) = model.resolve_value(name)
                            {
                                edges.insert(identity);
                            }
                        },
                    }
                    .visit_expr(initializer);
                    graph.insert(identity, edges.iter().cloned().collect());
                }
            }
        }
    }

    /// Iterative Kosaraju traversal: O(V + E), without revisiting initializer
    /// syntax or marking acyclic consumers of a cycle as cyclic themselves.
    fn cyclic_value_identities(
        graph: &FxHashMap<DeclarationIdentity, Vec<DeclarationIdentity>>,
    ) -> FxHashSet<DeclarationIdentity> {
        let mut visited = FxHashSet::default();
        let mut order = Vec::with_capacity(graph.len());
        let mut reverse: FxHashMap<_, Vec<_>> = FxHashMap::default();
        for (identity, edges) in graph {
            for edge in edges.iter().filter(|edge| graph.contains_key(*edge)) {
                reverse.entry(edge).or_default().push(identity);
            }
        }
        let mut stack = Vec::with_capacity(graph.len());
        for identity in graph.keys() {
            stack.push((identity, false));
            while let Some((identity, finished)) = stack.pop() {
                if finished {
                    order.push(identity);
                } else if visited.insert(identity) {
                    stack.push((identity, true));
                    stack.extend(
                        graph[identity]
                            .iter()
                            .filter(|edge| graph.contains_key(*edge))
                            .map(|edge| (edge, false)),
                    );
                }
            }
        }
        visited.clear();
        let mut cyclic = FxHashSet::default();
        let mut component = Vec::with_capacity(graph.len());
        for identity in order.into_iter().rev() {
            if !visited.insert(identity) {
                continue;
            }
            component.clear();
            component.push(identity);
            let mut cursor = 0;
            while cursor < component.len() {
                if let Some(edges) = reverse.get(component[cursor]) {
                    for &edge in edges {
                        if visited.insert(edge) {
                            component.push(edge);
                        }
                    }
                }
                cursor += 1;
            }
            if component.len() > 1 || graph[identity].contains(identity) {
                cyclic.extend(component.iter().map(|identity| (*identity).clone()));
            }
        }
        cyclic
    }

    fn resolve_value_initializer(
        &mut self,
        identity: &DeclarationIdentity,
        initializer: Expr<'_>,
        model: &SemanticModel<'_>,
        pending: &mut FxHashMap<DeclarationIdentity, ResolvedType>,
        retry_partial: bool,
    ) {
        if self.is_cyclic_value(identity)
            || self
                .value_type(identity)
                .is_some_and(|ty| !has_inference_holes(ty))
        {
            return;
        }
        let ty = TypeEnvironment::new().infer_with_signatures(
            initializer,
            model,
            self,
            &TypeParameterScope::default(),
        );
        let ty = if ty == ResolvedType::NULL {
            ResolvedType::DYNAMIC
        } else {
            ty
        };
        let dependencies_known = has_inference_holes(&ty)
            || !retry_partial
            || self.declaration_dependencies_known(
                initializer,
                model,
                None,
                &TypeEnvironment::new(),
                &TypeParameterScope::default(),
            );
        if inference_deferred() {
            pending.remove(identity);
            return;
        }
        if has_inference_holes(&ty) {
            // Delay partial evidence until concrete progress stops. Published
            // partials may become concrete, but must never recursively expand.
            if ty != ResolvedType::UNKNOWN
                && (retry_partial
                    || self
                        .value_types
                        .get(identity)
                        .is_none_or(|ty| *ty == ResolvedType::UNKNOWN))
            {
                pending.insert(identity.clone(), ty);
            } else {
                pending.remove(identity);
            }
        } else if !dependencies_known {
            pending.insert(identity.clone(), ty);
        } else {
            pending.remove(identity);
            self.publish_value_type(identity, ty);
        }
    }

    fn publish_value_type(&mut self, identity: &DeclarationIdentity, ty: ResolvedType) {
        if inference_deferred() {
            return;
        }
        let slot = self
            .value_types
            .entry(identity.clone())
            .or_insert(ResolvedType::UNKNOWN);
        if refine_inferred_type(slot, ty) {
            record_inference_write(|| InferenceInput::Value(identity.clone()));
        }
    }

    pub(super) fn add_program(&mut self, program: Program<'_>, model: &SemanticModel<'_>) {
        self.add_declarations(program, model, true);
        self.finish();
        self.infer_programs(&[(program, *model)]);
        self.mark_field_forwarders(model.identities, model.types);
    }

    pub(super) fn infer_programs(&mut self, programs: &[(Program<'_>, SemanticModel<'_>)]) {
        enum Work<'a> {
            Constructor(index_build::ConstructorInferenceEntry<'a>),
            Override(DeclarationIdentity, ClassMember<'a>),
            Member {
                identity: DeclarationIdentity,
                name: &'a str,
                expression: Option<Expr<'a>>,
                context: Arc<(TypeEnvironment, TypeParameterScope)>,
            },
            Value(DeclarationIdentity, Expr<'a>),
        }
        impl Work<'_> {
            fn header(&self) -> bool {
                matches!(
                    self,
                    Self::Constructor(_)
                        | Self::Override(..)
                        | Self::Member {
                            expression: None,
                            ..
                        }
                )
            }

            fn output_ref(&self) -> InferenceInputRef<'_> {
                match self {
                    Self::Value(identity, _) => InferenceInputRef::Value(identity),
                    Self::Constructor(index_build::ConstructorInferenceEntry::Alias {
                        identity,
                        ..
                    }) => InferenceInputRef::Constructors(identity),
                    Self::Constructor(_) => {
                        InferenceInputRef::Constructor(self.identity(), self.name())
                    }
                    _ => InferenceInputRef::Member(self.identity(), self.name()),
                }
            }

            fn output(&self) -> InferenceInput {
                self.output_ref().into_owned()
            }

            fn identity(&self) -> &DeclarationIdentity {
                match self {
                    Self::Constructor(
                        index_build::ConstructorInferenceEntry::Alias { identity, .. }
                        | index_build::ConstructorInferenceEntry::Declaration { identity, .. },
                    )
                    | Self::Override(identity, _)
                    | Self::Member { identity, .. }
                    | Self::Value(identity, _) => identity,
                }
            }

            fn name(&self) -> &str {
                match self {
                    Self::Constructor(index_build::ConstructorInferenceEntry::Alias {
                        name,
                        ..
                    }) => name,
                    Self::Constructor(index_build::ConstructorInferenceEntry::Declaration {
                        constructor_name,
                        ..
                    }) => constructor_name,
                    Self::Override(_, member) => match member {
                        ClassMember::Method(method) => method.name().name(),
                        ClassMember::Operator(operator) => operator.lookup_name(),
                        ClassMember::Setter(setter) => setter.name().name(),
                        _ => unreachable!("non-callable override inference entry"),
                    },
                    Self::Member { name, .. } => name,
                    Self::Value(..) => "initializer",
                }
            }
        }

        let mut dependencies = FxHashMap::default();
        let mut entries = Vec::new();
        for (file, (program, model)) in programs.iter().enumerate() {
            Self::collect_value_dependencies(*program, model, &mut dependencies);
            entries.extend(
                Self::constructor_inference_entries(*program, model)
                    .into_iter()
                    .map(|entry| (file, Work::Constructor(entry))),
            );
            for declaration in program.declarations() {
                if let TopLevelDecl::Variable(variable) = declaration {
                    if variable.var_type().is_none() {
                        for declarator in variable.declarators() {
                            if model.owns_declaration(declarator.name().name())
                                && let Some(identity) =
                                    model.resolve_value(declarator.name().name())
                                && let Some(expression) = declarator.initializer()
                            {
                                entries.push((file, Work::Value(identity, expression)));
                            }
                        }
                    }
                    continue;
                }
                if !model.owns_top_level_declaration(declaration) {
                    continue;
                }
                let (name, members) = match declaration {
                    TopLevelDecl::Class(class) => (class.name(), class.members()),
                    TopLevelDecl::MixinClass(class) => (class.name(), class.members()),
                    TopLevelDecl::Mixin(mixin) => (mixin.name(), mixin.members()),
                    TopLevelDecl::Enum(enumeration) => (enumeration.name(), enumeration.members()),
                    TopLevelDecl::ExtensionType(extension) => {
                        (extension.name(), extension.members())
                    }
                    _ => continue,
                };
                let Some(identity) = model.resolve_name(name.name()) else {
                    continue;
                };
                let mut context = None;
                let mut member_context = || {
                    Arc::clone(context.get_or_insert_with(|| {
                        let mut scope = TypeParameterScope::default();
                        let parameters = declaration.type_params();
                        scope.push(parameters.clone(), model);
                        let receiver = ResolvedType::from_kind(ResolvedTypeKind::Interface {
                            occurrence: Default::default(),
                            identity: identity.clone(),
                            arguments: parameters
                                .filter_map(|parameter| scope.lookup(parameter.name().name()))
                                .collect(),
                            nullable: false,
                            extension_type: matches!(declaration, TopLevelDecl::ExtensionType(_)),
                        });
                        let mut environment = TypeEnvironment::new();
                        environment.declare_receiver(receiver);
                        Arc::new((environment, scope))
                    }))
                };
                for member in members {
                    if !matches!(declaration, TopLevelDecl::ExtensionType(_))
                        && inherits_method_types(member)
                    {
                        entries.push((file, Work::Override(identity.clone(), member)));
                    }
                    match member {
                        ClassMember::Field(field) if field.field_type().is_none() => {
                            for declarator in field.declarators() {
                                entries.push((
                                    file,
                                    Work::Member {
                                        identity: identity.clone(),
                                        name: declarator.name().name(),
                                        expression: declarator.initializer(),
                                        context: member_context(),
                                    },
                                ));
                            }
                        }
                        ClassMember::Getter(getter) if getter.return_type().is_none() => {
                            entries.push((
                                file,
                                Work::Member {
                                    identity: identity.clone(),
                                    name: getter.name().name(),
                                    expression: None,
                                    context: member_context(),
                                },
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }
        self.cyclic_values = Self::cyclic_value_identities(&dependencies);
        let mut headers = std::collections::VecDeque::with_capacity(entries.len());
        let mut bodies = std::collections::VecDeque::with_capacity(entries.len());
        let mut queued = vec![true; entries.len()];
        let mut visits = vec![0usize; entries.len()];
        let mut reads = vec![FxHashSet::default(); entries.len()];
        let mut subscribers: FxHashMap<InferenceInput, FxHashSet<usize>> = FxHashMap::default();
        for (id, (_, work)) in entries.iter().enumerate() {
            if work.header() {
                headers.push_back(id);
            } else {
                bodies.push_back(id);
            }
        }
        let mut pending = FxHashMap::default();
        let mut retry_partial = true;
        let mut recording = InferenceRecording::default();
        let mut blocked = std::collections::BTreeSet::new();
        let mut settling = None;
        let mut producer_data = None;
        let mut settlement: Option<std::rc::Rc<std::cell::RefCell<InferenceSettlement>>> = None;
        // A backstop proportional to possible producer/consumer pairs must not
        // reject long chains or wide fan-in. Queue exhaustion and monotone
        // writes establish convergence independently of this generous budget.
        let work_count = entries.len().max(1);
        let visit_limit = work_count.saturating_mul(work_count).saturating_mul(8);
        'worklist: loop {
            let processed = headers.pop_front().or_else(|| bodies.pop_front());
            if let Some(id) = processed {
                queued[id] = false;
                visits[id] += 1;
                assert!(
                    visits[id] <= visit_limit,
                    "declaration inference did not converge for {:?}.{}; previously read inputs: {:?}",
                    entries[id].1.identity(),
                    entries[id].1.name(),
                    reads[id]
                );
                let (file, work) = &entries[id];
                #[cfg(test)]
                INFERENCE_VISITS.with(|visits| {
                    if let Some(counts) = visits.borrow_mut().last_mut() {
                        let input = work.output();
                        *counts.entry(input).or_default() += 1;
                    }
                });
                let model = &programs[*file].1;
                recording.blocked = false;
                recording.settlement = if settling.take() == Some(id) {
                    settlement.clone()
                } else {
                    None
                };
                recording.output = recording.settlement.as_ref().map(|_| work.output());
                let recorder = InferenceRecorder::start(std::mem::take(&mut recording));
                match work {
                    Work::Constructor(entry) => {
                        self.resolve_constructor_entry(entry, model);
                    }
                    Work::Override(identity, member) => {
                        self.inherit_method_types(
                            identity,
                            *member,
                            model,
                            &mut inherited::OverrideInferenceLookup::default(),
                        );
                    }
                    Work::Member {
                        identity,
                        name,
                        expression,
                        context,
                    } => {
                        self.infer_member_type(
                            identity,
                            name,
                            *expression,
                            model,
                            &context.0,
                            &context.1,
                        );
                    }
                    Work::Value(identity, expression) => {
                        self.resolve_value_initializer(
                            identity,
                            *expression,
                            model,
                            &mut pending,
                            retry_partial,
                        );
                    }
                }
                recording = recorder.finish();
                if let Some(state) = &settlement {
                    let state_ref = state.as_ref().borrow();
                    let expanded = recording.reads.difference(&reads[id]).any(|input| {
                        input.as_input() != work.output_ref()
                            && !state_ref.input_stable(input.as_input())
                    });
                    let invalidate = expanded
                        || (recording.blocked
                            && (recording.settlement.is_some() || !blocked.contains(&id)));
                    drop(state_ref);
                    if invalidate {
                        settlement = None;
                    } else if recording.settlement.is_some() {
                        state.borrow_mut().pending.remove(&id);
                    }
                }
                if recording.blocked {
                    blocked.insert(id);
                } else {
                    blocked.remove(&id);
                }
                for input in reads[id].difference(&recording.reads) {
                    subscribers.get_mut(input).unwrap().remove(&id);
                }
                for input in recording.reads.difference(&reads[id]) {
                    subscribers.entry(input.clone()).or_default().insert(id);
                }
                std::mem::swap(&mut reads[id], &mut recording.reads);
                recording.reads.clear();
            } else if !pending.is_empty() {
                recording.blocked = false;
                recording.settlement = None;
                recording.output = None;
                let recorder = InferenceRecorder::start(std::mem::take(&mut recording));
                for (identity, ty) in pending.drain() {
                    self.publish_value_type(&identity, ty);
                }
                retry_partial = false;
                recording = recorder.finish();
            } else {
                if blocked.is_empty() {
                    break;
                }
                let (outputs, producers) = producer_data.get_or_insert_with(|| {
                    let outputs: Vec<_> = entries.iter().map(|(_, work)| work.output()).collect();
                    let mut producers: FxHashMap<InferenceInput, Vec<usize>> = FxHashMap::default();
                    for (id, (_, work)) in entries.iter().enumerate() {
                        producers.entry(outputs[id].clone()).or_default().push(id);
                        if matches!(
                            work,
                            Work::Constructor(
                                index_build::ConstructorInferenceEntry::Declaration { .. }
                            )
                        ) {
                            producers
                                .entry(InferenceInput::Constructors(work.identity().clone()))
                                .or_default()
                                .push(id);
                        }
                    }
                    (outputs, Arc::new(producers))
                });
                let state = settlement.get_or_insert_with(|| {
                    std::rc::Rc::new(std::cell::RefCell::new(InferenceSettlement::new(
                        Arc::clone(producers),
                        outputs,
                        &reads,
                        &blocked,
                    )))
                });
                let id = loop {
                    let Some(id) = state.borrow_mut().order.pop_front() else {
                        break 'worklist;
                    };
                    if blocked.contains(&id) {
                        break id;
                    }
                    state.borrow_mut().pending.remove(&id);
                };
                settling = Some(id);
                queued[id] = true;
                if entries[id].1.header() {
                    headers.push_back(id);
                } else {
                    bodies.push_back(id);
                }
                continue;
            }
            for input in recording.writes.drain() {
                if let Some(dependents) = subscribers.get(&input) {
                    for &id in dependents {
                        if Some(id) != processed && !queued[id] {
                            queued[id] = true;
                            if entries[id].1.header() {
                                headers.push_back(id);
                            } else {
                                bodies.push_back(id);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Fill omitted instance callable types from the native combined inherited
    /// signature, including contravariant parameters and generic alpha-renaming.
    fn inherit_method_types(
        &mut self,
        identity: &DeclarationIdentity,
        member: ClassMember<'_>,
        model: &SemanticModel<'_>,
        lookup: &mut inherited::OverrideInferenceLookup,
    ) {
        let (name, params, infer_return, setter) = match member {
            ClassMember::Method(method) => (
                method.name().name(),
                Some(method.params()),
                method.return_type().is_none(),
                false,
            ),
            ClassMember::Operator(operator) => (
                operator.lookup_name(),
                Some(operator.params()),
                operator.return_type().is_none(),
                false,
            ),
            ClassMember::Setter(setter) => (setter.name().name(), None, false, true),
            _ => return,
        };
        let unresolved = self.override_inference_unresolved(identity, name, setter, model, lookup);
        let inherited = if unresolved {
            None
        } else {
            self.override_inference_signature(identity, name, setter, model, lookup)
        };
        let fallback = if unresolved {
            ResolvedType::UNKNOWN
        } else {
            ResolvedType::DYNAMIC
        };
        if inference_deferred() {
            return;
        }
        let Some(entry) = self
            .member_entries
            .get_mut(identity)
            .and_then(|entries| entries.get_mut(name))
        else {
            return;
        };
        let update = |slot: &mut ResolvedType, ty: ResolvedType| {
            if refine_inferred_type(slot, ty) {
                record_inference_write(|| {
                    InferenceInput::Member(identity.clone(), name.to_owned())
                });
            }
        };
        if setter {
            if let Some(slot) = entry.write.as_mut() {
                let ty = inherited
                    .as_ref()
                    .and_then(|(positional, ..)| positional.first())
                    .unwrap_or(&fallback)
                    .clone();
                update(slot, ty);
            }
            return;
        }
        let Some(signature) = entry.signature.as_mut() else {
            return;
        };
        let untyped = |param: FormalParam<'_>| {
            param.param_type().is_none()
                && param.function_params().is_none()
                && param.initializing().is_none()
        };
        let params = params.expect("method or operator parameters");
        for (index, (param, slot)) in params
            .positional()
            .zip(&mut signature.positional)
            .enumerate()
        {
            if untyped(param) {
                let ty = inherited
                    .as_ref()
                    .and_then(|(positional, ..)| positional.get(index))
                    .unwrap_or(&fallback)
                    .clone();
                update(slot, ty);
            }
        }
        for param in params.named().filter(|param| untyped(*param)) {
            if let Some(slot) = signature.named.get_mut(param.name().name()) {
                let ty = inherited
                    .as_ref()
                    .and_then(|(_, named, _)| named.get(param.name().name()))
                    .unwrap_or(&fallback)
                    .clone();
                update(slot, ty);
            }
        }
        if infer_return {
            let ty = inherited
                .map(|(_, _, return_type)| return_type)
                .unwrap_or(fallback);
            update(&mut signature.return_type, ty);
        }
    }

    fn infer_member_type(
        &mut self,
        identity: &DeclarationIdentity,
        name: &str,
        expression: Option<Expr<'_>>,
        model: &SemanticModel<'_>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
    ) {
        let Some(entry) = self.member_entry(identity, name) else {
            return;
        };
        let Some((_, current)) = &entry.field else {
            return;
        };
        if !has_inference_holes(current) {
            return;
        }
        let mut inherited = None;
        if entry.facts.iter().any(|fact| !fact.is_static)
            && let Some((_, supertypes)) = self
                .type_facts
                .get(identity)
                .and_then(|facts| facts.supertypes.as_ref())
        {
            for supertype in supertypes {
                let Some((declared, substitutions)) = self.resolved_field(supertype, name, model)
                else {
                    continue;
                };
                let ty = model.substitute(&declared, &substitutions);
                // A super member may itself still need an inference pass.
                // Do not freeze an initializer/body type ahead of that pass.
                if has_inference_holes(&ty) {
                    return;
                }
                if inherited.as_ref().is_some_and(|previous| *previous != ty) {
                    inherited = Some(ResolvedType::UNKNOWN);
                    break;
                }
                inherited = Some(ty);
            }
        }
        let ty = if let Some(ty) = inherited {
            ty
        } else if let Some(expression) = expression {
            if !self.declaration_dependencies_known(
                expression,
                model,
                Some(identity),
                environment,
                scope,
            ) {
                return;
            }
            let lookup = |name: &str| {
                self.member_entry(identity, name)
                    .and_then(|entry| entry.field.as_ref().map(|(_, ty)| ty.clone()))
            };
            let ty = environment
                .infer_with_signatures_using_lookup(expression, model, self, scope, &lookup);
            if has_inference_holes(&ty) {
                return;
            }
            ty
        } else if entry.implicit_getter {
            // Getter bodies do not participate in declaration inference. An
            // omitted getter with no inherited evidence retains its default.
            ResolvedType::DYNAMIC
        } else {
            return;
        };
        if inference_deferred() || *current == ty {
            return;
        }
        let entry = self
            .member_entries
            .get_mut(identity)
            .unwrap()
            .get_mut(name)
            .unwrap();
        let mut changed = refine_inferred_type(&mut entry.field.as_mut().unwrap().1, ty.clone());
        if let Some(read) = entry.read.as_mut() {
            changed |= refine_inferred_type(read, ty.clone());
        }
        if !entry.implicit_getter
            && let Some(write) = entry.write.as_mut()
        {
            changed |= refine_inferred_type(write, ty);
        }
        if changed {
            record_inference_write(|| InferenceInput::Member(identity.clone(), name.to_owned()));
        }
    }

    fn member_type_pending(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        self.member_entry(identity, name)
            .and_then(|entry| entry.field.as_ref())
            .is_some_and(|(_, ty)| has_inference_holes(ty))
    }

    fn declaration_dependencies_known(
        &self,
        expression: Expr<'_>,
        model: &SemanticModel<'_>,
        owner: Option<&DeclarationIdentity>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
    ) -> bool {
        self.declaration_dependencies_known_using_lookup(
            expression,
            model,
            owner,
            environment,
            scope,
            &|name| owner.and_then(|owner| self.field(owner, name).map(|(_, ty)| ty.clone())),
        )
    }

    pub(super) fn declaration_dependencies_known_using_lookup(
        &self,
        expression: Expr<'_>,
        model: &SemanticModel<'_>,
        owner: Option<&DeclarationIdentity>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> bool {
        let mut unresolved = false;
        // Each field result is retained once, so a.b.c does not infer a.b again for c.
        let mut receivers: FxHashMap<NodeId, ResolvedType> = FxHashMap::default();
        let mut member_receivers: FxHashMap<NodeId, ResolvedType> = FxHashMap::default();
        InitializerDependencies {
            environment: environment.clone(),
            inspect: |expression: Expr<'_>, environment: &TypeEnvironment| {
                if let Some(name) = initializer_value_name(expression)
                    && !environment.is_bound(name.prefix.unwrap_or(name.name))
                {
                    let member_owner = match name.prefix {
                        None => owner.cloned(),
                        Some(prefix) => model.resolve_name(prefix),
                    };
                    let member = member_owner.as_ref().and_then(|owner| {
                        self.member_entry(owner, name.name)
                            .and_then(|entry| entry.field.as_ref().map(|(_, ty)| ty))
                    });
                    let variable = model.resolve_value(name).filter(|identity| {
                        // A cycle's Unknown is terminal, not work waiting for
                        // another pass. Its acyclic consumers can still infer.
                        !self.is_cyclic_value(identity)
                    });
                    unresolved |= member
                        .or_else(|| {
                            variable
                                .as_ref()
                                .and_then(|identity| self.value_type(identity))
                        })
                        .is_some_and(has_inference_holes);
                    if let Some(owner) = member_owner {
                        unresolved |= self.member_type_pending(&owner, name.name);
                    }
                }
                if let Expr::Field(access) = expression {
                    let object = access.object();
                    let field = access.field();
                    let static_owner = initializer_value_name(object)
                        .filter(|name| !environment.is_bound(name.prefix.unwrap_or(name.name)))
                        .and_then(|name| model.resolve_name(name));
                    if let Some(identity) = &static_owner {
                        unresolved |= self.member_type_pending(identity, field.name());
                    }
                    let receiver = receivers.remove(&object.syntax().id()).unwrap_or_else(|| {
                        environment.receiver_type(
                            object,
                            field.name(),
                            &InferenceContext {
                                model,
                                signatures: self,
                                type_parameters: scope,
                                lookup,
                                argument_fallback: None,
                            },
                            None,
                        )
                    });
                    // Cache reached-branch types separately from the null
                    // introduced by shorting, retaining declared nullability.
                    let receiver = if access.is_null_safe() {
                        receiver.non_null()
                    } else {
                        receiver
                    };
                    if let Some(ResolvedType {
                        kind: ResolvedTypeKind::Interface { identity, .. },
                    }) = self.resolved_instance_owner(
                        &receiver,
                        field.name(),
                        InstanceAccess::Read,
                        model,
                    ) {
                        unresolved |= self.member_type_pending(&identity, field.name());
                    }
                    let ty = static_owner
                        .as_ref()
                        .and_then(|identity| self.field(identity, field.name()))
                        .map(|(_, ty)| ty.clone())
                        .or_else(|| {
                            self.resolved_field(&receiver, field.name(), model)
                                .or_else(|| {
                                    self.resolved_extension_getter(&receiver, field.name(), model)
                                })
                                .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
                        })
                        .unwrap_or(ResolvedType::UNKNOWN);
                    receivers.insert(expression.syntax().id(), ty);
                    member_receivers.insert(expression.syntax().id(), receiver);
                } else if let Expr::Call(call) = expression {
                    let target = match call.callee() {
                        Expr::GenericInstantiation(instantiation) => instantiation.target(),
                        target => target,
                    };
                    if let Some(receiver) = member_receivers.remove(&target.syntax().id()) {
                        let ty = environment.infer_unshorted(
                            expression,
                            InferenceContext {
                                model,
                                signatures: self,
                                type_parameters: scope,
                                lookup,
                                argument_fallback: None,
                            },
                            None,
                            Some(receiver),
                        );
                        receivers.insert(expression.syntax().id(), ty);
                    }
                }
            },
        }
        .visit_expr(expression);
        !unresolved
    }

    /// Block promotion of private getters that a concrete `noSuchMethod`
    /// forwards. Each lookup views members from the library that owns the
    /// declaration or private name, never from an arbitrary indexed file.
    pub(super) fn mark_field_forwarders(
        &mut self,
        identities: &IdentityIndex,
        types: Option<&TypeIndex>,
    ) {
        self.has_field_forwarders = self
            .external
            .as_ref()
            .is_some_and(|base| base.has_field_forwarders)
            || self.member_entries.local.iter().any(|(owner, entries)| {
                !owner.is_sdk("dart:core", "Object")
                    && entries.get("noSuchMethod").is_some_and(|entry| {
                        entry
                            .facts
                            .iter()
                            .any(|fact| !fact.is_static && !fact.is_abstract)
                    })
            });
        if !self.has_field_forwarders {
            return;
        }
        let mut blockers = HashSet::new();
        for (identity, declaration) in
            self.type_facts
                .local
                .iter()
                .filter_map(|(identity, facts)| {
                    facts.declaration.as_ref().map(|facts| (identity, facts))
                })
        {
            if declaration.is_abstract {
                continue;
            }
            let receiver = ResolvedType::from_kind(ResolvedTypeKind::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: empty_type_arguments(),
                nullable: false,
                extension_type: false,
            });
            let declaring = SemanticModel::in_library(
                identities,
                types,
                identities.declaration_library(identity),
            );
            let Some(ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity: owner, ..
                    },
            }) = self.resolved_owner_through(
                &receiver,
                "noSuchMethod",
                InstanceAccess::Invoke,
                false,
                &declaring,
            )
            else {
                continue;
            };
            if owner.is_sdk("dart:core", "Object")
                || !self
                    .member_entry(&owner, "noSuchMethod")
                    .is_some_and(|entry| {
                        entry
                            .facts
                            .iter()
                            .any(|fact| !fact.is_static && !fact.is_abstract)
                    })
            {
                continue;
            }
            let mut getters = HashSet::new();
            self.abstract_private_getters(identity, &mut HashSet::new(), &mut getters);
            for (name, library) in getters {
                if self
                    .blocked_field_promotions
                    .get(&library)
                    .is_some_and(|names| names.contains(name))
                {
                    continue;
                }
                let private_scope = SemanticModel::in_library(identities, types, Some(library));
                let implemented = self.resolved_owner_through(&receiver, name, InstanceAccess::Read, false, &private_scope)
                    .and_then(|owner| match owner {
                        ResolvedType { kind: ResolvedTypeKind::Interface { identity: owner, .. } } => {
                            let same_library = matches!(&owner, DeclarationIdentity::Library { library: owner_library, .. } if *owner_library == library);
                            Some(same_library && self.member_entry(&owner, name).is_some_and(|entry|
                                entry.facts.iter().any(|fact| !fact.is_static && !fact.is_abstract)))
                        }
                        _ => None,
                    }).unwrap_or(false);
                if !implemented {
                    blockers.insert((library, name));
                }
            }
        }
        let blockers: Vec<_> = blockers
            .into_iter()
            .map(|(library, name)| (library, name.to_owned()))
            .collect();
        for (library, name) in blockers {
            self.blocked_field_promotions
                .entry(library)
                .or_default()
                .insert(name);
        }
    }

    fn abstract_private_getters<'a>(
        &'a self,
        identity: &DeclarationIdentity,
        visited: &mut HashSet<DeclarationIdentity>,
        getters: &mut HashSet<(&'a str, LibraryId)>,
    ) {
        if !visited.insert(identity.clone()) {
            return;
        }
        if let DeclarationIdentity::Library { library, .. } = identity
            && let Some(entries) = self.member_entries.get(identity)
        {
            for (name, entry) in entries {
                if name.starts_with('_')
                    && entry.facts.iter().any(|fact| {
                        !fact.is_static && fact.is_abstract && fact.kind == MemberKind::Getter
                    })
                {
                    getters.insert((name.as_str(), *library));
                }
            }
        }
        if let Some((_, supertypes)) = self
            .type_facts
            .get(identity)
            .and_then(|facts| facts.supertypes.as_ref())
        {
            for supertype in supertypes {
                if let ResolvedType {
                    kind: ResolvedTypeKind::Interface { identity, .. },
                } = supertype
                {
                    self.abstract_private_getters(identity, visited, getters);
                }
            }
        }
    }
}

#[cfg(test)]
mod recording_tests {
    use super::*;

    #[test]
    fn inference_recordings_are_exact_nested_and_unwind_safe() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SignatureIndex>();
        let index = SignatureIndex::default();
        let owner = DeclarationIdentity::sdk("test", "Owner");
        let unrelated = DeclarationIdentity::sdk("test", "Unrelated");
        let outer = InferenceRecorder::start(InferenceRecording::default());
        index.member_entry(&owner, "first");
        index.member_entry(&owner, "first");
        index.is_const_constructor(&owner, "new");
        {
            let inner = InferenceRecorder::start(InferenceRecording::default());
            index.member_entry(&unrelated, "first");
            let recording = inner.finish();
            assert_eq!(
                recording.reads,
                FxHashSet::from_iter([InferenceInput::Member(unrelated.clone(), "first".into())])
            );
        }
        let unwind = std::panic::catch_unwind(|| {
            let _inner = InferenceRecorder::start(InferenceRecording::default());
            record_inference_read(InferenceInputRef::Value(&unrelated));
            panic!("exercise recorder cleanup");
        });
        assert!(unwind.is_err());
        index.member_entry(&owner, "second");
        let recording = outer.finish();
        assert_eq!(
            recording.reads,
            FxHashSet::from_iter([
                InferenceInput::Member(owner.clone(), "first".into()),
                InferenceInput::Member(owner.clone(), "second".into()),
                InferenceInput::Constructors(owner),
            ])
        );
        assert!(recording.writes.is_empty());
        INFERENCE_RECORDINGS.with(|recordings| assert!(recordings.borrow().is_empty()));
    }
}
