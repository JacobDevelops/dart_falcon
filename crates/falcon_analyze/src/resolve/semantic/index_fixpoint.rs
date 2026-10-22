use super::*;

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
        self.environment.declare(name, ResolvedType::Unknown);
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
                        .declare(variable.name().name(), ResolvedType::Unknown);
                }
                if let falcon_syntax::cst::Pattern::Object(object) = pattern {
                    for field in object.fields() {
                        if field.pattern().is_none() {
                            self.0.declare(field.name().name(), ResolvedType::Unknown);
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

impl<'a, F: FnMut(Expr<'a>, &TypeEnvironment)> Visitor<'a> for InitializerDependencies<F> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        if let Expr::FuncExpr(function) = expression {
            self.scoped(|visitor| {
                for parameter in function.params().params() {
                    visitor.bind(parameter.name().name());
                }
                walk_expr(visitor, expression);
                (visitor.inspect)(expression, &visitor.environment);
            });
        } else {
            walk_expr(self, expression);
            (self.inspect)(expression, &self.environment);
        }
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

    fn visit_collection_element(&mut self, element: falcon_syntax::cst::CollectionElement<'a>) {
        use falcon_syntax::cst::CollectionElement;
        match element {
            CollectionElement::For(_) => self.scoped(|visitor| {
                falcon_syntax::cst::visit::walk_collection_element(visitor, element)
            }),
            CollectionElement::If(branch) => {
                self.condition(branch.condition(), |visitor| {
                    visitor.visit_collection_element(branch.then_elem())
                });
                if let Some(element) = branch.else_elem() {
                    self.visit_collection_element(element);
                }
            }
            _ => falcon_syntax::cst::visit::walk_collection_element(self, element),
        }
    }

    fn visit_map_element(&mut self, element: falcon_syntax::cst::MapElement<'a>) {
        use falcon_syntax::cst::MapElement;
        match element {
            MapElement::For(_) => {
                self.scoped(|visitor| falcon_syntax::cst::visit::walk_map_element(visitor, element))
            }
            MapElement::If(branch) => {
                self.condition(branch.condition(), |visitor| {
                    visitor.visit_map_element(branch.then_entry())
                });
                if let Some(element) = branch.else_entry() {
                    self.visit_map_element(element);
                }
            }
            _ => falcon_syntax::cst::visit::walk_map_element(self, element),
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

    fn resolve_value_initializers(
        &mut self,
        program: Program<'_>,
        model: &SemanticModel<'_>,
        pending: &mut FxHashMap<DeclarationIdentity, ResolvedType>,
        retry_partial: bool,
    ) -> bool {
        let mut changed = false;
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
                let Some(identity) = model.resolve_value(declarator.name().name()) else {
                    continue;
                };
                if self.is_cyclic_value(&identity) {
                    continue;
                }
                if self
                    .value_types
                    .get(&identity)
                    .is_some_and(|ty| !has_inference_holes(ty))
                {
                    continue;
                }
                let Some(initializer) = declarator.initializer() else {
                    continue;
                };
                let ty = TypeEnvironment::new().infer_with_signatures(
                    initializer,
                    model,
                    self,
                    &TypeParameterScope::default(),
                );
                let ty = if ty == ResolvedType::Null {
                    ResolvedType::Dynamic
                } else {
                    ty
                };
                if has_inference_holes(&ty) {
                    // Retain partial evidence until concrete progress stops.
                    // Once published, only accept a hole-free replacement:
                    // feeding partial refinements back would grow recursive
                    // containers indefinitely.
                    if ty != ResolvedType::Unknown
                        && (retry_partial
                            || self
                                .value_types
                                .get(&identity)
                                .is_none_or(|published| *published == ResolvedType::Unknown))
                    {
                        pending.insert(identity, ty);
                    } else {
                        pending.remove(&identity);
                    }
                } else {
                    if retry_partial
                        && !self.declaration_dependencies_known(
                            initializer,
                            model,
                            None,
                            &TypeEnvironment::new(),
                            &TypeParameterScope::default(),
                        )
                    {
                        // Replace stale partial evidence even when this
                        // concrete candidate must wait for a dependency.
                        pending.insert(identity, ty);
                        continue;
                    }
                    pending.remove(&identity);
                    self.value_types.insert(identity, ty);
                    changed = true;
                }
            }
        }
        changed
    }

    pub(super) fn add_program(&mut self, program: Program<'_>, model: &SemanticModel<'_>) {
        self.add_declarations(program, model, true);
        self.finish();
        self.infer_programs(&[(program, *model)]);
        self.mark_field_forwarders(model.identities, model.types);
    }

    fn converge(
        &mut self,
        programs: &[(Program<'_>, SemanticModel<'_>)],
        mut step: impl FnMut(&mut Self, Program<'_>, &SemanticModel<'_>) -> bool,
    ) {
        loop {
            let mut changed = false;
            for (program, model) in programs {
                changed |= step(self, *program, model);
            }
            if !changed {
                break;
            }
        }
    }

    pub(super) fn infer_programs(&mut self, programs: &[(Program<'_>, SemanticModel<'_>)]) {
        let mut dependencies = FxHashMap::default();
        for (program, model) in programs {
            Self::collect_value_dependencies(*program, model, &mut dependencies);
        }
        self.cyclic_values = Self::cyclic_value_identities(&dependencies);
        // Settle forwarded and inherited headers before initializers can freeze implicit types.
        self.converge(programs, Self::resolve_constructor_formals);
        self.converge(programs, Self::infer_override_types);
        let mut pending_values = FxHashMap::default();
        let mut retry_partial = true;
        loop {
            loop {
                let mut declarations_changed = false;
                let mut values_changed = false;
                for (program, model) in programs {
                    declarations_changed |= self.infer_declaration_types(*program, model);
                    values_changed |= self.resolve_value_initializers(
                        *program,
                        model,
                        &mut pending_values,
                        retry_partial,
                    );
                }
                if declarations_changed {
                    self.converge(programs, Self::resolve_constructor_formals);
                }
                if !declarations_changed && !values_changed {
                    break;
                }
            }
            if pending_values.is_empty() {
                break;
            }
            // Published partials may become concrete, but never expand to another partial.
            self.value_types.extend(pending_values.drain());
            retry_partial = false;
        }
    }

    /// Propagate callable headers before initializers and revisit dependencies
    /// whose inherited getter or field types were previously unresolved.
    pub(super) fn infer_override_types(
        &mut self,
        program: Program<'_>,
        model: &SemanticModel<'_>,
    ) -> bool {
        let mut changed = false;
        let mut lookup = inherited::OverrideInferenceLookup::default();
        for declaration in program.declarations() {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) => (class.name(), class.members()),
                TopLevelDecl::MixinClass(class) => (class.name(), class.members()),
                TopLevelDecl::Mixin(mixin) => (mixin.name(), mixin.members()),
                TopLevelDecl::Enum(enumeration) => (enumeration.name(), enumeration.members()),
                // Extension type members do not override implemented members.
                _ => continue,
            };
            let Some(identity) = model.resolve_name(name.name()) else {
                continue;
            };
            for member in members.filter(|member| inherits_method_types(*member)) {
                changed |= self.inherit_method_types(&identity, member, model, &mut lookup);
            }
        }
        changed
    }

    /// Advance callable, field, and getter inference to a fixed point.
    /// Unresolved initializer dependencies remain holes for later passes.
    fn infer_declaration_types(&mut self, program: Program<'_>, model: &SemanticModel<'_>) -> bool {
        let mut changed = self.infer_override_types(program, model);
        for declaration in program.declarations() {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let parameters = declaration.type_params();
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) => (class.name(), class.members()),
                TopLevelDecl::MixinClass(class) => (class.name(), class.members()),
                TopLevelDecl::Mixin(mixin) => (mixin.name(), mixin.members()),
                TopLevelDecl::Enum(enumeration) => (enumeration.name(), enumeration.members()),
                TopLevelDecl::ExtensionType(extension) => (extension.name(), extension.members()),
                _ => continue,
            };
            let Some(identity) = model.resolve_name(name.name()) else {
                continue;
            };
            if !members.clone().any(|member| match member {
                ClassMember::Field(field) if field.field_type().is_none() => {
                    field.declarators().any(|declarator| {
                        self.member_entry(&identity, declarator.name().name())
                            .and_then(|entry| entry.field.as_ref())
                            .is_some_and(|(_, ty)| {
                                matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic)
                            })
                    })
                }
                ClassMember::Getter(getter) if getter.return_type().is_none() => self
                    .member_type_pending(
                        &identity,
                        getter.name().name(),
                        model,
                        &mut FxHashSet::default(),
                    ),
                _ => false,
            }) {
                continue;
            }
            let mut scope = TypeParameterScope::default();
            scope.push(parameters.clone(), model);
            let receiver = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: parameters
                    .filter_map(|parameter| scope.lookup(parameter.name().name()))
                    .collect(),
                nullable: false,
                extension_type: matches!(declaration, TopLevelDecl::ExtensionType(_)),
            };
            let mut environment = TypeEnvironment::new();
            environment.declare("this", receiver);
            for member in members {
                match member {
                    ClassMember::Field(field) if field.field_type().is_none() => {
                        for declarator in field.declarators() {
                            changed |= self.infer_member_type(
                                &identity,
                                declarator.name().name(),
                                declarator.initializer(),
                                model,
                                &environment,
                                &scope,
                            );
                        }
                    }
                    ClassMember::Getter(getter)
                        if getter.return_type().is_none()
                            && self.member_type_pending(
                                &identity,
                                getter.name().name(),
                                model,
                                &mut FxHashSet::default(),
                            ) =>
                    {
                        changed |= self.infer_member_type(
                            &identity,
                            getter.name().name(),
                            None,
                            model,
                            &environment,
                            &scope,
                        );
                    }
                    _ => {}
                }
            }
        }
        changed
    }

    /// Fill omitted instance callable types from the native combined inherited
    /// signature, including contravariant parameters and generic alpha-renaming.
    fn inherit_method_types(
        &mut self,
        identity: &DeclarationIdentity,
        member: ClassMember<'_>,
        model: &SemanticModel<'_>,
        lookup: &mut inherited::OverrideInferenceLookup,
    ) -> bool {
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
            _ => return false,
        };
        let unresolved = self.override_inference_unresolved(identity, name, setter, model, lookup);
        let inherited = if unresolved {
            None
        } else {
            self.override_inference_signature(identity, name, setter, model, lookup)
        };
        let fallback = if unresolved {
            ResolvedType::Unknown
        } else {
            ResolvedType::Dynamic
        };
        let Some(entry) = self
            .member_entries
            .get_mut(identity)
            .and_then(|entries| entries.get_mut(name))
        else {
            return false;
        };
        let mut changed = false;
        let mut update = |slot: &mut ResolvedType, ty: ResolvedType| {
            if *slot != ty {
                *slot = ty;
                changed = true;
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
            return changed;
        }
        let Some(signature) = entry.signature.as_mut() else {
            return false;
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
        changed
    }

    fn infer_member_type(
        &mut self,
        identity: &DeclarationIdentity,
        name: &str,
        expression: Option<Expr<'_>>,
        model: &SemanticModel<'_>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
    ) -> bool {
        let Some(entry) = self.member_entry(identity, name) else {
            return false;
        };
        let Some((_, current)) = &entry.field else {
            return false;
        };
        if !has_inference_holes(current) && !matches!(current, ResolvedType::Dynamic) {
            return false;
        }
        let mut inherited = None;
        if entry.facts.iter().any(|fact| !fact.is_static)
            && let Some((_, supertypes)) = self.supertypes.get(identity)
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
                    return false;
                }
                if inherited.as_ref().is_some_and(|previous| *previous != ty) {
                    inherited = Some(ResolvedType::Unknown);
                    break;
                }
                inherited = Some(ty);
            }
        }
        let ty = if let Some(ty) = inherited {
            ty
        } else {
            let Some(expression) = expression else {
                // Getter bodies do not participate in declaration inference.
                return false;
            };
            if !self.declaration_dependencies_known(
                expression,
                model,
                Some(identity),
                environment,
                scope,
            ) {
                return false;
            }
            let lookup = |name: &str| {
                self.member_entry(identity, name)
                    .and_then(|entry| entry.field.as_ref().map(|(_, ty)| ty.clone()))
            };
            let ty = environment
                .infer_with_signatures_using_lookup(expression, model, self, scope, &lookup);
            if has_inference_holes(&ty) {
                return false;
            }
            ty
        };
        if *current == ty {
            return false;
        }
        let entry = self
            .member_entries
            .get_mut(identity)
            .unwrap()
            .get_mut(name)
            .unwrap();
        entry.field.as_mut().unwrap().1 = ty.clone();
        if entry
            .read
            .as_ref()
            .is_some_and(|ty| matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
        {
            entry.read = Some(ty.clone());
        }
        if entry
            .write
            .as_ref()
            .is_some_and(|ty| matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
        {
            entry.write = Some(ty);
        }
        true
    }

    /// An implicit dynamic getter may still inherit its type in a later
    /// pass. Explicit dynamic getters and dynamic fields are already complete.
    fn member_type_pending(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        model: &SemanticModel<'_>,
        visited: &mut FxHashSet<DeclarationIdentity>,
    ) -> bool {
        if !visited.insert(identity.clone()) {
            return false;
        }
        let Some(entry) = self.member_entry(identity, name) else {
            return false;
        };
        let Some((_, ty)) = &entry.field else {
            return false;
        };
        if has_inference_holes(ty) {
            return true;
        }
        if !entry.implicit_getter
            || !matches!(ty, ResolvedType::Dynamic)
            || entry.facts.iter().all(|fact| fact.is_static)
        {
            return false;
        }
        self.supertypes
            .get(identity)
            .is_some_and(|(_, supertypes)| {
                supertypes.iter().any(|supertype| {
                    let Some(ResolvedType::Interface { identity, .. }) =
                        self.resolved_instance_owner(supertype, name, InstanceAccess::Read, model)
                    else {
                        return false;
                    };
                    self.member_entry(&identity, name)
                        .and_then(|entry| entry.field.as_ref())
                        .is_some_and(|(_, ty)| !matches!(ty, ResolvedType::Dynamic))
                        || self.member_type_pending(&identity, name, model, visited)
                })
            })
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
        let mut visited = FxHashSet::default();
        InitializerDependencies {
            environment: environment.clone(),
            inspect: |expression: Expr<'_>, environment: &TypeEnvironment| {
                if unresolved {
                    return;
                }
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
                        visited.clear();
                        unresolved |=
                            self.member_type_pending(&owner, name.name, model, &mut visited);
                    }
                }
                if let Expr::Field(access) = expression {
                    let object = access.object();
                    let field = access.field();
                    let static_owner = initializer_value_name(object)
                        .filter(|name| !environment.is_bound(name.prefix.unwrap_or(name.name)))
                        .and_then(|name| model.resolve_name(name));
                    if let Some(identity) = &static_owner {
                        visited.clear();
                        unresolved |=
                            self.member_type_pending(identity, field.name(), model, &mut visited);
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
                    if let Some(ResolvedType::Interface { identity, .. }) = self
                        .resolved_instance_owner(
                            &receiver,
                            field.name(),
                            InstanceAccess::Read,
                            model,
                        )
                    {
                        visited.clear();
                        unresolved |=
                            self.member_type_pending(&identity, field.name(), model, &mut visited);
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
                        .unwrap_or(ResolvedType::Unknown);
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
        for (identity, declaration) in self.declarations.local.iter() {
            if declaration.is_abstract {
                continue;
            }
            let receiver = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: empty_type_arguments(),
                nullable: false,
                extension_type: false,
            };
            let declaring = SemanticModel::in_library(
                identities,
                types,
                identities.declaration_library(identity),
            );
            let Some(ResolvedType::Interface {
                identity: owner, ..
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
                        ResolvedType::Interface { identity: owner, .. } => {
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
        if let Some((_, supertypes)) = self.supertypes.get(identity) {
            for supertype in supertypes {
                if let ResolvedType::Interface { identity, .. } = supertype {
                    self.abstract_private_getters(identity, visited, getters);
                }
            }
        }
    }
}
