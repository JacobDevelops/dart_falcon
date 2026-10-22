//! Flags arguments that pass exactly the parameter's default value.
//!
//! An optional parameter's default already applies when the argument is
//! omitted, so `f(flag: false)` against `void f({bool flag = false})` is noise.
//! The rule compares each constant argument with the constant default of the
//! optional parameter it binds to and reports matches. Arguments are visited
//! from last to first, stopping after the first optional positional parameter:
//! an earlier positional argument may be needed to reach a later one. For a
//! redirecting factory constructor the defaults of the final redirect target
//! apply. Required parameters and `@required` ones are never reported.
//!
//! Defaults come from indexed callable declarations, local functions and
//! function-valued locals, and the `call` method of callable objects. An optional
//! parameter without a declared default defaults to `null`; a super formal
//! inherits its super parameter's default.
//! Members of generic declarations whose function type mentions the owner's
//! type parameters are skipped: the analyzer invokes them through a
//! substituted type whose parameters carry no defaults. Values are evaluated
//! in the default's declaring library (including external named extensions)
//! and captured local constant scope, and the caller's lexical context separately.
//! Values that cannot be folded are skipped; the analyzer's unknown web
//! environment booleans remain unknown even with an explicit fallback.
//!
//! Adopted from the official `avoid_redundant_argument_values` lint.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

use falcon_analyze::constant_value::{
    contextual_number, evaluate_constant_with_local_values_in_context,
};
use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, LibraryId, ParameterDefault, QualifiedName,
    ResolvedSignature, ResolvedType, Rule, RulePreparation, evaluate_constant_in_library,
    expression_segments, resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{self as cst_visit, Visitor};
use falcon_syntax::cst::{self, AstChildren, Span};

use falcon_analyze::ConstContexts;
use crate::lint::const_eval::mentions_type_parameter;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState, explicit_call_receiver};

#[derive(Default)]
pub struct AvoidRedundantArgumentValues;

impl Rule for AvoidRedundantArgumentValues {
    fn prepare<'a>(
        &self,
        program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if ctx.identities().is_none() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            program,
            diags: Vec::new(),
            targets: Targets {
                ctx,
                contexts: ctx.const_contexts(program),
                locals: HashMap::new(),
            },
        }))
    }
}

/// The declared type of the named or positional parameter `name`.
fn parameter_type<'s>(signature: &'s ResolvedSignature, name: &str) -> Option<&'s ResolvedType> {
    signature.named.get(name).or_else(|| {
        let index = signature
            .positional_parameter_names
            .iter()
            .position(|parameter| parameter == name)?;
        signature.positional.get(index)
    })
}

/// The indexed signature and its lexical declaration owner.
struct Target<'s> {
    signature: Cow<'s, ResolvedSignature>,
    /// The named declaration's member namespace, when present.
    owner: Option<DeclarationIdentity>,
    /// An anonymous extension still has a declaring library.
    library: Option<LibraryId>,
    /// Local literal defaults are evaluated in their captured declaration scope.
    defaults: Option<Cow<'s, HashMap<String, ConstantValue>>>,
}

impl Target<'_> {
    /// Retain ownership once for a tear-off alias, never once per invocation.
    fn into_owned(self) -> Target<'static> {
        Target {
            signature: Cow::Owned(self.signature.into_owned()),
            owner: self.owner,
            library: self.library,
            defaults: self
                .defaults
                .map(|defaults| Cow::Owned(defaults.into_owned())),
        }
    }
}

struct Collector<'a> {
    program: cst::Program<'a>,
    diags: Vec<Diagnostic>,
    targets: Targets<'a>,
}

struct Targets<'a> {
    ctx: &'a AnalyzeContext<'a>,
    contexts: &'a ConstContexts,
    /// Local declarations keyed by the shared walker's visible binding identity.
    locals: HashMap<Span, Target<'static>>,
}

impl Targets<'_> {
    /// Redirecting factories bind defaults from the final target declaration.
    fn constructor_target<'s>(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        state: &'s SemanticState<'_>,
    ) -> Option<Target<'s>> {
        let mut owner = &identity;
        let mut name = name;
        let mut visited = HashSet::new();
        loop {
            let facts = state.signatures.declaration(owner).and_then(|declaration| {
                declaration
                    .constructors
                    .iter()
                    .find(|constructor| constructor.name == name)
            });
            if let Some((target, constructor)) =
                facts.and_then(|facts| facts.redirect_target.as_ref())
            {
                if !visited.insert((owner, name)) {
                    return None;
                }
                let ResolvedType::Interface { identity, .. } = target else {
                    return None;
                };
                owner = identity;
                name = constructor;
                continue;
            }
            return Some(Target {
                signature: Cow::Borrowed(state.signatures.constructor(owner, name)?),
                owner: Some(owner.clone()),
                library: None,
                defaults: None,
            });
        }
    }

    /// Instance members of generic declarations are invoked through their
    /// substituted function type. When that type mentions an owner type
    /// parameter, the analyzer rebuilds every parameter as a synthetic element
    /// without its default value, so no argument binds to a known default.
    fn method_target<'s>(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        state: &'s SemanticState<'_>,
    ) -> Option<Target<'s>> {
        let signature = state.signatures.member(&identity, name)?;
        (!signature.mentions_owner_parameters()).then_some(Target {
            signature: Cow::Borrowed(signature),
            owner: Some(identity),
            library: None,
            defaults: None,
        })
    }

    /// The class member, or else the applicable extension method, `name`
    /// invoked on `receiver`.
    fn instance_target<'s>(
        &self,
        receiver: &ResolvedType,
        name: &str,
        state: &'s SemanticState<'_>,
    ) -> Option<Target<'s>> {
        if let Some(owner) = state
            .signatures
            .resolved_member_owner(receiver, name, &state.model)
        {
            return self.method_target(owner, name, state);
        }
        let (owner, library, signature) =
            state
                .signatures
                .extension_method(receiver, name, &state.model)?;
        (!signature.mentions_owner_parameters()).then(|| Target {
            signature: Cow::Borrowed(signature),
            owner: owner.cloned(),
            library,
            defaults: None,
        })
    }

    /// The local function or function-valued local variable `name` binds.
    fn local_target<'s>(&'s self, name: &str, state: &SemanticState<'_>) -> Option<Target<'s>> {
        let declaration = state.environment.binding_declaration(name)?;
        let target = self.locals.get(&declaration)?;
        Some(Target {
            signature: Cow::Borrowed(&target.signature),
            owner: target.owner.clone(),
            library: target.library,
            defaults: target
                .defaults
                .as_ref()
                .map(|defaults| Cow::Borrowed(&**defaults)),
        })
    }

    /// Record a callable's actual source binding rather than a reusable scope index.
    fn declare_local(&mut self, declaration: Span, target: Option<Target<'static>>) {
        if let Some(target) = target {
            self.locals.insert(declaration, target);
        }
    }

    /// A local function or function literal's own declaration.
    fn literal_target(
        &self,
        type_params: AstChildren<'_, cst::TypeParam<'_>>,
        params: cst::FormalParamList<'_>,
        state: &SemanticState<'_>,
    ) -> Target<'static> {
        let mut scope = state.type_parameters.clone();
        scope.push(type_params.clone(), &state.model);
        let signature =
            ResolvedSignature::local_callable(&scope, type_params, params, &state.model);
        let owner = self.enclosing_type(state);
        let locals = state.visible_local_constants();
        let defaults = signature
            .defaults
            .iter()
            .filter_map(|(name, default)| {
                let value = match default {
                    ParameterDefault::Declared(expression) => {
                        evaluate_constant_with_local_values_in_context(
                            expression.expr(),
                            owner.as_ref(),
                            &locals,
                            &state.model,
                            &state.signatures,
                            parameter_type(&signature, name)
                                .filter(|ty| !mentions_type_parameter(ty)),
                        )
                    }
                    ParameterDefault::Null => Some(ConstantValue::Null),
                    ParameterDefault::Inherited { .. } => None,
                }?;
                Some((name.clone(), value))
            })
            .collect();
        Target {
            signature: Cow::Owned(signature),
            owner,
            library: None,
            defaults: Some(Cow::Owned(defaults)),
        }
    }

    fn enclosing_type(&self, state: &SemanticState<'_>) -> Option<DeclarationIdentity> {
        state.constant_owner.clone()
    }

    /// A decided lookup may be absent and must not fall through at this level.
    fn decided_level_target<'s>(
        &'s self,
        callee: cst::Expr<'_>,
        state: &'s SemanticState<'_>,
    ) -> ControlFlow<Option<Target<'s>>> {
        // `G<int>.named(...)`: type arguments on the class still name it.
        let segments = expression_segments(callee, true);
        if let cst::Expr::Ident(identifier) = callee.unparenthesized() {
            let name = identifier.name();
            if state.environment.is_bound(name) {
                return ControlFlow::Break(
                    self.local_target(name, state)
                        .or_else(|| self.instance_target(&state.infer(callee), "call", state)),
                );
            }
            if let Some(receiver @ ResolvedType::Interface { identity, .. }) =
                state.enclosing_type.as_ref()
            {
                if !state
                    .signatures
                    .declared_member_facts(identity, name)
                    .is_empty()
                {
                    return ControlFlow::Break(self.method_target(identity.clone(), name, state));
                }
                if let Some(owner) =
                    state
                        .signatures
                        .resolved_readable_member_owner(receiver, name, &state.model)
                {
                    return ControlFlow::Break(self.method_target(owner, name, state));
                }
            }
        }
        let unshadowed = segments.as_ref().is_some_and(|segments| {
            segments.first().is_some_and(|name| {
                !state.environment.is_bound(name)
                    && !state.enclosing_type.as_ref().is_some_and(|receiver| {
                        matches!(receiver, ResolvedType::Interface { identity, .. }
                            if !state.signatures.declared_member_facts(identity, name).is_empty())
                            || state
                                .signatures
                                .resolved_readable_member_owner(receiver, name, &state.model)
                                .is_some()
                    })
            })
        });
        if unshadowed {
            if let Some(identity) = segments
                .as_ref()
                .and_then(|segments| QualifiedName::from_segments(segments))
                .and_then(|name| state.model.resolve_value(name))
                && let Some(signature) = state.signatures.function(&identity)
            {
                return ControlFlow::Break(Some(Target {
                    signature: Cow::Borrowed(signature),
                    owner: Some(identity),
                    library: None,
                    defaults: None,
                }));
            }
            if let Some((identity, name)) = resolve_call_constructor(callee, &state.model, true)
                && let Some(target) = self.constructor_target(identity, name, state)
            {
                return ControlFlow::Break(Some(target));
            }
            if let cst::Expr::Field(field) = callee.unparenthesized()
                && let Some(identity) = expression_segments(field.object(), false)
                    .and_then(|segments| state.model.resolve_dotted(&segments))
                && state.signatures.type_kind(&identity).is_some()
            {
                return ControlFlow::Break(self.method_target(
                    identity,
                    field.field().name(),
                    state,
                ));
            }
        }
        ControlFlow::Continue(())
    }

    fn member_level_target<'s>(
        &'s self,
        callee: cst::Expr<'_>,
        state: &'s SemanticState<'_>,
    ) -> Option<Target<'s>> {
        let (receiver, name) = match callee.unparenthesized() {
            cst::Expr::Ident(identifier) => {
                (state.enclosing_type.as_ref()?.clone(), identifier.name())
            }
            cst::Expr::Field(field) => {
                let object = field.object().unparenthesized();
                // An explicit named extension call determines the extension
                // override before receiver inference can lose that identity.
                if let cst::Expr::Call(extension_call) = object
                    && let Some(identity) = state.named_extension(extension_call.callee())
                    && let Some(signature) = state
                        .signatures
                        .extension_override_method(&identity, field.field().name())
                {
                    return (!signature.mentions_owner_parameters()).then(|| Target {
                        signature: Cow::Borrowed(signature),
                        owner: Some(identity),
                        library: None,
                        defaults: None,
                    });
                }
                (state.infer_receiver(field.object()), field.field().name())
            }
            _ => return None,
        };
        self.instance_target(&receiver, name, state)
    }

    fn call_target<'s>(
        &'s self,
        callee: cst::Expr<'_>,
        state: &'s SemanticState<'_>,
    ) -> Option<Target<'s>> {
        let mut callee = callee;
        let mut fallback = None;
        loop {
            if let ControlFlow::Break(target) = self.decided_level_target(callee, state) {
                return target.or(fallback);
            }
            let target = self.member_level_target(callee, state);
            // The closest callable target is only a fallback; a target found
            // by following an explicit receiver takes precedence.
            let Some(receiver) = explicit_call_receiver(callee) else {
                return target.or(fallback);
            };
            if target.is_some() {
                fallback = target;
            }
            callee = receiver;
        }
    }

    fn check(
        &self,
        args: cst::ArgList<'_>,
        target: &Target<'_>,
        state: &SemanticState<'_>,
        caller: Option<&DeclarationIdentity>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let signature = &*target.signature;
        let mut arguments: Vec<_> = args
            .positional()
            .enumerate()
            .map(|(index, argument)| {
                (
                    argument,
                    signature
                        .positional_parameter_names
                        .get(index)
                        .map(String::as_str),
                    signature.positional.get(index),
                    true,
                )
            })
            .chain(args.named().map(|argument| {
                let name = argument.name().name();
                (
                    argument.value(),
                    signature.named.contains_key(name).then_some(name),
                    signature.named.get(name),
                    false,
                )
            }))
            .collect();
        arguments.sort_by_key(|(argument, ..)| argument.span().start);
        let caller_type = caller.map(|identity| match state.enclosing_type.as_ref() {
            Some(
                receiver @ ResolvedType::Interface {
                    identity: current, ..
                },
            ) if current == identity => Cow::Borrowed(receiver),
            _ => Cow::Owned(ResolvedType::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: Vec::new().into(),
                nullable: false,
                extension_type: false,
            }),
        });
        for (argument, name, ty, positional) in arguments.into_iter().rev() {
            let Some(name) = name else {
                continue;
            };
            self.check_argument(
                argument,
                (name, ty),
                target,
                caller_type.as_deref(),
                state,
                diags,
            );
            // Only the last positional argument can be omitted. Earlier
            // required positionals have no reportable default either.
            if positional {
                break;
            }
        }
    }

    fn check_argument(
        &self,
        argument: cst::Expr<'_>,
        parameter: (&str, Option<&ResolvedType>),
        target: &Target<'_>,
        caller: Option<&ResolvedType>,
        state: &SemanticState<'_>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let (name, declared_type) = parameter;
        let Some(default) = target.signature.defaults.get(name) else {
            return;
        };
        if target.signature.non_optional_defaults.contains(name)
            || !self.is_constant(argument, state, caller)
        {
            return;
        }
        let (default, default_owner) = match default {
            ParameterDefault::Declared(default) => (Some(default.expr()), target.owner.as_ref()),
            ParameterDefault::Inherited { owner, default } => match default.as_ref() {
                ParameterDefault::Declared(default) => (Some(default.expr()), Some(owner)),
                _ => (None, None),
            },
            ParameterDefault::Null => (None, None),
        };
        let double_context = declared_type.is_some_and(|ty| ty.interface("dart:core", "double"));
        // Collection literals take their type arguments from the parameter type,
        // so `const []` defaulting a `List<int>` equals `const <int>[]`.
        let context = declared_type.filter(|ty| !mentions_type_parameter(ty));
        let default_value = match default {
            Some(default) => {
                let value = match &target.defaults {
                    Some(defaults) => defaults.get(name).cloned(),
                    None => evaluate_constant_in_library(
                        default,
                        default_owner,
                        target.library,
                        &HashMap::new(),
                        context,
                        &state.model,
                        &state.signatures,
                    ),
                };
                let Some(value) = value else {
                    return;
                };
                contextual_number(value, default, double_context)
            }
            None => ConstantValue::Null,
        };
        let caller_owner = match caller {
            Some(ResolvedType::Interface { identity, .. }) => Some(identity),
            _ => None,
        };
        let Some(value) = evaluate_constant_with_local_values_in_context(
            argument,
            caller_owner,
            &state.visible_local_constants(),
            &state.model,
            &state.signatures,
            context,
        ) else {
            return;
        };
        if contextual_number(value, argument, double_context) != default_value {
            return;
        }
        diags.push(self.ctx.diagnostic("The value of the argument is redundant because it matches the default value.", argument.span()));
    }

    /// Whether `argument` is a constant expression: collection literals and
    /// constructor calls only count when they are in a constant context.
    fn is_constant(
        &self,
        argument: cst::Expr<'_>,
        state: &SemanticState<'_>,
        caller: Option<&ResolvedType>,
    ) -> bool {
        let mut checker = NonConstantFinder {
            found: false,
            constant_context: self.contexts.contains_including_explicit(&argument.span()),
            state,
            caller,
        };
        checker.visit_expr(argument);
        !checker.found
    }

    fn visit_declarations(
        &self,
        program: cst::Program<'_>,
        state: &SemanticState<'_>,
        diags: &mut Vec<Diagnostic>,
    ) {
        for declaration in program.declarations() {
            let name = match declaration {
                cst::TopLevelDecl::Class(declaration) => Some(declaration.name()),
                cst::TopLevelDecl::Mixin(declaration) => Some(declaration.name()),
                cst::TopLevelDecl::MixinClass(declaration) => Some(declaration.name()),
                cst::TopLevelDecl::Enum(declaration) => Some(declaration.name()),
                cst::TopLevelDecl::Extension(declaration) => declaration.name(),
                cst::TopLevelDecl::ExtensionType(declaration) => Some(declaration.name()),
                _ => None,
            };
            let owner = name.and_then(|name| state.model.resolve_name(name.name()));
            if let cst::TopLevelDecl::Enum(declaration) = declaration
                && let Some(owner) = &owner
            {
                for variant in declaration.variants() {
                    let Some(args) = variant.args() else { continue };
                    let name = variant.constructor_name().map_or("new", |name| name.name());
                    if let Some(target) = self.constructor_target(owner.clone(), name, state) {
                        self.check(args, &target, state, Some(owner), diags);
                    }
                }
            }
            let mut visit = |annotation: cst::Annotation<'_>| {
                let Some(args) = annotation.args() else {
                    return;
                };
                let Some(identity) = QualifiedName::from_identifiers(annotation.name())
                    .and_then(|name| state.model.resolve_name(name))
                else {
                    return;
                };
                let constructor = annotation
                    .constructor_name()
                    .map_or("new", |name| name.name());
                if let Some(target) = self.constructor_target(identity, constructor, state) {
                    self.check(args, &target, state, owner.as_ref(), diags);
                }
            };
            AnnotationVisitor { visit: &mut visit }.visit_top_level_decl(declaration);
        }
    }
}

/// Rejects nonconstant candidates and local bindings that shadow indexed constants.
struct NonConstantFinder<'a, 'm> {
    found: bool,
    constant_context: bool,
    state: &'a SemanticState<'m>,
    caller: Option<&'a ResolvedType>,
}
impl<'a> cst_visit::Visitor<'a> for NonConstantFinder<'_, '_> {
    fn visit_expr(&mut self, node: cst::Expr<'a>) {
        if let cst::Expr::Ident(identifier) = node {
            let name = identifier.name();
            let constant = matches!(self.caller,
                Some(ResolvedType::Interface { identity, .. })
                    if self.state.signatures.declaration(identity).is_some_and(|facts|
                        facts.static_consts.iter().any(|field| field.name == name)));
            let bound = self.state.environment.is_bound(name)
                && self.state.local_constant(name).is_none()
                && !(constant && self.state.environment.is_bound_in_root_scope(name));
            let member = self.caller.is_some_and(|receiver| {
                matches!(receiver, ResolvedType::Interface { identity, .. }
                    if !self.state.signatures.declared_member_facts(identity, name).is_empty())
                    || self
                        .state
                        .signatures
                        .resolved_readable_member_owner(receiver, name, &self.state.model)
                        .is_some()
            });
            if bound || (member && !constant) {
                self.found = true;
            }
            return;
        }
        let (is_container, is_const) = match node {
            cst::Expr::List(value) => (true, value.is_const()),
            cst::Expr::Map(value) => (true, value.is_const()),
            cst::Expr::Set(value) => (true, value.is_const()),
            cst::Expr::Record(value) => (true, value.is_const()),
            cst::Expr::New(value) => (true, value.is_const()),
            _ => (false, false),
        };
        if is_container {
            // Unlike other collections, a record literal is not itself nonconstant;
            // descend into its fields to check the contained expressions.
            if !is_const && !self.constant_context && !matches!(node, cst::Expr::Record(_)) {
                self.found = true;
                return;
            }
            let previous = self.constant_context;
            self.constant_context |= is_const;
            cst_visit::walk_expr(self, node);
            self.constant_context = previous;
            return;
        }
        match node {
            cst::Expr::Call(_) if !self.constant_context => self.found = true,
            cst::Expr::FuncExpr(_) | cst::Expr::This(_) => self.found = true,
            _ => cst_visit::walk_expr(self, node),
        }
    }
}

/// Visit annotation arguments immediately without cloning or retaining their syntax nodes.
struct AnnotationVisitor<'a, 'tree> {
    visit: &'a mut dyn FnMut(cst::Annotation<'tree>),
}

impl<'tree> cst_visit::Visitor<'tree> for AnnotationVisitor<'_, 'tree> {
    fn visit_annotation(&mut self, node: cst::Annotation<'tree>) {
        (self.visit)(node);
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        self.targets
            .visit_declarations(self.program, state, &mut self.diags);
    }

    fn visit_stmt(&mut self, node: cst::Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            cst::Stmt::LocalFunc(function) => {
                let target =
                    self.targets
                        .literal_target(function.type_params(), function.params(), state);
                self.targets
                    .declare_local(function.name().span(), Some(target));
            }
            cst::Stmt::LocalVar(declaration) => {
                for declarator in declaration.declarators() {
                    let initializer = declarator.initializer().map(cst::Expr::unparenthesized);
                    let target = match initializer {
                        _ if declaration.var_type().is_some() => None,
                        Some(cst::Expr::FuncExpr(function)) => Some(self.targets.literal_target(
                            function.type_params(),
                            function.params(),
                            state,
                        )),
                        Some(initializer @ (cst::Expr::Ident(_) | cst::Expr::Field(_))) => {
                            self.targets.call_target(initializer, state)
                        }
                        _ => None,
                    }
                    .map(Target::into_owned);
                    self.targets.declare_local(declarator.name().span(), target);
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: cst::Expr<'a>, state: &SemanticState<'a>) {
        let target = match node {
            cst::Expr::Call(call) => self
                .targets
                .call_target(call.callee(), state)
                .map(|target| (target, call.args())),
            cst::Expr::New(new) => match state.model.resolve_type(new.dart_type()) {
                ResolvedType::Interface { identity, .. } => self
                    .targets
                    .constructor_target(
                        identity,
                        new.constructor_name().map_or("new", |name| name.name()),
                        state,
                    )
                    .map(|target| (target, new.args())),
                _ => None,
            },
            cst::Expr::Cascade(cascade) => {
                let receiver = state.infer(cascade.object());
                let caller = self.targets.enclosing_type(state);
                // The first operation identifies this section's selector;
                // later cascade operations apply to its result.
                for section in cascade.sections() {
                    if let Some(cst::CascadeOp::Call(call)) = section.ops().next()
                        && let Some(target) =
                            self.targets
                                .instance_target(&receiver, call.name().name(), state)
                    {
                        self.targets.check(
                            call.args(),
                            &target,
                            state,
                            caller.as_ref(),
                            &mut self.diags,
                        );
                    }
                }
                None
            }
            _ => None,
        };
        if let Some((target, args)) = target {
            let caller = self.targets.enclosing_type(state);
            self.targets
                .check(args, &target, state, caller.as_ref(), &mut self.diags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_analyze::{IdentityIndex, IdentitySource, ParsedFile, SignatureIndex, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// In-memory imported libraries: no filesystem or installed SDK is needed.
    fn reports(source: &str, imported: &str) -> Vec<String> {
        let path = PathBuf::from("/project/lib/main.dart");
        let imported_path = PathBuf::from("/project/lib/imported.dart");
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let dependency = parse(imported);
        assert!(dependency.errors.is_empty(), "{:?}", dependency.errors);
        let dependency = Arc::new(ParsedFile {
            source: imported.into(),
            tree: dependency.tree,
        });
        let indexes = crate::lint::test_support::TestIndexes::new(&[(&path, &parsed), (&imported_path, &dependency)]);
        let config = FalconConfig::default();
        let context = indexes.context(&path, &config, &parsed);
        let mut diagnostics = AvoidRedundantArgumentValues.analyze(AvoidRedundantArgumentValues::NAME, parsed.program(), &context);
        diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
        diagnostics
            .into_iter()
            .map(|diagnostic| source[diagnostic.span.start..diagnostic.span.end].to_string())
            .collect()
    }

    #[test]
    fn external_extension_override_defaults_use_declaring_library() {
        use falcon_analyze::{
            AnalysisEnvironment, ExternalLibraries, ExternalSources, LibrarySource,
        };

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let app = root.join("app");
        let dep = root.join("dep");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(app.join("lib")).unwrap();
        std::fs::create_dir_all(dep.join("lib")).unwrap();

        crate::lint::test_support::write_sdk(root, &[("core", include_str!("../../../tests/support/sdk/lib/core/core.dart"))]);
        std::fs::write(
            app.join(".dart_tool/package_config.json"),
            serde_json::json!({
                "configVersion": 2,
                "flutterRoot": "../../flutter",
                "packages": [
                    {"name":"app","rootUri":"../","packageUri":"lib/"},
                    {"name":"dep","rootUri":"../../dep","packageUri":"lib/"}
                ]
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dep.join("lib/ext.dart"),
            "const _limit = 5; extension Ex on int { void take([int n = _limit]) {} }",
        )
        .unwrap();
        std::fs::write(dep.join("lib/api.dart"), "export 'ext.dart' show Ex;").unwrap();
        let path = app.join("lib/main.dart");
        let source = r#"
import 'package:dep/api.dart' as p;
const _limit = 9;
void run() {
  p.Ex(0).take(5);
  p.Ex(0).take(9);
  p.Ex(0).take(_limit);
}
"#;
        std::fs::write(&path, source).unwrap();
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let external = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), parsed.program(), source)],
            |candidate| candidate == path,
        );
        let identities = IdentityIndex::build(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        let mut types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                LibrarySource {
                    program: parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        types.add_external(&external, &identities);
        let signatures = SignatureIndex::build(
            &[(path.clone(), Arc::clone(&parsed))],
            Some(&external),
            &identities,
            &types,
        );
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        let diagnostics = AvoidRedundantArgumentValues.analyze(AvoidRedundantArgumentValues::NAME, parsed.program(), &context);
        let arguments: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(arguments, ["5"]);
    }

    const IMPORTED: &str = r#"
const seed = 7;
enum Mode { on, off }
void function({int value = seed}) {}
class Base {
  static const seed = 11;
  Base({int value = seed});
  Base.named({int value = seed});
  void method({int value = seed, Mode mode = Mode.on}) {}
  static void staticMethod({int value = seed}) {}
}
class Derived extends Base {
  static const seed = 99;
}
"#;

    #[test]
    fn imported_functions_methods_and_constructors_use_indexed_defaults() {
        let source = r#"
import 'imported.dart';
void run(Base base) {
  function(value: 7);
  base.method(value: 11);
  Base(value: 11);
  Base.named(value: 11);
  Base.staticMethod(value: 11);
  function(value: 8);
  base.method(value: 12);
}
"#;
        assert_eq!(reports(source, IMPORTED), ["7", "11", "11", "11", "11"]);
    }

    #[test]
    fn prefixed_calls_and_inherited_methods_use_declaring_owner_constants() {
        let source = r#"
import 'imported.dart' as dep;
const seed = 99;
enum Mode { on, off }
class Caller {
  static const seed = 99;
  void run(dep.Derived derived) {
    dep.function(value: 7);
    dep.Base(value: 11);
    dep.Base.named(value: 11);
    dep.Base.staticMethod(value: 11);
    derived.method(value: 11, mode: dep.Mode.on);
    derived.method(value: seed, mode: Mode.on);
  }
}
"#;
        assert_eq!(
            reports(source, IMPORTED),
            ["7", "11", "11", "11", "11", "dep.Mode.on"]
        );
    }

    #[test]
    fn caller_constants_are_evaluated_in_the_callers_lexical_context() {
        let source = r#"
import 'imported.dart' as dep;
const seed = 7;
class Caller {
  static const seed = 11;
  void run(dep.Base base) {
    dep.function(value: 7);
    dep.function(value: seed);
    base.method(value: seed);
  }
}
void run() { dep.function(value: seed); }
"#;
        assert_eq!(reports(source, IMPORTED), ["7", "seed", "seed"]);
    }

    #[test]
    fn only_last_optional_positional_argument_is_omittable() {
        let source = r#"
import 'imported.dart' as dep;
void run() {
  dep.positional(1, 2);
  dep.positional(1, 3);
  dep.positional(1);
  dep.noLastDefault(1, null);
  dep.annotated(1, 2);
}
"#;
        let imported = r#"
void positional([int first = 1, int second = 2]) {}
void noLastDefault([int first = 1, int? second]) {}
void annotated([int first = 1, @required int second = 2]) {}
"#;
        // A missing default is `null`: `noLastDefault(1, null)` is redundant.
        assert_eq!(reports(source, imported), ["2", "1", "null"]);
    }

    #[test]
    fn abstract_method_defaults_remain_reportable_without_sdk_special_cases() {
        let source = r#"
import 'imported.dart';
void run(Range range, Iterable<int> values) {
  range.setRange(0, 1, values, 0);
  range.setRange(0, 1, values, 1);
}
"#;
        let imported = r#"
abstract class Range {
  void setRange(int start, int end, Iterable<int> values, [int skipCount = 0]);
}
"#;
        assert_eq!(reports(source, imported), ["0"]);
    }

    #[test]
    fn required_annotations_shadowed_callees_and_nonconstant_candidates_are_not_reported() {
        let source = r#"
import 'imported.dart' as dep;
void run(dynamic dep, dynamic function) {
  dep.function(value: 7);
  function(value: 7);
}
void other() {
  dep.requiredValue(value: 7);
  dep.annotated(value: 7);
  dep.collection(value: [1]);
}
"#;
        let imported = r#"
void function({int value = 7}) {}
void requiredValue({required int value = 7}) {}
void annotated({@required int value = 7}) {}
void collection({List<int> value = const [1]}) {}
"#;
        assert!(reports(source, imported).is_empty());
    }

    #[test]
    fn imported_and_local_redirecting_factories_follow_final_defaults() {
        let source = r#"
import 'imported.dart' as dep;
abstract class LocalFactory {
  factory LocalFactory({int value}) = LocalTarget;
}
class LocalTarget implements LocalFactory {
  static const seed = 19;
  LocalTarget({int value = seed});
}
void run() {
  dep.Factory(value: 23);
  dep.Factory(value: 0);
  LocalFactory(value: 19);
  LocalFactory(value: 0);
}
"#;
        let imported = r#"
abstract class Factory {
  factory Factory({int value}) = Target;
}
class Target implements Factory {
  static const seed = 23;
  Target({int value = seed});
}
"#;
        assert_eq!(reports(source, imported), ["23", "19"]);
    }

    #[test]
    fn initializing_formal_double_type_preserves_numeric_candidate_semantics() {
        let source = r#"
class Box {
  final double value;
  Box([this.value = 0.0]);
}
void integer([int value = 0]) {}
void run() {
  Box(0);
  integer(0.0);
}
"#;
        assert_eq!(reports(source, ""), ["0"]);
    }

    #[test]
    fn caller_parameters_and_receiver_roots_cannot_capture_static_constants() {
        let source = r#"
import 'imported.dart' as dep;
class Caller {
  static const seed = 11;
  void run(dep.Base base, int seed, dynamic dep) {
    base.method(value: seed);
    base.method(mode: dep.Mode.on);
  }
}
"#;
        assert!(reports(source, IMPORTED).is_empty());
    }

    #[test]
    fn annotations_and_enum_variants_keep_their_lexical_owner() {
        let source = r#"
import 'imported.dart' as dep;
const seed = 99;
class Annotated {
  static const seed = 0;
  @dep.Mark(value: seed)
  void method() {}
}
enum Choice {
  on(seed);
  static const seed = 0;
  final int value;
  const Choice([this.value = seed]);
}
"#;
        let imported = "class Mark { const Mark({int value = 0}); }";
        assert_eq!(reports(source, imported), ["seed", "seed"]);
    }

    #[test]
    fn indexed_getters_and_methods_shadow_top_level_names() {
        let source = r#"
import 'imported.dart' as dep;
const seed = 11;
void work([int value = 1]) {}
void accepts({Object value = seed}) {}
class GetterCaller {
  void callback(int value) {}
  void Function(int) get work => callback;
  int get seed => 11;
  dynamic get dep => null;
  void run() {
    work(1);
    accepts(value: seed);
    dep.function(value: 7);
  }
}
class MethodCaller {
  void seed() {}
  void run() { accepts(value: seed); }
}
class Base {
  void work([int value = 2]) {}
}
class Derived extends Base {
  void run() { work(1); work(2); }
}
"#;
        assert_eq!(reports(source, IMPORTED), ["2"]);
    }

    #[test]
    fn double_aliases_and_explicit_collection_types_remain_distinct() {
        let source = r#"
const one = 1.0;
void object([Object value = 1]) {}
void typed([Object value = const <int>[1]]) {}
void number([double value = 1.0]) {}
void signed([double value = -0.0]) {}
void run() {
  object(one);
  object(1 / 1);
  typed(const <num>[1]);
  number(1);
  signed(-0);
  signed(0);
}
"#;
        assert_eq!(reports(source, ""), ["1", "-0"]);
    }

    #[test]
    fn sibling_scope_bindings_never_inherit_a_closed_local_callable() {
        let source = r#"
class List<E> {
  void forEach(void Function(E) action) {}
}
void run(bool d, List<void Function(int)> cbs, void Function(int) callback) {
  if (d) { void log([int l = 0]) {} }
  cbs.forEach((log) => log(0));
  {
    if (d) { void log([int l = 0]) {} }
    cbs.forEach((log) => log(0));
  }
  if (d) { void log([int l = 0]) {} }
  for (final log in cbs) { log(0); }
  if (d) { void log([int l = 0]) {} }
  if (callback case final log) { log(0); }
  if (d) { void log([int l = 0]) {} }
  try {} on void Function(int) catch (log) { log(0); }
}
"#;
        assert!(reports(source, "").is_empty());
    }

    #[test]
    fn local_defaults_capture_enclosing_constants_at_their_declaration() {
        let source = r#"
const x = 2;
void run() {
  const x = 1;
  void g([int a = x]) {}
  final alias = g;
  final closure = ([int a = x]) {};
  g(2);
  g(1);
  alias(2);
  alias(1);
  closure(2);
  closure(1);
  {
    const x = 3;
    g(3);
    g(1);
    alias(3);
    alias(1);
  }
  g(x);
}
class Host {
  static const x = 2;
  void run() {
    const x = 1;
    void g([int a = x]) {}
    g(2);
    g(1);
  }
}
"#;
        assert_eq!(reports(source, ""), ["1", "1", "1", "1", "1", "x", "1"]);
    }

    #[test]
    fn anonymous_extension_defaults_use_library_not_caller_class_constants() {
        let source = r#"
const limit = 1;
class Receiver {}
extension on Receiver {
  void method([int value = limit]) {}
}
class Caller {
  static const limit = 2;
  void run(Receiver receiver) {
    receiver.method(2);
    receiver.method(1);
    final alias = receiver.method;
    alias(2);
    alias(1);
  }
}
"#;
        assert_eq!(reports(source, ""), ["1", "1"]);
    }

    #[test]
    fn nullable_double_constants_coerce_integer_literals_without_sdk() {
        let source = r#"
const double? topOne = 1;
const double? topNegativeZero = -0;
class Defaults {
  static const double? one = 1;
  static const double? negativeZero = -0;
}
void one([num? value = 1.0]) {}
void negative([num? value = -0.0]) {}
void nullable([double? value = -0]) {}
void run() {
  const double? localOne = 1;
  const double? localNegativeZero = -0;
  one(topOne);
  one(Defaults.one);
  one(localOne);
  negative(topNegativeZero);
  negative(Defaults.negativeZero);
  negative(localNegativeZero);
  nullable(-0);
}
"#;
        assert_eq!(
            reports(source, ""),
            [
                "topOne",
                "Defaults.one",
                "localOne",
                "topNegativeZero",
                "Defaults.negativeZero",
                "localNegativeZero",
                "-0",
            ]
        );
    }

    #[test]
    fn typed_constants_preserve_double_identity_and_negative_zero_without_sdk() {
        let source = r#"
const double topOne = 1;
const double topNegativeZero = -0;
class Defaults {
  static const double one = 1;
  static const double negativeZero = -0;
}
void one([num value = 1.0]) {}
void negative([num value = -0.0]) {}
void positive([num value = 0.0]) {}
void topDefault([num value = topNegativeZero]) {}
void staticDefault([num value = Defaults.negativeZero]) {}
void run() {
  const double localOne = 1;
  const double localNegativeZero = -0;
  const captured = localNegativeZero;
  one(topOne);
  one(Defaults.one);
  one(localOne);
  negative(topNegativeZero);
  negative(Defaults.negativeZero);
  negative(localNegativeZero);
  negative(captured);
  topDefault(-0.0);
  staticDefault(-0.0);
  void localDefault([num value = localNegativeZero]) {}
  localDefault(-0.0);
  positive(topNegativeZero);
  positive(Defaults.negativeZero);
  positive(localNegativeZero);
  negative(0.0);
  const integer = 1;
  one(integer);
}
"#;
        assert_eq!(
            reports(source, ""),
            [
                "topOne",
                "Defaults.one",
                "localOne",
                "topNegativeZero",
                "Defaults.negativeZero",
                "localNegativeZero",
                "captured",
                "-0.0",
                "-0.0",
                "-0.0",
            ]
        );
    }

    #[test]
    fn transitive_local_constants_retain_their_initializer_scope() {
        let source = r#"
void f([int a = 2]) {}
void run() {
  const x = 1;
  const y = x;
  {
    const x = 2;
    f(y);
    void g([int a = y]) {}
    g(2);
    g(1);
    final alias = g;
    alias(2);
    alias(1);
  }
}
"#;
        assert_eq!(reports(source, ""), ["1", "1"]);
    }

    #[test]
    fn extension_local_defaults_and_arguments_do_not_use_on_type_statics() {
        let source = r#"
const limit = 1;
void f([int a = 1]) {}
class Receiver { static const limit = 2; }
extension on Receiver {
  void run() {
    void g([int value = limit]) {}
    g(2);
    g(1);
    f(limit);
    const captured = limit;
    void h([int value = captured]) {}
    h(2);
    h(1);
  }
}
"#;
        assert_eq!(reports(source, ""), ["1", "limit", "1"]);
    }

    #[test]
    fn cli_environment_constants_preserve_unknown_web_values_without_sdk() {
        let source = r#"
const web = bool.fromEnvironment('dart.library.js_util');
const interop = bool.fromEnvironment('dart.library.js_interop');
const name = 'dart.library.js_util';
void yes({bool value = true}) {}
void no({bool value = false}) {}
void text({String value = ''}) {}
void number({int value = 0}) {}
void cursor({int value = 2}) {}
void unknownDefault({bool value = web}) {}
void run() {
  yes(value: web);
  no(value: web);
  yes(value: interop);
  no(value: interop);
  no(value: const bool.fromEnvironment(name));
  yes(value: const bool.fromEnvironment(name, defaultValue: true));
  cursor(value: web ? 1 : 2);
  unknownDefault(value: false);
  no(value: const bool.fromEnvironment('user.option'));
  no(value: const bool.fromEnvironment('dart.library.io'));
  yes(value: const bool.fromEnvironment('dart.library.io', defaultValue: true));
  no(value: const bool.hasEnvironment('dart.library.js_util'));
  text(value: const String.fromEnvironment('dart.library.js_util'));
  number(value: const int.fromEnvironment('dart.library.js_interop'));
}
"#;
        assert_eq!(
            reports(source, ""),
            [
                "const bool.fromEnvironment('user.option')",
                "const bool.fromEnvironment('dart.library.io')",
                "const bool.fromEnvironment('dart.library.io', defaultValue: true)",
                "const bool.hasEnvironment('dart.library.js_util')",
                "const String.fromEnvironment('dart.library.js_util')",
                "const int.fromEnvironment('dart.library.js_interop')",
            ]
        );
    }

    #[test]
    fn environment_special_cases_require_core_constructor_provenance() {
        let source = r#"
import 'imported.dart' as dep;
void accepts({dep.bool value = const dep.bool.fromEnvironment('dart.library.js_util')}) {}
void run() {
  accepts(value: const dep.bool.fromEnvironment('dart.library.js_util'));
}
"#;
        let imported = r#"
class bool {
  final String name;
  const bool.fromEnvironment(this.name);
}
"#;
        assert_eq!(
            reports(source, imported),
            ["const dep.bool.fromEnvironment('dart.library.js_util')"]
        );
    }
}
