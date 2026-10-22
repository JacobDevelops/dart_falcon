use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FlowBinding {
    declared: ResolvedType,
    /// Source identity of a syntax binding, absent for synthetic bindings.
    declaration: Option<Span>,
    /// Dart's promotion chain: each type a subtype of the previous; the last is current.
    promoted: Vec<ResolvedType>,
    /// Types tested or promoted to, kept across writes as types of interest.
    tested: Vec<ResolvedType>,
    pub(super) promotable: bool,
    pub(super) captured: bool,
    local_function: bool,
    fields: HashMap<String, ResolvedType>,
}

impl FlowBinding {
    fn push_promotion(&mut self, ty: ResolvedType) {
        if !self.tested.contains(&ty) {
            self.tested.push(ty.clone());
        }
        if self.promoted.last() != Some(&ty) {
            self.promoted.push(ty);
        }
    }
}

/// Lexical flow state with copy-on-write scopes for independent branch snapshots.
#[derive(Debug, Clone, Default)]
pub struct TypeEnvironment {
    // Branch snapshots share scopes until a write touches that scope.
    pub(super) scopes: Vec<Arc<FxHashMap<String, FlowBinding>>>,
    callable_boundaries: Vec<usize>,
    /// Whether the library's language version promotes private fields.
    pub(super) field_promotions: bool,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Arc::new(FxHashMap::default())],
            callable_boundaries: Vec::new(),
            field_promotions: false,
        }
    }

    /// An empty environment whose expression inference promotes private
    /// fields when `field_promotions`.
    pub fn with_field_promotions(field_promotions: bool) -> Self {
        Self {
            field_promotions,
            ..Self::new()
        }
    }

    /// Whether a shared snapshot still has exactly this environment's flow state.
    pub(crate) fn same_flow_state(&self, other: &Self) -> bool {
        self.field_promotions == other.field_promotions
            && self.callable_boundaries == other.callable_boundaries
            && self.scopes.len() == other.scopes.len()
            && self
                .scopes
                .iter()
                .zip(&other.scopes)
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }

    /// The flow analysis promoting within expressions this environment infers.
    pub(super) fn flow<'a, 'model>(
        &self,
        model: &'a SemanticModel<'model>,
        signatures: &'a SignatureIndex,
        type_parameters: &'a TypeParameterScope,
    ) -> super::flow::FlowContext<'a, 'model> {
        super::flow::FlowContext {
            model,
            signatures,
            type_parameters,
            field_promotions: self.field_promotions,
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Arc::new(FxHashMap::default()));
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn declare(&mut self, name: impl Into<String>, ty: ResolvedType) {
        let name = name.into();
        if let ResolvedType::NonNullTypeParameter { id, bound } = ty {
            // Inferred locals declare `T`, then promote to `T & Object`.
            // A later write of `T` must be able to discard the intersection.
            let declared = ResolvedType::TypeParameter {
                id: id.clone(),
                bound: bound.clone(),
                nullable: false,
            };
            self.declare_binding(name, declared, true)
                .promoted
                .push(ResolvedType::NonNullTypeParameter { id, bound });
        } else {
            self.declare_binding(name, ty, true);
        }
    }

    /// Declares a syntax binding, retaining its lexical declaration identity.
    pub fn declare_at(&mut self, name: impl Into<String>, ty: ResolvedType, span: Span) {
        self.declare_binding(name.into(), ty, true).declaration = Some(span);
    }

    /// The declaration of the nearest visible binding, not its reusable scope depth.
    pub fn binding_declaration(&self, name: &str) -> Option<Span> {
        self.binding(name)?.declaration
    }

    pub fn declare_unpromotable(&mut self, name: impl Into<String>, ty: ResolvedType) {
        self.declare_binding(name.into(), ty, false);
    }

    fn declare_binding(
        &mut self,
        name: String,
        ty: ResolvedType,
        promotable: bool,
    ) -> &mut FlowBinding {
        Arc::make_mut(self.scopes.last_mut().expect("root scope"))
            .entry(name)
            .insert_entry(FlowBinding {
                declared: ty,
                declaration: None,
                promoted: Vec::new(),
                tested: Vec::new(),
                promotable,
                captured: false,
                local_function: false,
                fields: HashMap::new(),
            })
            .into_mut()
    }

    pub fn declare_local_function(&mut self, name: impl Into<String>, ty: ResolvedType) {
        self.declare_binding(name.into(), ty, true).local_function = true;
    }

    /// Declares a local function with the identity of its source name.
    pub fn declare_local_function_at(
        &mut self,
        name: impl Into<String>,
        ty: ResolvedType,
        span: Span,
    ) {
        let binding = self.declare_binding(name.into(), ty, true);
        binding.local_function = true;
        binding.declaration = Some(span);
    }

    pub fn is_local_function(&self, name: &str) -> bool {
        self.binding(name)
            .is_some_and(|binding| binding.local_function)
    }

    pub fn mark_captured(&mut self, name: &str) {
        if self.binding(name).is_some_and(|binding| {
            !binding.captured
                || binding.promotable
                || !binding.promoted.is_empty()
                || !binding.fields.is_empty()
        }) && let Some(binding) = self.binding_mut(name)
        {
            binding.captured = true;
            binding.promotable = false;
            binding.promoted.clear();
            binding.fields.clear();
        }
    }

    pub fn promoted_this_field(&self, field: &str) -> Option<ResolvedType> {
        let scope = self
            .scopes
            .iter()
            .rev()
            .find(|scope| scope.contains_key("this"))?;
        if let Some(binding) = scope.get(field) {
            return binding.promoted.last().cloned();
        }
        scope.get("this")?.fields.get(field).cloned()
    }

    pub fn promote_this_field(&mut self, field: &str, ty: ResolvedType) {
        if let Some(scope) = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key("this"))
        {
            if let Some(binding) = scope.get(field) {
                if !binding.promotable
                    || binding.captured
                    || (binding.promoted.last() == Some(&ty) && binding.tested.contains(&ty))
                {
                    return;
                }
            } else if scope
                .get("this")
                .is_some_and(|binding| binding.fields.get(field) == Some(&ty))
            {
                return;
            }
            let scope = Arc::make_mut(scope);
            if let Some(binding) = scope.get_mut(field) {
                if binding.promotable && !binding.captured {
                    binding.push_promotion(ty);
                }
            } else if let Some(binding) = scope.get_mut("this") {
                binding.fields.insert(field.to_owned(), ty);
            }
        }
    }

    pub(super) fn binding(&self, name: &str) -> Option<&FlowBinding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn binding_mut(&mut self, name: &str) -> Option<&mut FlowBinding> {
        let scope = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key(name))?;
        Arc::make_mut(scope).get_mut(name)
    }

    pub fn declared_type(&self, name: &str) -> ResolvedType {
        self.binding(name)
            .map(|binding| binding.declared.clone())
            .unwrap_or(ResolvedType::Unknown)
    }

    /// The promotion chain of `name`, most general first.
    pub fn promotion_chain(&self, name: &str) -> &[ResolvedType] {
        self.binding(name)
            .map_or(&[], |binding| binding.promoted.as_slice())
    }

    /// The types of interest recorded for `name`, besides its declared type.
    pub fn tested_types(&self, name: &str) -> &[ResolvedType] {
        self.binding(name)
            .map_or(&[], |binding| binding.tested.as_slice())
    }

    /// Records the actual tested type, independently of the branch's promotion.
    pub(crate) fn record_tested_type(&mut self, name: &str, ty: ResolvedType) {
        if ty != ResolvedType::Unknown
            && self
                .binding(name)
                .is_some_and(|binding| !binding.tested.contains(&ty))
            && let Some(binding) = self.binding_mut(name)
        {
            binding.tested.push(ty);
        }
    }

    /// Applies a flow refinement without making it a tested type of interest.
    pub(crate) fn promote_without_test(&mut self, name: &str, ty: ResolvedType) {
        if ty != ResolvedType::Unknown
            && self.binding(name).is_some_and(|binding| {
                binding.promotable && !binding.captured && binding.promoted.last() != Some(&ty)
            })
            && let Some(binding) = self.binding_mut(name)
        {
            binding.promoted.push(ty);
        }
    }

    /// Records a write of `name` whose promotion chain becomes `promoted`.
    pub fn assign(&mut self, name: &str, promoted: Vec<ResolvedType>) -> bool {
        self.record_write(name);
        let Some(binding) = self.binding_mut(name) else {
            return false;
        };
        binding.fields.clear();
        if binding.promotable && !binding.captured {
            binding.promoted = promoted;
        } else {
            binding.promoted.clear();
        }
        true
    }

    pub fn promote(&mut self, name: &str, ty: ResolvedType) {
        if ty != ResolvedType::Unknown
            && self.binding(name).is_some_and(|binding| {
                binding.promotable
                    && !binding.captured
                    && (binding.promoted.last() != Some(&ty) || !binding.tested.contains(&ty))
            })
            && let Some(binding) = self.binding_mut(name)
        {
            binding.push_promotion(ty);
        }
    }

    pub fn demote(&mut self, name: &str) {
        self.record_write(name);
        self.forget(name);
    }

    /// Drops the promotions of `name` without recording a write.
    pub fn forget(&mut self, name: &str) {
        if self
            .binding(name)
            .is_some_and(|binding| !binding.promoted.is_empty() || !binding.fields.is_empty())
            && let Some(binding) = self.binding_mut(name)
        {
            binding.promoted.clear();
            binding.fields.clear();
        }
    }

    /// The state after `try`/`finally`: `self` (after the body and catches)
    /// with every binding the `finally` block changed, from `entry` to `end`,
    /// taken from `end`.
    pub fn attach_finally(&mut self, entry: &Self, end: &Self) {
        for (index, scope) in self.scopes.iter_mut().enumerate() {
            if entry
                .scopes
                .get(index)
                .zip(end.scopes.get(index))
                .is_some_and(|(before, after)| Arc::ptr_eq(before, after))
            {
                continue;
            }
            let changed = scope.keys().any(|name| {
                let before = entry.scopes.get(index).and_then(|scope| scope.get(name));
                let after = end.scopes.get(index).and_then(|scope| scope.get(name));
                after.is_some() && before != after
            });
            if !changed {
                continue;
            }
            for (name, binding) in Arc::make_mut(scope) {
                let before = entry.scopes.get(index).and_then(|scope| scope.get(name));
                if let Some(after) = end.scopes.get(index).and_then(|scope| scope.get(name))
                    && before != Some(after)
                {
                    binding.clone_from(after);
                }
            }
        }
    }

    pub fn enter_callable(&mut self) {
        self.callable_boundaries.push(self.scopes.len());
    }

    pub(super) fn record_write(&mut self, name: &str) {
        if let Some(boundary) = self.callable_boundaries.last()
            && let Some(index) = self.binding_depth(name)
            && index < *boundary
            && self.scopes[index]
                .get(name)
                .is_some_and(|binding| !binding.captured)
            && let Some(binding) = Arc::make_mut(&mut self.scopes[index]).get_mut(name)
        {
            binding.captured = true;
        }
    }

    pub fn demote_captured(&mut self) {
        for scope in &mut self.scopes {
            if !scope.values().any(|binding| {
                binding.captured && (!binding.promoted.is_empty() || !binding.fields.is_empty())
            }) {
                continue;
            }
            for binding in Arc::make_mut(scope).values_mut() {
                if binding.captured {
                    binding.promoted.clear();
                    binding.fields.clear();
                }
            }
        }
    }

    fn receiver_name<'syntax>(object: Expr<'syntax>) -> Option<&'syntax str> {
        match object {
            Expr::Ident(identifier) => Some(identifier.name()),
            Expr::This(_) => Some("this"),
            _ => None,
        }
    }

    pub fn promoted_field(&self, object: Expr<'_>, field: &str) -> Option<ResolvedType> {
        if matches!(object, Expr::This(_)) {
            return self.promoted_this_field(field);
        }
        self.binding(Self::receiver_name(object)?)?
            .fields
            .get(field)
            .cloned()
    }

    pub fn promote_field(&mut self, object: Expr<'_>, field: &str, ty: ResolvedType) {
        if matches!(object, Expr::This(_)) {
            self.promote_this_field(field, ty);
            return;
        }
        if let Some(name) = Self::receiver_name(object)
            && self.binding(name).is_some_and(|binding| {
                !binding.captured && binding.promotable && binding.fields.get(field) != Some(&ty)
            })
            && let Some(binding) = self.binding_mut(name)
        {
            binding.fields.insert(field.to_owned(), ty);
        }
    }

    pub fn join(&self, other: &Self) -> Self {
        let mut joined = self.clone();
        for (index, scope) in joined.scopes.iter_mut().enumerate() {
            // Identical snapshots already share every promotion and type of interest.
            // Capture may still disable promotion even when both branches agree.
            if other
                .scopes
                .get(index)
                .is_some_and(|peer| Arc::ptr_eq(scope, peer))
                && scope
                    .values()
                    .all(|binding| !binding.captured || !binding.promotable)
            {
                continue;
            }
            for (name, binding) in Arc::make_mut(scope) {
                let peer = other.scopes.get(index).and_then(|scope| scope.get(name));
                binding.captured |= peer.is_some_and(|peer| peer.captured);
                if binding.captured {
                    binding.promotable = false;
                }
                match peer {
                    // Dart keeps the types both promotion chains share, in order.
                    Some(peer) => {
                        binding.promoted.retain(|ty| peer.promoted.contains(ty));
                        for ty in &peer.tested {
                            if !binding.tested.contains(ty) {
                                binding.tested.push(ty.clone());
                            }
                        }
                    }
                    None => binding.promoted.clear(),
                }
                binding
                    .fields
                    .retain(|name, ty| peer.is_some_and(|peer| peer.fields.get(name) == Some(ty)));
            }
        }
        joined
    }

    pub fn bind_params(
        &mut self,
        params: FormalParamList<'_>,
        model: &SemanticModel<'_>,
        type_params: &TypeParameterScope,
    ) {
        for param in params.params() {
            let ty = model.resolve_formal_parameter_in(param, type_params);
            self.declare_at(param.name().name().to_owned(), ty, param.name().span());
        }
    }

    /// The number of open scopes.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// The index of the innermost open scope binding `name`.
    pub fn binding_depth(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))
    }

    pub fn is_bound(&self, name: &str) -> bool {
        self.binding(name).is_some()
    }

    /// Whether the nearest declaration of `name` belongs to the root scope.
    /// An inner declaration shadows the root, even when their types agree.
    pub fn is_bound_in_root_scope(&self, name: &str) -> bool {
        self.binding_depth(name) == Some(0)
    }

    pub fn lookup(&self, name: &str) -> ResolvedType {
        self.binding(name)
            .map(|binding| binding.promoted.last().unwrap_or(&binding.declared).clone())
            .unwrap_or(ResolvedType::Unknown)
    }
}

/// The syntax function literals and local function declarations share.
pub(super) struct FunctionSyntax<'f> {
    pub(super) type_params: AstChildren<'f, TypeParam<'f>>,
    pub(super) params: FormalParamList<'f>,
    pub(super) body: FunctionBody<'f>,
    pub(super) async_marker: falcon_syntax::cst::AsyncMarker,
    pub(super) return_type: Option<DartType<'f>>,
}
